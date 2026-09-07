use std::sync::Arc;

use aisdk::core::LanguageModelRequest;
use serde::Deserialize;
use zegion_memory::{Memory, MemoryKind, MemoryStore, EMBEDDING_DIM};

use crate::error::Result;
use crate::provider::AnyEmbeddingModel;

/// Durable facts extracted from raw conversation turns by the worker model.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ExtractedFacts {
    /// Durable, non-obvious facts, preferences, or lessons worth remembering long-term.
    pub facts: Vec<ExtractedFact>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ExtractedFact {
    /// The fact in one concise sentence.
    pub fact: String,
    /// Importance 0.0-1.0.
    pub importance: f32,
}

/// Consolidates raw episodic turns into durable semantic memories using the cheap
/// worker model, then clears the episodic window. Returns how many facts were stored.
pub async fn consolidate<M>(
    memory: &MemoryStore,
    worker: &mut M,
    embedder: Option<&AnyEmbeddingModel>,
    max_turns: usize,
) -> Result<usize>
where
    M: aisdk::core::LanguageModel
        + aisdk::core::capabilities::TextInputSupport
        + aisdk::core::capabilities::StructuredOutputSupport,
{
    let episodic = memory.recent_episodic(max_turns).await?;
    if episodic.is_empty() {
        return Ok(0);
    }

    let transcript = episodic
        .iter()
        .map(|m| {
            format!(
                "{}: {}",
                m.role.clone().unwrap_or_else(|| "unknown".into()),
                m.content
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let system = "You extract durable, long-term facts from a conversation transcript. \
                  Only capture non-obvious, reusable facts about the user, their \
                  preferences, goals, environment, and important events. Ignore small \
                  talk and one-off details."
        .to_string();

    let mut req = LanguageModelRequest::builder()
        .model(worker.clone())
        .system(system)
        .prompt(format!("Transcript:\n{transcript}"))
        .schema::<ExtractedFacts>()
        .build();

    let response = req
        .generate_text()
        .await
        .map_err(|e| crate::error::Error::Provider(e.to_string()))?;

    let extracted: ExtractedFacts = response
        .into_schema()
        .map_err(|e| crate::error::Error::Memory(format!("fact parse failed: {e}")))?;

    let mut stored = 0;
    for f in extracted.facts {
        let fact = f.fact.trim();
        if fact.is_empty() {
            continue;
        }
        let mut m = Memory::new(MemoryKind::Semantic, fact);
        m.importance = f.importance.clamp(0.0, 1.0);
        m.source = Some("consolidation".into());
        memory.insert(&m).await?;
        if let Some(emb) = embedder {
            if let Ok(v) = embed_one(emb, fact).await {
                memory.set_embedding(m.id, v).await.ok();
            }
        }
        stored += 1;
    }

    // Reflect: produce a single higher-order summary of what was learned.
    if stored > 0 {
        reflect(memory, worker, embedder, &transcript).await.ok();
    }

    memory.clear_episodic().await?;
    Ok(stored)
}

/// Generate a reflection (a synthesized lesson/summary) from the transcript.
async fn reflect<M>(
    memory: &MemoryStore,
    worker: &mut M,
    embedder: Option<&AnyEmbeddingModel>,
    transcript: &str,
) -> Result<()>
where
    M: aisdk::core::LanguageModel
        + aisdk::core::capabilities::TextInputSupport
        + aisdk::core::capabilities::StructuredOutputSupport,
{
    let system = "You write a short reflection (2-4 sentences) summarizing the most \
                  important theme, preference, or lesson from a conversation. Be concrete."
        .to_string();

    let mut req = LanguageModelRequest::builder()
        .model(worker.clone())
        .system(system)
        .prompt(format!("Transcript:\n{transcript}"))
        .build();

    let response = req
        .generate_text()
        .await
        .map_err(|e| crate::error::Error::Provider(e.to_string()))?;

    if let Some(text) = response.text() {
        let text = text.trim();
        if !text.is_empty() {
            let mut m = Memory::new(MemoryKind::Reflection, text);
            m.importance = 0.6;
            m.source = Some("reflection".into());
            memory.insert(&m).await?;
            if let Some(emb) = embedder {
                if let Ok(v) = embed_one(emb, text).await {
                    memory.set_embedding(m.id, v).await.ok();
                }
            }
        }
    }
    Ok(())
}

async fn embed_one(emb: &AnyEmbeddingModel, text: &str) -> Result<Vec<f32>> {
    use aisdk::core::embedding_model::EmbeddingModelOptions;
    use aisdk::core::EmbeddingModel;
    let res = emb
        .embed(EmbeddingModelOptions {
            input: vec![text.to_string()],
            dimensions: Some(EMBEDDING_DIM),
        })
        .await
        .map_err(|e| crate::error::Error::Memory(e.to_string()))?;
    res.into_iter()
        .next()
        .ok_or_else(|| crate::error::Error::Memory("empty embedding".into()))
}

/// Spawn a background consolidation loop that runs every `interval`.
pub fn spawn_consolidation_loop(
    agent: Arc<tokio::sync::RwLock<crate::agent::Agent>>,
    interval: std::time::Duration,
    threshold: usize,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        loop {
            ticker.tick().await;
            let mut guard = agent.write().await;
            if let Err(e) = guard.consolidate_memory(threshold).await {
                tracing::warn!("memory consolidation failed: {e}");
            }
        }
    })
}
