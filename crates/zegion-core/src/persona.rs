use std::path::Path;

use crate::error::{Error, Result};

/// The agent's persona, loaded from PERSONAL.md and injected into the system prompt.
#[derive(Debug, Clone)]
pub struct Persona {
    pub raw_markdown: String,
    pub name: String,
}

impl Persona {
    pub async fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let raw = tokio::fs::read_to_string(path).await.map_err(|e| {
            Error::Config(format!("cannot read persona file {}: {e}", path.display()))
        })?;
        let name = extract_name(&raw).unwrap_or_else(|| "Zegion".to_string());
        Ok(Self {
            raw_markdown: raw,
            name,
        })
    }

    /// The persona block used inside the system prompt.
    pub fn system_block(&self) -> String {
        format!(
            "You are {name}.\n\n# Persona\n{persona}",
            name = self.name,
            persona = self.raw_markdown.trim()
        )
    }
}

fn extract_name(md: &str) -> Option<String> {
    for line in md.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("- **Name:**") {
            return Some(rest.trim().to_string());
        }
        if let Some(rest) = line.strip_prefix("**Name:**") {
            return Some(rest.trim().to_string());
        }
    }
    None
}
