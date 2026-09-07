# Memory

Zegion's memory is **local-first**: a SQLite database (bundled, no server) with
`sqlite-vec` for vector recall and FTS5 for keyword recall. Nothing leaves the machine.

## Layers

| layer       | kind        | lifetime  | purpose                                   |
| ----------- | ----------- | --------- | ----------------------------------------- |
| working     | sliding window | session | bounded rolling conversation context      |
| episodic    | `episodic`  | short     | raw conversation turns, before consolidation |
| semantic    | `semantic`  | long      | durable facts, preferences, lessons       |
| reflection  | `reflection`| long      | synthesized higher-order summaries        |

## How a turn uses memory

1. **Recall** — the agent runs keyword (FTS5) and vector (sqlite-vec) recall for the
   current user text, blends scores with importance, and injects the top matches
   (above `recall_min_score`) into the system prompt.
2. **Store** — the turn's user/assistant text is written to episodic memory and embedded
   for later semantic recall.
3. **Consolidate** — a background worker (every 5 min, once episodic passes
   `raw_turn_window`) uses the cheap worker model to distill episodic turns into semantic
   memories and a reflection, then clears the episodic window.

## Explicit control

- `zegion learn "fact"` — write a durable fact to semantic memory.
- `zegion recall "query"` — search memory from the CLI.
- `memory_store` / `memory_recall` tools — the agent manages memory itself.

## Extending: custom backends

`MemoryBackend` (`zegion-memory/src/backend.rs`) is the storage contract. Implement it to
back memory with Postgres, LanceDB, a remote store, etc. The SQLite store is the default
implementation; the agent talks to the trait, not the concrete store.

```rust
#[async_trait]
pub trait MemoryBackend: Send + Sync {
    async fn insert(&self, memory: &Memory) -> Result<()>;
    async fn set_embedding(&self, id: Uuid, embedding: Vec<f32>) -> Result<()>;
    async fn recall_keyword(&self, query: &str, limit: usize) -> Result<Vec<ScoredMemory>>;
    async fn recall_vector(&self, query_embedding: Vec<f32>, limit: usize) -> Result<Vec<ScoredMemory>>;
    async fn recent_episodic(&self, limit: usize) -> Result<Vec<Memory>>;
    async fn clear_episodic(&self) -> Result<usize>;
    async fn count(&self, kind: MemoryKind) -> Result<usize>;
}
```

## Tuning

See `[memory]` in [configuration.md](configuration.md): `recall_top_k`,
`recall_min_score`, `raw_turn_window`. Embeddings use a 384-dimension vector
(`text-embedding-3-small` with `dimensions=384` by default).
