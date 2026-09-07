//! The SQLite-backed memory store.
//!
//! Synchronous `rusqlite` calls are wrapped with `tokio::task::spawn_blocking`
//! so the store is safe to use from the async agent loop. All data is local.

use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::types::{Memory, MemoryConfig, MemoryKind, ScoredMemory};

/// Embedding dimension used across the app. Keep in sync with the configured
/// embedding model (text-embedding-3-small with dimensions=384).
pub const EMBEDDING_DIM: usize = 384;

/// A cloned handle to the memory store. Cheap to clone, safe to share across
/// tasks. All operations are async.
#[derive(Clone)]
pub struct MemoryStore {
    conn: Arc<Mutex<Connection>>,
    cfg: MemoryConfig,
}

impl MemoryStore {
    /// Open (creating if necessary) the memory database and run migrations.
    pub async fn open(cfg: MemoryConfig) -> Result<Self> {
        let db_path = cfg.db_path.clone();
        let conn = tokio::task::spawn_blocking(move || -> Result<Connection> {
            if let Some(parent) = Path::new(&db_path).parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent).ok();
                }
            }

            // Register sqlite-vec as an auto extension before opening any connection.
            type AutoExtEntry = unsafe extern "C" fn(
                *mut rusqlite::ffi::sqlite3,
                *mut *mut std::os::raw::c_char,
                *const rusqlite::ffi::sqlite3_api_routines,
            ) -> std::os::raw::c_int;
            unsafe {
                let init: AutoExtEntry =
                    std::mem::transmute(sqlite_vec::sqlite3_vec_init as *const ());
                rusqlite::ffi::sqlite3_auto_extension(Some(init));
            }

            let conn = Connection::open(&db_path)
                .with_context(|| format!("failed to open memory db at {db_path}"))?;
            conn.pragma_update(None, "journal_mode", "WAL")?;
            conn.pragma_update(None, "foreign_keys", "ON")?;
            migrate(&conn)?;
            Ok(conn)
        })
        .await??;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            cfg,
        })
    }

    /// Run a blocking closure against the connection on a blocking thread.
    async fn with_conn<T, F>(&self, f: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&Connection) -> Result<T> + Send + 'static,
    {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || {
            let guard = conn.lock().expect("memory conn poisoned");
            f(&guard)
        })
        .await?
    }

    pub fn config(&self) -> &MemoryConfig {
        &self.cfg
    }

    /// Insert a memory (no embedding yet).
    pub async fn insert(&self, memory: &Memory) -> Result<()> {
        let m = memory.clone();
        self.with_conn(move |conn| {
            conn.execute(
                "INSERT INTO memories (id, kind, content, role, source, importance, tags, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    m.id.to_string(),
                    m.kind.as_str(),
                    m.content,
                    m.role,
                    m.source,
                    m.importance,
                    serde_json::to_string(&m.tags)?,
                    m.created_at.to_rfc3339(),
                ],
            )?;
            // Keep the FTS index in sync.
            conn.execute(
                "INSERT INTO memories_fts (rowid, content)
                 SELECT rowid, content FROM memories WHERE id = ?1",
                params![m.id.to_string()],
            )?;
            Ok(())
        })
        .await
    }

    /// Attach an embedding vector to an existing memory for semantic recall.
    pub async fn set_embedding(&self, id: Uuid, embedding: Vec<f32>) -> Result<()> {
        if embedding.len() != EMBEDDING_DIM {
            anyhow::bail!(
                "embedding dim mismatch: got {}, expected {EMBEDDING_DIM}",
                embedding.len()
            );
        }
        self.with_conn(move |conn| {
            let vec_json = serde_json::to_string(&embedding)?;
            conn.execute(
                "INSERT OR REPLACE INTO memories_vec (id, embedding) VALUES (?1, ?2)",
                params![id.to_string(), vec_json],
            )?;
            Ok(())
        })
        .await
    }

    /// Keyword recall over FTS5.
    pub async fn recall_keyword(&self, query: &str, limit: usize) -> Result<Vec<ScoredMemory>> {
        let q = query.to_string();
        self.with_conn(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT m.id, m.kind, m.content, m.role, m.source, m.importance, m.tags, m.created_at,
                        bm25(memories_fts) AS rank
                 FROM memories_fts f
                 JOIN memories m ON m.rowid = f.rowid
                 WHERE memories_fts MATCH ?1
                 ORDER BY rank
                 LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![q, limit as i64], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, f32>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, f64>(8)?,
                ))
            })?;
            let mut out = Vec::new();
            for r in rows.flatten() {
                if let Some(sm) = row_to_scored(r, None) {
                    out.push(sm);
                }
            }
            Ok(out)
        })
        .await
    }

    /// Vector recall over sqlite-vec using cosine distance.
    pub async fn recall_vector(
        &self,
        query_embedding: Vec<f32>,
        limit: usize,
    ) -> Result<Vec<ScoredMemory>> {
        if query_embedding.len() != EMBEDDING_DIM {
            anyhow::bail!(
                "query embedding dim mismatch: got {}, expected {EMBEDDING_DIM}",
                query_embedding.len()
            );
        }
        self.with_conn(move |conn| {
            let vec_json = serde_json::to_string(&query_embedding)?;
            let mut stmt = conn.prepare(
                "SELECT m.id, m.kind, m.content, m.role, m.source, m.importance, m.tags, m.created_at,
                        vec_distance_cosine(v.embedding, ?1) AS distance
                 FROM memories_vec v
                 JOIN memories m ON m.id = v.id
                 ORDER BY distance
                 LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![vec_json, limit as i64], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, f32>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, f64>(8)?,
                ))
            })?;
            let mut out = Vec::new();
            for r in rows.flatten() {
                if let Some(sm) = row_to_scored(r, Some(true)) {
                    out.push(sm);
                }
            }
            Ok(out)
        })
        .await
    }

    /// Recent episodic turns for a session-ish window (most recent last).
    pub async fn recent_episodic(&self, limit: usize) -> Result<Vec<Memory>> {
        self.with_conn(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, kind, content, role, source, importance, tags, created_at
                 FROM memories
                 WHERE kind = 'episodic'
                 ORDER BY created_at DESC
                 LIMIT ?1",
            )?;
            let rows = stmt.query_map(params![limit as i64], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, f32>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    0.0_f64,
                ))
            })?;
            let mut out: Vec<Memory> = rows
                .flatten()
                .filter_map(|r| row_to_scored(r, None).map(|s| s.memory))
                .collect();
            out.reverse(); // chronological
            Ok(out)
        })
        .await
    }

    /// Delete all episodic memories (after consolidation).
    pub async fn clear_episodic(&self) -> Result<usize> {
        self.with_conn(move |conn| {
            let n = conn.execute("DELETE FROM memories WHERE kind = 'episodic'", [])?;
            // Rebuild FTS to drop removed rows.
            conn.execute_batch(
                "DELETE FROM memories_fts WHERE content NOT IN (SELECT content FROM memories);",
            )?;
            Ok(n)
        })
        .await
    }

    /// Count memories by kind.
    pub async fn count(&self, kind: MemoryKind) -> Result<usize> {
        let k = kind.as_str().to_string();
        self.with_conn(move |conn| {
            let n: i64 = conn.query_row(
                "SELECT COUNT(*) FROM memories WHERE kind = ?1",
                params![k],
                |r| r.get(0),
            )?;
            Ok(n as usize)
        })
        .await
    }
}

