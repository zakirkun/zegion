use std::sync::Arc;

use aisdk::core::{EmbeddingModel, LanguageModelRequest, LanguageModelStreamChunkType, Message};
use futures::StreamExt;
use zegion_memory::{Memory, MemoryKind, MemoryStore, EMBEDDING_DIM};

use crate::config::Config;
use crate::error::Result;
use crate::hooks::ApprovalGate;
use crate::persona::Persona;
use crate::provider::{build_embedding_model, build_language_model, AnyEmbeddingModel, AnyLanguageModel};
use crate::skills::SkillRegistry;
use crate::tools::{build_tools, ToolContext};

/// The Zegion agent. Owns persona, memory, skills, and the model handles, and
/// runs the perceive -> recall -> think -> act -> remember loop.
pub struct Agent {
    pub config: Config,
    pub persona: Persona,
    pub memory: MemoryStore,
    pub skills: SkillRegistry,
    model: crate::pipeline::Pipeline<AnyLanguageModel>,
    worker: crate::pipeline::Pipeline<AnyLanguageModel>,
    embedder: Option<AnyEmbeddingModel>,
    /// Tools discovered from connected MCP servers (wrapped as aisdk tools).
    mcp_tools: Vec<aisdk::core::Tool>,
    /// Bounded rolling conversation for the active session (sliding-window memory).
    window: zegion_memory::SlidingWindow,
}

impl Agent {
    pub async fn new(config: Config) -> Result<Arc<tokio::sync::RwLock<Self>>> {
        let persona = Persona::load(&config.agent.persona_path).await?;
        let memory = MemoryStore::open(config.memory_config()).await?;

        let guardrails = std::sync::Arc::new(crate::guardrails::default_guardrails());
        let cache = std::sync::Arc::new(crate::pipeline::ResponseCache::new(
            std::time::Duration::from_secs(300),
            64,
        ));
        let retry = crate::pipeline::RetryPolicy::default();

        let model = crate::pipeline::Pipeline::new(build_language_model(
            &config,
            &config.models.default_provider,
            &config.models.default_model,
        )?)
        .with_retry(retry)
        .with_cache(cache.clone())
        .with_guardrails(guardrails.clone());

        let worker = crate::pipeline::Pipeline::new(build_language_model(
            &config,
            &config.models.worker_provider,
            &config.models.worker_model,
        )?)
        .with_retry(retry)
        .with_guardrails(guardrails);

        let embedder = build_embedding_model(
            &config,
            &config.models.embedding_provider,
            &config.models.embedding_model,
        )
        .ok();

        // Auto-load skills, plugins, and MCP tools through the unified registry.
        // MCP presets are opt-in; explicit config always wins.
        let caps = crate::registry::CapabilityRegistry::load(
            "skills",
            &config.plugins.dir,
            &config.plugins.mcp,
            config.plugins.auto_presets,
        )
        .await;
        let skills = caps.skills.clone();
        let mcp_tools = caps.mcp_tools.clone();
        if !mcp_tools.is_empty() {
            tracing::info!("discovered {} MCP tool(s)", mcp_tools.len());
        }

        let window_size = config.memory.raw_turn_window * 2;
        Ok(Arc::new(tokio::sync::RwLock::new(Self {
            config,
            persona,
            memory,
            skills,
            model,
            worker,
            embedder,
            mcp_tools,
            window: zegion_memory::SlidingWindow::new(window_size),
        })))
    }

    /// Public accessor for the memory store (used by CLI subcommands).
    pub fn memory(&self) -> &MemoryStore {
        &self.memory
    }

    /// Compose the system prompt from persona + skill catalog + memory guidance.
    fn system_prompt(&self, recalled: &str) -> String {
        let mut s = self.persona.system_block();
        s.push_str(&self.skills.catalog());
        s.push_str(
            "\n# Memory Guidance\nYou have persistent memory. Use recalled memories below. \
             When you learn durable, non-obvious facts about the user, remember them.\n",
        );
        if !recalled.is_empty() {
            s.push_str("\n# Recalled Memories\n");
            s.push_str(recalled);
            s.push('\n');
        }
        s
    }

