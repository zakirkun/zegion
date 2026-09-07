use std::sync::Arc;

use crate::channel::Channel;
use crate::config::SecurityConfig;

/// Decides whether a tool call is allowed to run autonomously.
#[derive(Clone)]
pub struct ApprovalGate {
    security: SecurityConfig,
    channel: Option<Arc<dyn Channel>>,
    user_id: String,
}

impl ApprovalGate {
    pub fn new(security: SecurityConfig, channel: Option<Arc<dyn Channel>>, user_id: String) -> Self {
        Self {
            security,
            channel,
            user_id,
        }
    }

    /// Returns true if the tool may proceed without asking.
    pub fn is_auto_allowed(&self, tool_name: &str) -> bool {
        if !self.security.supervised {
            return true;
        }
        !self
            .security
            .require_approval
            .iter()
            .any(|t| t == tool_name)
    }

    /// Ask the user for approval in-channel. Falls back to deny when no channel.
    pub async fn request(&self, tool_name: &str, detail: &str) -> bool {
        if self.is_auto_allowed(tool_name) {
            return true;
        }
        if let Some(ch) = &self.channel {
            let prompt = format!("Zegion wants to run `{tool_name}`: {detail}\nAllow? (yes/no)");
            ch.ask_approval(&self.user_id, &prompt).await.unwrap_or(false)
        } else {
            false
        }
    }
}
