use async_trait::async_trait;
use futures::{SinkExt, StreamExt};
use serde_json::json;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};
use zegion_core::channel::{Channel, IncomingMessage, OutgoingMessage};
use zegion_core::error::{Error, Result};

/// Slack channel using Socket Mode (no public URL needed) for receiving and
/// `chat.postMessage` for sending. Requires a bot token and an app-level token.
pub struct SlackChannel {
    bot_token: String,
    app_token: String,
    client: reqwest::Client,
}

impl SlackChannel {
    pub fn new(bot_token: impl Into<String>, app_token: impl Into<String>) -> Self {
        Self {
            bot_token: bot_token.into(),
            app_token: app_token.into(),
            client: reqwest::Client::new(),
        }
    }

    async fn socket_mode_url(&self) -> Result<String> {
        let resp: serde_json::Value = self
            .client
            .post("https://slack.com/api/apps.connections.open")
            .header("Authorization", format!("Bearer {}", self.app_token))
            .send()
            .await
            .map_err(|e| Error::Channel(e.to_string()))?
            .json()
            .await
            .map_err(|e| Error::Channel(e.to_string()))?;
        resp["url"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| Error::Channel(format!("slack connections.open failed: {resp}")))
    }

    async fn send_message(&self, channel: &str, text: &str) -> Result<()> {
        let resp: serde_json::Value = self
            .client
            .post("https://slack.com/api/chat.postMessage")
            .header("Authorization", format!("Bearer {}", self.bot_token))
            .json(&json!({ "channel": channel, "text": text }))
            .send()
            .await
            .map_err(|e| Error::Channel(e.to_string()))?
            .json()
            .await
            .map_err(|e| Error::Channel(e.to_string()))?;
        if resp["ok"].as_bool() != Some(true) {
            return Err(Error::Channel(format!("slack postMessage failed: {resp}")));
        }
        Ok(())
    }
}

#[async_trait]
impl Channel for SlackChannel {
    fn name(&self) -> &str {
        "slack"
    }

    async fn start(&self, incoming: mpsc::Sender<IncomingMessage>) -> Result<()> {
        let bot = self.bot_token.clone();
        let app = self.app_token.clone();
        tokio::spawn(async move {
            if let Err(e) = run_socket_mode(bot, app, incoming).await {
                tracing::error!("slack socket mode stopped: {e}");
            }
        });
        Ok(())
    }

    async fn send(&self, msg: OutgoingMessage) -> Result<()> {
        for chunk in split_message(&msg.text, 3500) {
            self.send_message(&msg.user_id, &chunk).await?;
        }
        Ok(())
    }

    async fn ask_approval(&self, user_id: &str, prompt: &str) -> Result<bool> {
        self.send_message(user_id, &format!("{prompt}\n\nReply yes or no."))
            .await?;
        Ok(false)
    }
}

async fn run_socket_mode(
    _bot_token: String,
    app_token: String,
    incoming: mpsc::Sender<IncomingMessage>,
) -> Result<()> {
    let chan = SlackChannel::new(_bot_token, app_token);
    let url = chan.socket_mode_url().await?;
    let (ws, _) = connect_async(&url)
        .await
        .map_err(|e| Error::Channel(format!("slack ws connect failed: {e}")))?;
    let (mut write, mut read) = ws.split();

    while let Some(frame) = read.next().await {
        let text = match frame {
            Ok(WsMessage::Text(t)) => t,
            _ => continue,
        };
        let v: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Acknowledge the envelope so Slack doesn't retry.
        if let Some(id) = v["envelope_id"].as_str() {
            let _ = write
                .send(WsMessage::Text(json!({ "envelope_id": id }).to_string()))
                .await;
        }

        if v["type"].as_str() != Some("events_api") {
            continue;
        }
        let event = &v["payload"]["event"];
        if event["type"].as_str() != Some("message") || event.get("subtype").is_some() {
            continue;
        }
        if event["bot_id"].is_string() {
            continue;
        }
        let text_body = event["text"].as_str().unwrap_or("").trim().to_string();
        if text_body.is_empty() {
            continue;
        }
        let channel = event["channel"].as_str().unwrap_or("").to_string();
        let user = event["user"].as_str().unwrap_or("unknown").to_string();
        let mut msg = IncomingMessage::new("slack", user, text_body);
        msg.reply_to = Some(channel);
        if incoming.send(msg).await.is_err() {
            break;
        }
    }
    Ok(())
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
