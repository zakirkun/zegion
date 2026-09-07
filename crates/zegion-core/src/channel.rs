use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::error::Result;

/// A normalized message coming in from any channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomingMessage {
    pub id: Uuid,
    pub channel: String,
    pub user_id: String,
    pub user_name: Option<String>,
    pub text: String,
    pub reply_to: Option<String>,
}

impl IncomingMessage {
    pub fn new(channel: impl Into<String>, user_id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            channel: channel.into(),
            user_id: user_id.into(),
            user_name: None,
            text: text.into(),
            reply_to: None,
        }
    }
}

/// A normalized message going out to a channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutgoingMessage {
    pub channel: String,
    pub user_id: String,
    pub text: String,
    /// If true this is a final answer; if false it is an interim status/stream chunk.
    pub is_final: bool,
}

/// The abstraction every chat surface implements (CLI, Telegram, Discord, ...).
#[async_trait]
pub trait Channel: Send + Sync {
    fn name(&self) -> &str;

    /// Start the channel. Should send incoming user messages into `incoming`,
    /// and return. Implementations typically spawn a background task.
    async fn start(&self, incoming: mpsc::Sender<IncomingMessage>) -> Result<()>;

    /// Send an outgoing message back to a user on this channel.
    async fn send(&self, msg: OutgoingMessage) -> Result<()>;

    /// Ask the user a yes/no approval question in-channel. Default denies.
    async fn ask_approval(&self, _user_id: &str, _prompt: &str) -> Result<bool> {
        Ok(false)
    }

    /// Opaque per-channel id for an in-progress streaming draft (e.g. a message id).
    /// `None` means the channel streams by progressive sends instead of edits.
    async fn send_draft(&self, user_id: &str, initial: &str) -> Result<Option<String>> {
        let _ = (user_id, initial);
        Ok(None)
    }

    async fn update_draft(&self, _user_id: &str, _handle: &str, _text: &str) -> Result<()> {
        Ok(())
    }
}
