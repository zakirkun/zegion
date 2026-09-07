use std::path::{Path, PathBuf};

use crate::error::Result;

/// A Rhai script plugin: a `.rhai` file exposing a callable `main(args)` function.
#[derive(Debug, Clone)]
pub struct ScriptPlugin {
    pub name: String,
    pub path: PathBuf,
    pub source: String,
}

#[derive(Debug, Clone, Default)]
pub struct PluginRegistry {
    pub scripts: Vec<ScriptPlugin>,
}

impl PluginRegistry {
    /// Load all `.rhai` script plugins from a directory.
    pub async fn load_from_dir(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        let mut scripts = Vec::new();
        let mut entries = match tokio::fs::read_dir(&dir).await {
            Ok(e) => e,
            Err(_) => return Ok(Self::default()),
        };
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("rhai") {
                continue;
            }
            if let Ok(source) = tokio::fs::read_to_string(&path).await {
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("plugin")
                    .to_string();
                scripts.push(ScriptPlugin { name, path, source });
            }
        }
        Ok(Self { scripts })
    }

    pub fn get(&self, name: &str) -> Option<&ScriptPlugin> {
        self.scripts.iter().find(|s| s.name == name)
    }

    pub fn names(&self) -> Vec<String> {
        self.scripts.iter().map(|s| s.name.clone()).collect()
    }

    /// Run a script plugin's `main(input)` function and return its string result.
    pub fn run(&self, name: &str, input: &str) -> Result<String> {
        let plugin = self
            .get(name)
            .ok_or_else(|| crate::error::Error::Plugin(format!("plugin `{name}` not found")))?;
        let engine = rhai::Engine::new();
        let mut scope = rhai::Scope::new();
        scope.push("input", input.to_string());
        let result: rhai::Dynamic = engine
            .eval_with_scope(&mut scope, &plugin.source)
            .map_err(|e| crate::error::Error::Plugin(format!("plugin `{name}` error: {e}")))?;
        Ok(result.to_string())
    }
}
