use async_trait::async_trait;
use serenity::all::{ChannelId, Context, EditMessage, EventHandler, GatewayIntents, Message};
use serenity::Client;
use tokio::sync::mpsc;
use zegion_core::channel::{Channel, IncomingMessage, OutgoingMessage};
use zegion_core::error::{Error, Result};

/// Discord channel backed by the serenity framework (gateway receive + REST send/edit).
pub struct DiscordChannel {
    token: String,
    http: serenity::http::Http,
}

struct Handler {
    incoming: mpsc::Sender<IncomingMessage>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, _ctx: Context, msg: Message) {
        if msg.author.bot {
            return;
        }
        let text = msg.content.trim();
        if text.is_empty() {
            return;
        }
        let mut im = IncomingMessage::new("discord", msg.author.id.to_string(), text);
        im.user_name = Some(msg.author.name.clone());
        im.reply_to = Some(msg.channel_id.to_string());
        let _ = self.incoming.send(im).await;
    }
}

impl DiscordChannel {
    pub fn new(token: impl Into<String>) -> Self {
        let token = token.into();
        let http = serenity::http::Http::new(&token);
        Self { token, http }
    }

    async fn send_text(&self, channel_id: u64, text: &str) -> Result<()> {
        let id = ChannelId::new(channel_id);
        id.say(&self.http, text)
            .await
            .map_err(|e| Error::Channel(format!("discord send failed: {e}")))?;
        Ok(())
    }
}

#[async_trait]
impl Channel for DiscordChannel {
    fn name(&self) -> &str {
        "discord"
    }

    async fn start(&self, incoming: mpsc::Sender<IncomingMessage>) -> Result<()> {
        let intents = GatewayIntents::GUILD_MESSAGES
            | GatewayIntents::DIRECT_MESSAGES
            | GatewayIntents::MESSAGE_CONTENT;
        let handler = Handler { incoming };
        let mut client = Client::builder(&self.token, intents)
            .event_handler(handler)
            .await
            .map_err(|e| Error::Channel(format!("discord client build failed: {e}")))?;
        tokio::spawn(async move {
            if let Err(e) = client.start().await {
                tracing::error!("discord client stopped: {e}");
            }
        });
        Ok(())
    }

    async fn send(&self, msg: OutgoingMessage) -> Result<()> {
        let channel_id: u64 = msg
            .user_id
            .parse()
            .map_err(|_| Error::Channel(format!("invalid discord channel id `{}`", msg.user_id)))?;
        for chunk in split_message(&msg.text, 1900) {
            self.send_text(channel_id, &chunk).await?;
        }
        Ok(())
    }

    async fn send_draft(&self, user_id: &str, initial: &str) -> Result<Option<String>> {
        let channel_id: u64 = user_id
            .parse()
            .map_err(|_| Error::Channel(format!("invalid discord channel id `{user_id}`")))?;
        let msg = ChannelId::new(channel_id)
            .say(&self.http, initial)
            .await
            .map_err(|e| Error::Channel(format!("discord draft failed: {e}")))?;
        Ok(Some(msg.id.to_string()))
    }

    async fn update_draft(&self, user_id: &str, handle: &str, text: &str) -> Result<()> {
        let channel_id: u64 = user_id
            .parse()
            .map_err(|_| Error::Channel(format!("invalid discord channel id `{user_id}`")))?;
        let message_id: u64 = handle
            .parse()
            .map_err(|_| Error::Channel(format!("invalid discord message id `{handle}`")))?;
        let trimmed: String = text.chars().take(1900).collect();
        let content = if trimmed.is_empty() {
            "…".to_string()
        } else {
            trimmed
        };
        ChannelId::new(channel_id)
            .edit_message(
                &self.http,
                serenity::all::MessageId::new(message_id),
                EditMessage::new().content(content),
            )
            .await
            .map_err(|e| Error::Channel(format!("discord edit failed: {e}")))?;
        Ok(())
    }

    async fn ask_approval(&self, user_id: &str, prompt: &str) -> Result<bool> {
        if let Ok(channel_id) = user_id.parse::<u64>() {
            let _ = self
                .send_text(channel_id, &format!("{prompt}\n\nReply yes or no."))
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
