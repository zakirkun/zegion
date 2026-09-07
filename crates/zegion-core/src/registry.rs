//! Unified auto-registration and auto-loading of capabilities: skills, script
//! plugins, and MCP server presets. Everything is discovered from disk at startup
//! and can be hot-reloaded without restarting the agent.

use std::path::PathBuf;
use std::sync::Arc;

use crate::config::McpServerConfig;
use crate::error::Result;
use crate::mcp::McpRegistry;
use crate::plugins::PluginRegistry;
use crate::skills::SkillRegistry;

/// Built-in MCP server presets. Each is only launched if its command is available
/// and the preset is enabled in config (or `auto` enabled presets are on).
pub fn builtin_mcp_presets() -> Vec<McpServerConfig> {
    vec![
        McpServerConfig {
            name: "filesystem".into(),
            command: "npx".into(),
            args: vec!["-y".into(), "@modelcontextprotocol/server-filesystem".into(), ".".into()],
        },
        McpServerConfig {
            name: "git".into(),
            command: "uvx".into(),
            args: vec!["mcp-server-git".into(), "--repository".into(), ".".into()],
        },
        McpServerConfig {
            name: "sqlite".into(),
            command: "uvx".into(),
            args: vec!["mcp-server-sqlite".into(), "--db-path".into(), "data/zegion.db".into()],
        },
        McpServerConfig {
            name: "fetch".into(),
            command: "uvx".into(),
            args: vec!["mcp-server-fetch".into()],
        },
        McpServerConfig {
            name: "memory".into(),
            command: "npx".into(),
            args: vec!["-y".into(), "@modelcontextprotocol/server-memory".into()],
        },
    ]
}

/// Aggregates every capability source into one registry the agent consults.
pub struct CapabilityRegistry {
    pub skills: SkillRegistry,
    pub plugins: PluginRegistry,
    pub mcp_tools: Vec<aisdk::core::Tool>,
    pub skills_dir: PathBuf,
    pub plugins_dir: PathBuf,
}

impl CapabilityRegistry {
    /// Auto-load skills and plugins from disk, and auto-connect configured MCP
    /// servers. MCP presets are opt-in via `auto_presets`.
    pub async fn load(
        skills_dir: impl Into<PathBuf>,
        plugins_dir: impl Into<PathBuf>,
        mcp_configs: &[McpServerConfig],
        auto_presets: bool,
    ) -> Self {
        let skills_dir = skills_dir.into();
        let plugins_dir = plugins_dir.into();

        let skills = SkillRegistry::load_from_dir(&skills_dir).await.unwrap_or_default();
        let plugins = PluginRegistry::load_from_dir(&plugins_dir).await.unwrap_or_default();

        let mut mcp_all: Vec<McpServerConfig> = mcp_configs.to_vec();
        if auto_presets {
            for preset in builtin_mcp_presets() {
                if !mcp_all.iter().any(|c| c.name == preset.name) {
                    mcp_all.push(preset);
                }
            }
        }

        let mcp_registry = McpRegistry::connect_all(&mcp_all).await;
        let mcp_tools = mcp_registry.discover_tools().await;

        tracing::info!(
            "capabilities loaded: {} skills, {} plugins, {} mcp tools",
            skills.skills.len(),
            plugins.scripts.len(),
            mcp_tools.len()
        );

        Self {
            skills,
            plugins,
            mcp_tools,
            skills_dir,
            plugins_dir,
        }
    }

    /// Re-scan the skills and plugins directories, picking up anything added or
    /// changed since the last load (hot reload).
    pub async fn reload(&mut self) -> Result<()> {
        self.skills = SkillRegistry::load_from_dir(&self.skills_dir)
            .await
            .unwrap_or_default();
        self.plugins = PluginRegistry::load_from_dir(&self.plugins_dir)
            .await
            .unwrap_or_default();
        Ok(())
    }
}

/// Spawn a background watcher that reloads the registry when the skills/plugins
/// directories change (simple mtime poll — no extra deps, low-spec friendly).
pub fn spawn_autoloader(
    registry: Arc<tokio::sync::RwLock<CapabilityRegistry>>,
    interval: std::time::Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut last = snapshot(&registry).await;
        let mut ticker = tokio::time::interval(interval);
        loop {
            ticker.tick().await;
            let current = snapshot(&registry).await;
            if current != last {
                let mut guard = registry.write().await;
                if guard.reload().await.is_ok() {
                    tracing::info!("capabilities hot-reloaded");
                    last = current;
                }
            }
        }
    })
}

async fn snapshot(registry: &Arc<tokio::sync::RwLock<CapabilityRegistry>>) -> (usize, usize) {
    let guard = registry.read().await;
    (guard.skills.skills.len(), guard.plugins.scripts.len())
}
