use async_trait::async_trait;
use teloxide::prelude::*;
use futures::StreamExt;
use teloxide::types::{ChatId, UpdateKind};
use teloxide::update_listeners::{polling_default, AsUpdateStream};
use tokio::sync::mpsc;
use zegion_core::channel::{Channel, IncomingMessage, OutgoingMessage};
use zegion_core::error::{Error, Result};

/// Telegram channel backed by the teloxide framework (long-poll receive + send/edit).
pub struct TelegramChannel {
    bot: Bot,
}

impl TelegramChannel {
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            bot: Bot::new(token.into()),
        }
    }

    fn chat_id(user_id: &str) -> Result<ChatId> {
        let id: i64 = user_id
            .parse()
            .map_err(|_| Error::Channel(format!("invalid telegram chat id `{user_id}`")))?;
        Ok(ChatId(id))
    }
}

#[async_trait]
impl Channel for TelegramChannel {
    fn name(&self) -> &str {
        "telegram"
    }

    async fn start(&self, incoming: mpsc::Sender<IncomingMessage>) -> Result<()> {
        let bot = self.bot.clone();
        tokio::spawn(async move {
            let mut listener = polling_default(bot.clone()).await;
            let stream = listener.as_stream();
            tokio::pin!(stream);
            while let Some(update) = stream.next().await {
                let update = match update {
                    Ok(u) => u,
                    Err(e) => {
                        tracing::warn!("telegram poll error: {e}");
                        continue;
                    }
                };
                if let UpdateKind::Message(msg) = update.kind {
                    let text = match msg.text() {
                        Some(t) if !t.trim().is_empty() => t.trim().to_string(),
                        _ => continue,
                    };
                    let (user_id, user_name) = match &msg.from {
                        Some(u) => (u.id.0.to_string(), u.username.clone().or(Some(u.first_name.clone()))),
                        None => (msg.chat.id.0.to_string(), None),
                    };
                    let mut im = IncomingMessage::new("telegram", user_id, text);
                    im.user_name = user_name;
                    im.reply_to = Some(msg.chat.id.0.to_string());
                    if incoming.send(im).await.is_err() {
                        return;
                    }
                }
            }
        });
        Ok(())
    }

    async fn send(&self, msg: OutgoingMessage) -> Result<()> {
        let chat = Self::chat_id(&msg.user_id)?;
        for chunk in split_message(&msg.text, 4000) {
            self.bot
                .send_message(chat, chunk)
                .await
                .map_err(|e| Error::Channel(format!("telegram send failed: {e}")))?;
        }
        Ok(())
    }

    async fn send_draft(&self, user_id: &str, initial: &str) -> Result<Option<String>> {
        let chat = Self::chat_id(user_id)?;
        let msg = self
            .bot
            .send_message(chat, initial)
            .await
            .map_err(|e| Error::Channel(format!("telegram draft failed: {e}")))?;
        Ok(Some(msg.id.0.to_string()))
    }

    async fn update_draft(&self, user_id: &str, handle: &str, text: &str) -> Result<()> {
        let chat = Self::chat_id(user_id)?;
        let message_id: i32 = handle
            .parse()
            .map_err(|_| Error::Channel(format!("invalid telegram message id `{handle}`")))?;
        let trimmed: String = text.chars().take(4000).collect();
        let content = if trimmed.is_empty() { "…".to_string() } else { trimmed };
        // Editing to identical content errors; treat it as a no-op.
        match self
            .bot
            .edit_message_text(chat, teloxide::types::MessageId(message_id), content)
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => {
                let s = e.to_string();
                if s.contains("message is not modified") {
                    Ok(())
                } else {
                    Err(Error::Channel(format!("telegram edit failed: {e}")))
                }
            }
        }
    }

    async fn ask_approval(&self, user_id: &str, prompt: &str) -> Result<bool> {
        if let Ok(chat) = Self::chat_id(user_id) {
            let _ = self
                .bot
                .send_message(chat, format!("{prompt}\n\nReply yes or no."))
                .await;
        }
        Ok(false)
    }
}

fn split_message(text: &str, max: usize) -> Vec<String> {
    if text.len() <= max {
        return vec![text.to_string()];
    }
    let mut out = Vec::new();
    let mut cur = String::new();
    for line in text.lines() {
        if cur.len() + line.len() + 1 > max {
            out.push(std::mem::take(&mut cur));
        }
        cur.push_str(line);
        cur.push('\n');
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}
