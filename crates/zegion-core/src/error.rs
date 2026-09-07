use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("config error: {0}")]
    Config(String),

    #[error("provider error: {0}")]
    Provider(String),

    #[error("memory error: {0}")]
    Memory(String),

    #[error("tool error: {0}")]
    Tool(String),

    #[error("channel error: {0}")]
    Channel(String),

    #[error("plugin error: {0}")]
    Plugin(String),

    #[error("skill error: {0}")]
    Skill(String),

    #[error("gateway error: {0}")]
    Gateway(String),

    #[error("approval required for tool `{0}`")]
    ApprovalRequired(String),

    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Serde(#[from] serde_json::Error),
}
