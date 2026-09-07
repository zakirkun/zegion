use async_trait::async_trait;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use tokio::sync::mpsc;
use zegion_core::channel::{Channel, IncomingMessage, OutgoingMessage};
use zegion_core::error::{Error, Result};

/// WhatsApp channel using the Meta Cloud API. Receives messages via a webhook
/// (requires a public URL, or a tunnel) and sends via the Graph API.
pub struct WhatsAppChannel {
    phone_number_id: String,
    access_token: String,
    verify_token: String,
    client: reqwest::Client,
}

#[derive(Clone)]
struct WebhookState {
    verify_token: String,
    incoming: mpsc::Sender<IncomingMessage>,
}

impl WhatsAppChannel {
    pub fn new(
        phone_number_id: impl Into<String>,
        access_token: impl Into<String>,
        verify_token: impl Into<String>,
    ) -> Self {
        Self {
            phone_number_id: phone_number_id.into(),
            access_token: access_token.into(),
            verify_token: verify_token.into(),
            client: reqwest::Client::new(),
        }
    }

    async fn send_message(&self, to: &str, text: &str) -> Result<()> {
        let url = format!(
            "https://graph.facebook.com/v20.0/{}/messages",
            self.phone_number_id
        );
        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.access_token)
            .json(&serde_json::json!({
                "messaging_product": "whatsapp",
                "to": to,
                "type": "text",
                "text": { "body": text }
            }))
            .send()
            .await
            .map_err(|e| Error::Channel(e.to_string()))?;
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(Error::Channel(format!("whatsapp send failed: {body}")));
        }
        Ok(())
    }
}

#[async_trait]
impl Channel for WhatsAppChannel {
    fn name(&self) -> &str {
        "whatsapp"
    }

    async fn start(&self, incoming: mpsc::Sender<IncomingMessage>) -> Result<()> {
        let state = WebhookState {
            verify_token: self.verify_token.clone(),
            incoming,
        };
        let app = Router::new()
            .route("/webhook/whatsapp", get(verify).post(receive))
            .with_state(state);
        let bind =
            std::env::var("ZEGION_WHATSAPP_BIND").unwrap_or_else(|_| "127.0.0.1:8790".into());
        let listener = tokio::net::TcpListener::bind(&bind).await?;
        tracing::info!("whatsapp webhook listening on http://{bind}/webhook/whatsapp");
        tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app).await {
                tracing::error!("whatsapp webhook error: {e}");
            }
        });
        Ok(())
    }

    async fn send(&self, msg: OutgoingMessage) -> Result<()> {
        for chunk in split_message(&msg.text, 4000) {
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

#[derive(Debug, Deserialize)]
struct VerifyQuery {
    #[serde(rename = "hub.mode")]
    mode: Option<String>,
    #[serde(rename = "hub.verify_token")]
    token: Option<String>,
    #[serde(rename = "hub.challenge")]
    challenge: Option<String>,
}

async fn verify(
    State(state): State<WebhookState>,
    Query(q): Query<VerifyQuery>,
) -> impl IntoResponse {
    if q.mode.as_deref() == Some("subscribe")
        && q.token.as_deref() == Some(state.verify_token.as_str())
    {
        (StatusCode::OK, q.challenge.unwrap_or_default()).into_response()
    } else {
        StatusCode::FORBIDDEN.into_response()
    }
}

async fn receive(
    State(state): State<WebhookState>,
    Json(body): Json<serde_json::Value>,
) -> StatusCode {
    for entry in body["entry"].as_array().into_iter().flatten() {
        for change in entry["changes"].as_array().into_iter().flatten() {
            let value = &change["value"];
            for message in value["messages"].as_array().into_iter().flatten() {
                if message["type"].as_str() != Some("text") {
                    continue;
                }
                let from = message["from"].as_str().unwrap_or("").to_string();
                let text = message["text"]["body"]
                    .as_str()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if from.is_empty() || text.is_empty() {
                    continue;
                }
                let mut msg = IncomingMessage::new("whatsapp", from.clone(), text);
                msg.reply_to = Some(from);
                let _ = state.incoming.send(msg).await;
            }
        }
    }
    StatusCode::OK
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
