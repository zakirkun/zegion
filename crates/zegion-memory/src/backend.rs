//! Extensible memory backend abstraction.
//!
//! `MemoryBackend` is the storage contract: any store that can persist memories,
//! attach embeddings, and serve keyword/vector recall. `SlidingWindow` is the
//! conversation policy that keeps the rolling context bounded regardless of backend.

use async_trait::async_trait;
use uuid::Uuid;

use crate::types::{Memory, MemoryKind, ScoredMemory};

/// Storage contract every memory backend implements.
#[async_trait]
pub trait MemoryBackend: Send + Sync {
    async fn insert(&self, memory: &Memory) -> anyhow::Result<()>;
    async fn set_embedding(&self, id: Uuid, embedding: Vec<f32>) -> anyhow::Result<()>;
    async fn recall_keyword(&self, query: &str, limit: usize) -> anyhow::Result<Vec<ScoredMemory>>;
    async fn recall_vector(
        &self,
        query_embedding: Vec<f32>,
        limit: usize,
    ) -> anyhow::Result<Vec<ScoredMemory>>;
    async fn recent_episodic(&self, limit: usize) -> anyhow::Result<Vec<Memory>>;
    async fn clear_episodic(&self) -> anyhow::Result<usize>;
    async fn count(&self, kind: MemoryKind) -> anyhow::Result<usize>;
}

/// A bounded rolling conversation window. Independent of storage — the agent keeps
/// this in-memory and the backend persists what it needs for long-term recall.
#[derive(Debug, Clone)]
pub struct SlidingWindow {
    max_items: usize,
    items: std::collections::VecDeque<(String, String)>,
}

impl SlidingWindow {
    pub fn new(max_items: usize) -> Self {
        Self {
            max_items: max_items.max(2),
            items: std::collections::VecDeque::new(),
        }
    }

    pub fn push(&mut self, role: impl Into<String>, content: impl Into<String>) {
        if self.items.len() >= self.max_items {
            self.items.pop_front();
        }
        self.items.push_back((role.into(), content.into()));
    }

    pub fn items(&self) -> impl Iterator<Item = &(String, String)> {
        self.items.iter()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }
}
