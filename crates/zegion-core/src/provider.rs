use std::pin::Pin;

use aisdk::core::capabilities::{StructuredOutputSupport, TextInputSupport, ToolCallSupport};
use aisdk::core::language_model::{LanguageModelOptions, LanguageModelResponse};
use aisdk::core::{DynamicModel, EmbeddingModel, LanguageModel};
use aisdk::providers::{Anthropic, Google, OpenAI, OpenAICompatible, Openrouter};
use futures::Stream;

use crate::config::{Config, ProviderConfig};
use crate::error::{Error, Result};

/// A language model instance resolved from config, erased to a common dynamic type
/// so any configured provider can be used interchangeably in the agent loop.
#[derive(Debug, Clone)]
pub struct AnyLanguageModel {
    inner: AnyLm,
}

#[derive(Debug, Clone)]
enum AnyLm {
    OpenAI(OpenAI<DynamicModel>),
    OpenAICompatible(OpenAICompatible<DynamicModel>),
    Openrouter(Openrouter<DynamicModel>),
    Anthropic(Anthropic<DynamicModel>),
    Google(Google<DynamicModel>),
}

// Capability markers so the wrapper can be used in the request builder.
impl ToolCallSupport for AnyLanguageModel {}
impl TextInputSupport for AnyLanguageModel {}
impl StructuredOutputSupport for AnyLanguageModel {}

#[async_trait::async_trait]
impl LanguageModel for AnyLanguageModel {
    fn name(&self) -> String {
        match &self.inner {
            AnyLm::OpenAI(m) => m.name(),
            AnyLm::OpenAICompatible(m) => m.name(),
            AnyLm::Openrouter(m) => m.name(),
            AnyLm::Anthropic(m) => m.name(),
            AnyLm::Google(m) => m.name(),
        }
    }

    async fn generate_text(
        &mut self,
        options: LanguageModelOptions,
    ) -> aisdk::Result<LanguageModelResponse> {
        match &self.inner {
            AnyLm::OpenAI(m) => m.clone().generate_text(options).await,
            AnyLm::OpenAICompatible(m) => m.clone().generate_text(options).await,
            AnyLm::Openrouter(m) => m.clone().generate_text(options).await,
            AnyLm::Anthropic(m) => m.clone().generate_text(options).await,
            AnyLm::Google(m) => m.clone().generate_text(options).await,
        }
    }

    async fn stream_text(
        &mut self,
        options: LanguageModelOptions,
    ) -> aisdk::Result<
        Pin<
            Box<
                dyn Stream<
                        Item = aisdk::Result<
                            Vec<aisdk::core::language_model::LanguageModelStreamChunk>,
                        >,
                    > + Send,
            >,
        >,
    > {
        match &self.inner {
            AnyLm::OpenAI(m) => m.clone().stream_text(options).await,
            AnyLm::OpenAICompatible(m) => m.clone().stream_text(options).await,
            AnyLm::Openrouter(m) => m.clone().stream_text(options).await,
            AnyLm::Anthropic(m) => m.clone().stream_text(options).await,
            AnyLm::Google(m) => m.clone().stream_text(options).await,
        }
    }
}

/// An embedding model instance resolved from config.
#[derive(Debug, Clone)]
pub struct AnyEmbeddingModel {
    inner: AnyEm,
}

#[derive(Debug, Clone)]
enum AnyEm {
    OpenAI(OpenAI<DynamicModel>),
    OpenAICompatible(OpenAICompatible<DynamicModel>),
    Google(Google<DynamicModel>),
}

#[async_trait::async_trait]
impl EmbeddingModel for AnyEmbeddingModel {
    async fn embed(
        &self,
        options: aisdk::core::embedding_model::EmbeddingModelOptions,
    ) -> aisdk::Result<aisdk::core::embedding_model::EmbeddingModelResponse> {
        match &self.inner {
            AnyEm::OpenAI(m) => m.embed(options).await,
            AnyEm::OpenAICompatible(m) => m.embed(options).await,
            AnyEm::Google(m) => m.embed(options).await,
        }
    }
}

fn provider_cfg<'a>(config: &'a Config, name: &str) -> Result<&'a ProviderConfig> {
    config
        .provider(name)
        .ok_or_else(|| Error::Provider(format!("provider `{name}` not configured")))
}

