use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use zegion_memory::MemoryConfig;

use crate::error::{Error, Result};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub agent: AgentConfig,
    pub models: ModelsConfig,
    pub providers: HashMap<String, ProviderConfig>,
    pub memory: MemorySection,
    pub security: SecurityConfig,
    pub channels: HashMap<String, ChannelConfig>,
    pub plugins: PluginsConfig,
}

impl Config {
    pub async fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let text = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| Error::Config(format!("cannot read config {}: {e}", path.display())))?;
        let expanded = expand_env(&text);
        let cfg: Config = toml::from_str(&expanded)
            .map_err(|e| Error::Config(format!("invalid config {}: {e}", path.display())))?;
        Ok(cfg)
    }

    pub fn provider(&self, name: &str) -> Option<&ProviderConfig> {
        self.providers.get(name)
    }

    pub fn memory_config(&self) -> MemoryConfig {
        MemoryConfig {
            db_path: self.memory.db_path.clone(),
            recall_top_k: self.memory.recall_top_k,
            recall_min_score: self.memory.recall_min_score,
            raw_turn_window: self.memory.raw_turn_window,
        }
    }

    /// Serialize the current config back to TOML (used by the onboarding wizard).
    pub fn to_toml(&self) -> Result<String> {
        toml::to_string_pretty(self)
            .map_err(|e| Error::Config(format!("failed to serialize config: {e}")))
    }

    pub async fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let text = self.to_toml()?;
        tokio::fs::write(path.as_ref(), text).await?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentConfig {
    pub name: String,
    pub persona_path: String,
    pub max_steps: usize,
    pub temperature: u32,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            name: "Zegion".into(),
            persona_path: "PERSONAL.md".into(),
            max_steps: 12,
            temperature: 40,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelsConfig {
    pub default_provider: String,
    pub default_model: String,
    pub worker_provider: String,
    pub worker_model: String,
    pub embedding_provider: String,
    pub embedding_model: String,
}

impl Default for ModelsConfig {
    fn default() -> Self {
        Self {
            default_provider: "openrouter".into(),
            default_model: "google/gemini-2.0-flash-001".into(),
            worker_provider: "openrouter".into(),
            worker_model: "google/gemini-2.0-flash-lite-001".into(),
            embedding_provider: "openai".into(),
            embedding_model: "text-embedding-3-small".into(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProviderConfig {
    pub api_key: Option<String>,
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MemorySection {
    pub db_path: String,
    pub recall_top_k: usize,
    pub recall_min_score: f32,
    pub raw_turn_window: usize,
}

impl Default for MemorySection {
    fn default() -> Self {
        Self {
            db_path: "data/zegion.db".into(),
            recall_top_k: 8,
            recall_min_score: 0.35,
            raw_turn_window: 24,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SecurityConfig {
    pub supervised: bool,
    pub require_approval: Vec<String>,
    pub daily_spend_cap_usd: f32,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            supervised: true,
            require_approval: vec!["shell".into(), "write_file".into(), "http_post".into()],
            daily_spend_cap_usd: 1.0,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ChannelConfig {
    pub enabled: bool,
    #[serde(flatten)]
    pub extra: HashMap<String, toml::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PluginsConfig {
    pub dir: String,
    pub mcp: Vec<McpServerConfig>,
    /// When true, built-in MCP presets (filesystem, git, sqlite, fetch, memory) are
    /// auto-connected in addition to any explicitly configured servers.
    pub auto_presets: bool,
}

impl Default for PluginsConfig {
    fn default() -> Self {
        Self {
            dir: "plugins".into(),
            mcp: Vec::new(),
            auto_presets: false,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct McpServerConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
}

/// Expand `${VAR}` placeholders using process environment variables.
fn expand_env(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find('}') {
            Some(end) => {
                let var = &after[..end];
                out.push_str(&std::env::var(var).unwrap_or_default());
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out
}