    /// Recall relevant memories for a user turn and render them as context.
    async fn recall(&self, query: &str) -> String {
        let top_k = self.memory.config().recall_top_k;
        let min_score = self.memory.config().recall_min_score;

        let mut hits = self
            .memory
            .recall_keyword(query, top_k)
            .await
            .unwrap_or_default();

        if let Some(emb) = &self.embedder {
            if let Ok(qv) = self.embed_text(emb, query).await {
                if let Ok(mut vhits) = self.memory.recall_vector(qv, top_k).await {
                    hits.append(&mut vhits);
                }
            }
        }

        // Deduplicate by id, keep best score.
        let mut best: std::collections::HashMap<uuid::Uuid, f32> = Default::default();
        let mut items: Vec<(uuid::Uuid, String, f32)> = Vec::new();
        for h in hits {
            let e = best.entry(h.memory.id).or_insert(h.score);
            if h.score > *e {
                *e = h.score;
            }
            items.push((h.memory.id, h.memory.content.clone(), h.score));
        }
        items.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        items
            .into_iter()
            .filter(|(_, _, s)| *s >= min_score)
            .take(top_k)
            .map(|(_, c, _)| format!("- {c}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    async fn embed_text(&self, emb: &AnyEmbeddingModel, text: &str) -> Result<Vec<f32>> {
        use aisdk::core::embedding_model::EmbeddingModelOptions;
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

    fn record_turn(&mut self, user_text: &str, reply: &str) {
        self.window.push("user", user_text);
        self.window.push("assistant", reply);
    }

    fn window_messages(&self) -> Vec<Message> {
        self.window
            .items()
            .map(|(role, content)| match role.as_str() {
                "assistant" => Message::Assistant(content.clone().into()),
                _ => Message::User(content.clone().into()),
            })
            .collect()
    }

    /// Handle one user turn and produce the assistant's reply text, using the given
    /// approval gate to control sensitive tools.
    pub async fn turn_with_gate(&mut self, user_text: &str, gate: Arc<ApprovalGate>) -> Result<String> {
        let recalled = self.recall(user_text).await;
        let system = self.system_prompt(&recalled);
        let mut messages = self.window_messages();
        messages.push(Message::User(user_text.into()));
        let ctx = ToolContext {
            memory: self.memory.clone(),
            gate,
        };
        let mut tools = build_tools(&ctx);
        tools.extend(self.mcp_tools.iter().cloned());

        let mut builder = LanguageModelRequest::builder()
            .model(self.model.clone())
            .system(system)
            .messages(messages);
        for t in tools {
            builder = builder.with_tool(t);
        }
        let mut req = builder
            .stop_when(aisdk::core::utils::step_count_is(self.config.agent.max_steps))
            .build();

        let response = req
            .generate_text()
            .await
            .map_err(|e| crate::error::Error::Provider(e.to_string()))?;

        let reply = response
            .text()
            .unwrap_or_else(|| "(no response)".to_string());

        self.record_turn(user_text, &reply);
        self.store_turn(user_text, &reply).await.ok();

        Ok(reply)
    }

    /// Streaming variant of `turn_with_gate`. Emits text deltas through `on_delta`
    /// as they arrive, then returns the final reply text.
    pub async fn turn_stream<F>(
        &mut self,
        user_text: &str,
        gate: Arc<ApprovalGate>,
        mut on_delta: F,
    ) -> Result<String>
    where
        F: FnMut(String) + Send,
    {
        let recalled = self.recall(user_text).await;
        let system = self.system_prompt(&recalled);
        let mut messages = self.window_messages();
        messages.push(Message::User(user_text.into()));
        let ctx = ToolContext {
            memory: self.memory.clone(),
            gate,
        };
        let mut tools = build_tools(&ctx);
        tools.extend(self.mcp_tools.iter().cloned());

        let mut builder = LanguageModelRequest::builder()
            .model(self.model.clone())
            .system(system)
            .messages(messages);
        for t in tools {
            builder = builder.with_tool(t);
        }
        let mut req = builder
            .stop_when(aisdk::core::utils::step_count_is(self.config.agent.max_steps))
            .build();

        let response = req
            .stream_text()
            .await
            .map_err(|e| crate::error::Error::Provider(e.to_string()))?;

        let mut response = response;
        while let Some(chunk) = response.stream.next().await {
            if let LanguageModelStreamChunkType::Text(delta) = chunk {
                if !delta.is_empty() {
                    on_delta(delta);
                }
            }
        }

        let reply = response
            .text()
            .await
            .unwrap_or_else(|| "(no response)".to_string());

        self.record_turn(user_text, &reply);
        self.store_turn(user_text, &reply).await.ok();

        Ok(reply)
    }

    /// Convenience turn with no in-channel approval channel (denies sensitive tools).
    pub async fn turn(&mut self, user_text: &str) -> Result<String> {
        let gate = Arc::new(ApprovalGate::new(
            self.config.security.clone(),
            None,
            "local".into(),
        ));
        self.turn_with_gate(user_text, gate).await
    }

    async fn store_turn(&self, user_text: &str, reply: &str) -> Result<()> {
        let mut user_m = Memory::new(MemoryKind::Episodic, user_text);
        user_m.role = Some("user".into());
        self.memory.insert(&user_m).await?;

        let mut asst_m = Memory::new(MemoryKind::Episodic, reply);
        asst_m.role = Some("assistant".into());
        self.memory.insert(&asst_m).await?;

        // Optionally embed for semantic recall.
        if let Some(emb) = &self.embedder {
            if let Ok(v) = self.embed_text(emb, user_text).await {
                self.memory.set_embedding(user_m.id, v).await.ok();
            }
            if let Ok(v) = self.embed_text(emb, reply).await {
                self.memory.set_embedding(asst_m.id, v).await.ok();
            }
        }
        Ok(())
    }

    /// Consolidate raw episodic turns into durable semantic memories + a reflection,
    /// but only if the episodic window has grown past `threshold`.
    pub async fn consolidate_memory(&mut self, threshold: usize) -> Result<usize> {
        let count = self.memory.count(MemoryKind::Episodic).await?;
        if count < threshold {
            return Ok(0);
        }
        let max_turns = self.config.memory.raw_turn_window;
        crate::worker::consolidate(
            &self.memory,
            &mut self.worker,
            self.embedder.as_ref(),
            max_turns,
        )
        .await
    }

    /// Explicitly teach the agent a durable fact (semantic memory).
    pub async fn learn(&self, fact: &str, importance: f32) -> Result<()> {
        let mut m = Memory::new(MemoryKind::Semantic, fact);
        m.importance = importance.clamp(0.0, 1.0);
        self.memory.insert(&m).await?;
        if let Some(emb) = &self.embedder {
            if let Ok(v) = self.embed_text(emb, fact).await {
                self.memory.set_embedding(m.id, v).await.ok();
            }
        }
        Ok(())
    }
}
