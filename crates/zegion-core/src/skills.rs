use std::path::{Path, PathBuf};

use crate::error::Result;

/// A skill: a markdown file with instructions the agent can load on demand.
/// Modeled after the "SKILL.md" pattern — a lightweight way to teach the agent
/// reusable procedures without retraining.
#[derive(Debug, Clone)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub body: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Default)]
pub struct SkillRegistry {
    pub skills: Vec<Skill>,
}

impl SkillRegistry {
    /// Load all `*.md` skills from a directory.
    pub async fn load_from_dir(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        let mut skills = Vec::new();
        let mut entries = match tokio::fs::read_dir(&dir).await {
            Ok(e) => e,
            Err(_) => return Ok(Self::default()),
        };
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("md") {
                continue;
            }
            if let Ok(body) = tokio::fs::read_to_string(&path).await {
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("skill")
                    .to_string();
                let description = first_heading_or_line(&body);
                skills.push(Skill {
                    name,
                    description,
                    body,
                    path,
                });
            }
        }
        Ok(Self { skills })
    }

    /// A compact catalog injected into the system prompt so the agent knows
    /// which skills exist and can ask to read them.
    pub fn catalog(&self) -> String {
        if self.skills.is_empty() {
            return String::new();
        }
        let mut s = String::from("\n# Available Skills\n");
        for sk in &self.skills {
            s.push_str(&format!("- {}: {}\n", sk.name, sk.description));
        }
        s
    }

    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.skills.iter().find(|s| s.name == name)
    }
}

fn first_heading_or_line(body: &str) -> String {
    for line in body.lines() {
        let l = line.trim();
        if let Some(h) = l.strip_prefix("# ") {
            return h.trim().to_string();
        }
        if !l.is_empty() {
            return l.chars().take(120).collect();
        }
    }
    String::new()
}
