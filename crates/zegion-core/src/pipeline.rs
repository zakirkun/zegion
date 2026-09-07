use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::{Duration, Instant};

use aisdk::core::language_model::{LanguageModelOptions, LanguageModelResponse};
use aisdk::core::LanguageModel;
use parking_lot::Mutex;

use crate::guardrails::GuardrailSet;

/// Composable LLM optimization pipeline. Each stage wraps the next, adding retry,
/// caching, and guardrail enforcement around a base language-model call.
#[derive(Clone)]
pub struct Pipeline<M: LanguageModel> {
    model: M,
    retry: Option<RetryPolicy>,
    cache: Option<Arc<ResponseCache>>,
    guardrails: Option<Arc<GuardrailSet>>,
}

impl<M: LanguageModel> std::fmt::Debug for Pipeline<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pipeline")
            .field("model", &self.model.name())
            .field("retry", &self.retry.is_some())
            .field("cache", &self.cache.is_some())
            .field("guardrails", &self.guardrails.is_some())
            .finish()
    }
}

impl<M: LanguageModel> Pipeline<M> {
    pub fn new(model: M) -> Self {
        Self {
            model,
            retry: None,
            cache: None,
            guardrails: None,
        }
    }

    pub fn with_retry(mut self, policy: RetryPolicy) -> Self {
        self.retry = Some(policy);
        self
    }

    pub fn with_cache(mut self, cache: Arc<ResponseCache>) -> Self {
        self.cache = Some(cache);
        self
    }

    pub fn with_guardrails(mut self, rails: Arc<GuardrailSet>) -> Self {
        self.guardrails = Some(rails);
        self
    }

    pub async fn generate_optimized(&self, options: LanguageModelOptions) -> aisdk::Result<LanguageModelResponse> {
        let fingerprint = fingerprint(&options);

        if let (Some(cache), Some(key)) = (&self.cache, fingerprint) {
            if let Some(hit) = cache.get(key) {
                tracing::debug!("llm cache hit");
                return Ok(hit);
            }
        }

        if let Some(rails) = &self.guardrails {
            if let Some(prompt) = last_user_text(&options) {
                rails
                    .check_input(&prompt)
                    .map_err(|e| aisdk::Error::Other(e.to_string()))?;
            }
        }

        let attempts = self.retry.map(|r| r.max_attempts).unwrap_or(1).max(1);
        let mut delay = self.retry.map(|r| r.initial_backoff).unwrap_or(Duration::ZERO);
        let mut last_err = None;
        let mut response = None;
        for attempt in 1..=attempts {
            let mut model = self.model.clone();
            match model.generate_text(options.clone()).await {
                Ok(resp) => {
                    response = Some(resp);
                    break;
                }
                Err(e) => {
                    last_err = Some(e);
                    if attempt < attempts {
                        tokio::time::sleep(delay).await;
                        delay = delay.saturating_mul(2);
                    }
                }
            }
        }

        let response = match response {
            Some(r) => r,
            None => return Err(last_err.unwrap_or_else(|| aisdk::Error::Other("llm call failed".into()))),
        };

        if let Some(rails) = &self.guardrails {
            if let Some(text) = response_text(&response) {
                rails
                    .check_output(&text)
                    .map_err(|e| aisdk::Error::Other(e.to_string()))?;
            }
        }

        if let (Some(cache), Some(key)) = (&self.cache, fingerprint) {
            cache.put(key, response.clone());
        }

        Ok(response)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub initial_backoff: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(300),
        }
    }
}

struct CacheEntry {
    response: LanguageModelResponse,
    at: Instant,
}

/// A small in-memory TTL cache for identical requests.
pub struct ResponseCache {
    ttl: Duration,
    cap: usize,
    map: Mutex<std::collections::HashMap<u64, CacheEntry>>,
}

impl ResponseCache {
    pub fn new(ttl: Duration, cap: usize) -> Self {
        Self {
            ttl,
            cap: cap.max(1),
            map: Mutex::new(std::collections::HashMap::new()),
        }
    }

    fn get(&self, key: u64) -> Option<LanguageModelResponse> {
        let mut map = self.map.lock();
        if let Some(entry) = map.get(&key) {
            if entry.at.elapsed() < self.ttl {
                return Some(entry.response.clone());
            }
            map.remove(&key);
        }
        None
    }

    fn put(&self, key: u64, response: LanguageModelResponse) {
        let mut map = self.map.lock();
        if map.len() >= self.cap {
            if let Some(oldest) = map.iter().max_by_key(|(_, e)| e.at.elapsed()).map(|(k, _)| *k) {
                map.remove(&oldest);
            }
        }
        map.insert(key, CacheEntry { response, at: Instant::now() });
    }
}

fn fingerprint(options: &LanguageModelOptions) -> Option<u64> {
    let text = last_user_text(options)?;
    let mut h = DefaultHasher::new();
    options.system.hash(&mut h);
    text.hash(&mut h);
    Some(h.finish())
}

fn last_user_text(options: &LanguageModelOptions) -> Option<String> {
    options
        .messages()
        .iter()
        .rev()
        .find_map(|m| match m {
            aisdk::core::Message::User(u) => Some(u.content.clone()),
            _ => None,
        })
}

fn response_text(response: &LanguageModelResponse) -> Option<String> {
    response.contents.iter().find_map(|c| match c {
        aisdk::core::language_model::LanguageModelResponseContentType::Text(t) => Some(t.clone()),
        _ => None,
    })
}

// Capability passthrough: the pipeline supports whatever the wrapped model supports,
// so it can be used as a drop-in model in the request builder.
impl<M: LanguageModel> aisdk::core::capabilities::TextInputSupport for Pipeline<M> {}
impl<M: LanguageModel> aisdk::core::capabilities::ToolCallSupport for Pipeline<M> {}
impl<M: LanguageModel> aisdk::core::capabilities::StructuredOutputSupport for Pipeline<M> {}

#[async_trait::async_trait]
impl<M: LanguageModel> LanguageModel for Pipeline<M> {
    fn name(&self) -> String {
        format!("pipeline({})", self.model.name())
    }

    async fn generate_text(
        &mut self,
        options: LanguageModelOptions,
    ) -> aisdk::Result<LanguageModelResponse> {
        self.generate_optimized(options).await
    }

    async fn stream_text(
        &mut self,
        options: LanguageModelOptions,
    ) -> aisdk::Result<
        std::pin::Pin<
            Box<
                dyn futures::Stream<
                        Item = aisdk::Result<
                            Vec<aisdk::core::language_model::LanguageModelStreamChunk>,
                        >,
                    > + Send,
            >,
        >,
    > {
        self.model.stream_text(options).await
    }
}