/// Build a dynamic language model for `provider_name` + `model` from config.
pub fn build_language_model(
    config: &Config,
    provider_name: &str,
    model: &str,
) -> Result<AnyLanguageModel> {
    let name = provider_name.to_ascii_lowercase();
    let inner = match name.as_str() {
        "openai" => {
            let c = provider_cfg(config, "openai")?;
            let mut b = OpenAI::<DynamicModel>::builder().model_name(model);
            if let Some(k) = &c.api_key {
                b = b.api_key(k);
            }
            if let Some(u) = &c.base_url {
                b = b.base_url(u);
            }
            AnyLm::OpenAI(b.build().map_err(|e| Error::Provider(e.to_string()))?)
        }
        "openrouter" => {
            let c = provider_cfg(config, "openrouter")?;
            let mut b = Openrouter::<DynamicModel>::builder().model_name(model);
            if let Some(k) = &c.api_key {
                b = b.api_key(k);
            }
            if let Some(u) = &c.base_url {
                b = b.base_url(u);
            }
            AnyLm::Openrouter(b.build().map_err(|e| Error::Provider(e.to_string()))?)
        }
        "anthropic" => {
            let c = provider_cfg(config, "anthropic")?;
            let mut b = Anthropic::<DynamicModel>::builder().model_name(model);
            if let Some(k) = &c.api_key {
                b = b.api_key(k);
            }
            if let Some(u) = &c.base_url {
                b = b.base_url(u);
            }
            AnyLm::Anthropic(b.build().map_err(|e| Error::Provider(e.to_string()))?)
        }
        "google" | "gemini" => {
            let c = provider_cfg(config, "google")?;
            let mut b = Google::<DynamicModel>::builder().model_name(model);
            if let Some(k) = &c.api_key {
                b = b.api_key(k);
            }
            if let Some(u) = &c.base_url {
                b = b.base_url(u);
            }
            AnyLm::Google(b.build().map_err(|e| Error::Provider(e.to_string()))?)
        }
        // Any OpenAI-compatible endpoint (Ollama, LM Studio, vLLM, Groq, Together, ...)
        other => {
            let c = provider_cfg(config, other)?;
            let mut b = OpenAICompatible::<DynamicModel>::builder()
                .provider_name(other)
                .model_name(model);
            if let Some(k) = &c.api_key {
                b = b.api_key(k);
            }
            if let Some(u) = &c.base_url {
                b = b.base_url(u);
            }
            AnyLm::OpenAICompatible(b.build().map_err(|e| Error::Provider(e.to_string()))?)
        }
    };
    Ok(AnyLanguageModel { inner })
}

/// Build a dynamic embedding model for `provider_name` + `model` from config.
pub fn build_embedding_model(
    config: &Config,
    provider_name: &str,
    model: &str,
) -> Result<AnyEmbeddingModel> {
    let name = provider_name.to_ascii_lowercase();
    let inner = match name.as_str() {
        "openai" => {
            let c = provider_cfg(config, "openai")?;
            let mut b = OpenAI::<DynamicModel>::builder().model_name(model);
            if let Some(k) = &c.api_key {
                b = b.api_key(k);
            }
            if let Some(u) = &c.base_url {
                b = b.base_url(u);
            }
            AnyEm::OpenAI(b.build().map_err(|e| Error::Provider(e.to_string()))?)
        }
        "google" | "gemini" => {
            let c = provider_cfg(config, "google")?;
            let mut b = Google::<DynamicModel>::builder().model_name(model);
            if let Some(k) = &c.api_key {
                b = b.api_key(k);
            }
            if let Some(u) = &c.base_url {
                b = b.base_url(u);
            }
            AnyEm::Google(b.build().map_err(|e| Error::Provider(e.to_string()))?)
        }
        other => {
            let c = provider_cfg(config, other)?;
            let mut b = OpenAICompatible::<DynamicModel>::builder()
                .provider_name(other)
                .model_name(model);
            if let Some(k) = &c.api_key {
                b = b.api_key(k);
            }
            if let Some(u) = &c.base_url {
                b = b.base_url(u);
            }
            AnyEm::OpenAICompatible(b.build().map_err(|e| Error::Provider(e.to_string()))?)
        }
    };
    Ok(AnyEmbeddingModel { inner })
}
