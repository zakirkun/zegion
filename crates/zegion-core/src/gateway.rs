use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc};
use tokio_stream::wrappers::UnboundedReceiverStream;
use tower_http::cors::CorsLayer;

use crate::channel::{IncomingMessage, OutgoingMessage};
use crate::error::Result;

#[derive(Clone)]
pub struct GatewayState {
    pub incoming: mpsc::UnboundedSender<IncomingMessage>,
    pub broadcast_tx: broadcast::Sender<OutgoingMessage>,
}

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub user_id: Option<String>,
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct ChatAck {
    pub id: String,
    pub status: String,
}

pub fn router(state: GatewayState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/chat", post(chat))
        .route("/v1/events", get(events))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn health() -> &'static str {
    "ok"
}

async fn chat(State(state): State<GatewayState>, Json(req): Json<ChatRequest>) -> Json<ChatAck> {
    let msg = IncomingMessage::new(
        "gateway",
        req.user_id.unwrap_or_else(|| "http".into()),
        req.text,
    );
    let id = msg.id.to_string();
    let _ = state.incoming.send(msg);
    Json(ChatAck {
        id,
        status: "queued".to_string(),
    })
}

async fn events(
    State(state): State<GatewayState>,
) -> Sse<UnboundedReceiverStream<std::result::Result<Event, std::convert::Infallible>>> {
    let (tx, rx) = mpsc::unbounded_channel();
    let mut brx = state.broadcast_tx.subscribe();
    tokio::spawn(async move {
        while let Ok(msg) = brx.recv().await {
            let data = serde_json::to_string(&msg).unwrap_or_default();
            if tx.send(Ok(Event::default().data(data))).is_err() {
                break;
            }
        }
    });
    Sse::new(UnboundedReceiverStream::new(rx)).keep_alive(KeepAlive::default())
}

pub async fn serve(
    bind: &str,
    incoming: mpsc::UnboundedSender<IncomingMessage>,
) -> Result<broadcast::Sender<OutgoingMessage>> {
    let (broadcast_tx, _rx) = broadcast::channel(256);
    let state = GatewayState {
        incoming,
        broadcast_tx: broadcast_tx.clone(),
    };
    let app = router(state);
    let listener = tokio::net::TcpListener::bind(bind).await?;
    tracing::info!("zegion gateway listening on http://{bind}");
    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("gateway error: {e}");
        }
    });
    Ok(broadcast_tx)
}
