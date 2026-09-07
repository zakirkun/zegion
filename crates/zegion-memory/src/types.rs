//! Core memory data types shared across the store and the agent.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Which layer of memory an entry belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryKind {
    /// Raw conversation turns (short window, later consolidated).
    Episodic,
    /// Durable learned facts / preferences (long-term).
    Semantic,
    /// Higher-order summaries and lessons produced by reflection.
    Reflection,
}

impl MemoryKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            MemoryKind::Episodic => "episodic",
            MemoryKind::Semantic => "semantic",
            MemoryKind::Reflection => "reflection",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "semantic" => MemoryKind::Semantic,
            "reflection" => MemoryKind::Reflection,
            _ => MemoryKind::Episodic,
        }
    }
}

/// A single memory record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: Uuid,
    pub kind: MemoryKind,
    /// The actual content the agent will read later.
    pub content: String,
    /// Optional role that produced it (user / assistant / system).
    pub role: Option<String>,
    /// Channel / surface the memory came from (cli, telegram, ...).
    pub source: Option<String>,
    /// Importance 0.0..1.0 used to rank recall.
    pub importance: f32,
    /// Free-form tags for filtering.
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
}

impl Memory {
    pub fn new(kind: MemoryKind, content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            kind,
            content: content.into(),
            role: None,
            source: None,
            importance: 0.5,
            tags: Vec::new(),
            created_at: Utc::now(),
        }
    }
}

/// A memory plus the recall score that produced it.
#[derive(Debug, Clone)]
pub struct ScoredMemory {
    pub memory: Memory,
    /// Higher is more relevant. Combined vector + keyword + importance score.
    pub score: f32,
}

/// Tunables controlling recall and consolidation behavior.
#[derive(Debug, Clone)]
pub struct MemoryConfig {
    pub db_path: String,
    pub recall_top_k: usize,
    pub recall_min_score: f32,
    pub raw_turn_window: usize,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            db_path: "data/zegion.db".into(),
            recall_top_k: 8,
            recall_min_score: 0.35,
            raw_turn_window: 24,
        }
    }
}