#[async_trait::async_trait]
impl crate::backend::MemoryBackend for MemoryStore {
    async fn insert(&self, memory: &Memory) -> Result<()> {
        MemoryStore::insert(self, memory).await
    }
    async fn set_embedding(&self, id: Uuid, embedding: Vec<f32>) -> Result<()> {
        MemoryStore::set_embedding(self, id, embedding).await
    }
    async fn recall_keyword(&self, query: &str, limit: usize) -> Result<Vec<ScoredMemory>> {
        MemoryStore::recall_keyword(self, query, limit).await
    }
    async fn recall_vector(
        &self,
        query_embedding: Vec<f32>,
        limit: usize,
    ) -> Result<Vec<ScoredMemory>> {
        MemoryStore::recall_vector(self, query_embedding, limit).await
    }
    async fn recent_episodic(&self, limit: usize) -> Result<Vec<Memory>> {
        MemoryStore::recent_episodic(self, limit).await
    }
    async fn clear_episodic(&self) -> Result<usize> {
        MemoryStore::clear_episodic(self).await
    }
    async fn count(&self, kind: MemoryKind) -> Result<usize> {
        MemoryStore::count(self, kind).await
    }
}

type RowTuple = (
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    f32,
    String,
    String,
    f64,
);

fn row_to_scored(t: RowTuple, is_vector: Option<bool>) -> Option<ScoredMemory> {
    let (id, kind, content, role, source, importance, tags, created_at, rank) = t;
    let id = Uuid::parse_str(&id).ok()?;
    let kind = MemoryKind::parse(&kind);
    let tags: Vec<String> = serde_json::from_str(&tags).unwrap_or_default();
    let created_at = chrono::DateTime::parse_from_rfc3339(&created_at)
        .map(|d| d.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now());

    // Score: vector distance -> similarity in 0..1; bm25 rank is negative, normalize roughly.
    let score = match is_vector {
        Some(true) => (1.0 - rank as f32).clamp(0.0, 1.0),
        _ => {
            // bm25 returns negative numbers where more negative = more relevant.
            (rank.abs() as f32 / (rank.abs() as f32 + 10.0)).clamp(0.0, 1.0)
        }
    };
    // Blend with importance.
    let score = (score * 0.8 + importance * 0.2).clamp(0.0, 1.0);

    Some(ScoredMemory {
        memory: Memory {
            id,
            kind,
            content,
            role,
            source,
            importance,
            tags,
            created_at,
        },
        score,
    })
}

fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS memories (
            id TEXT PRIMARY KEY,
            kind TEXT NOT NULL,
            content TEXT NOT NULL,
            role TEXT,
            source TEXT,
            importance REAL NOT NULL DEFAULT 0.5,
            tags TEXT NOT NULL DEFAULT '[]',
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_memories_kind ON memories(kind);
        CREATE INDEX IF NOT EXISTS idx_memories_created ON memories(created_at);

        CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
            content,
            content='memories',
            content_rowid='rowid'
        );
        "#,
    )?;

    // vec0 virtual table with an auxiliary TEXT primary key for the memory id.
    // Created separately; ignore "already exists" style errors for robustness.
    let vec_sql = format!(
        "CREATE VIRTUAL TABLE memories_vec USING vec0(id TEXT PRIMARY KEY, embedding float[{EMBEDDING_DIM}])"
    );
    if let Err(e) = conn.execute_batch(&vec_sql) {
        tracing::debug!("memories_vec vec0 table (likely already exists): {e}");
    }
    Ok(())
}
