//! Zegion memory: local-first memory store.
//!
//! Backed by SQLite (bundled), `sqlite-vec` for vector (semantic) recall and
//! FTS5 for keyword recall. All memory stays on the user's machine.

pub mod backend;
pub mod store;
pub mod types;

pub use backend::{MemoryBackend, SlidingWindow};
pub use store::{MemoryStore, EMBEDDING_DIM};
pub use types::*;
