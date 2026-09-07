use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::{broadcast, RwLock};

use crate::error::{Error, Result};

/// A message on the bus. `payload` is the typed body; the topic routes it.
#[derive(Debug, Clone)]
pub struct Envelope {
    pub topic: String,
    pub from: String,
    pub payload: Arc<dyn Any + Send + Sync>,
}

impl Envelope {
    pub fn new<T: Any + Send + Sync>(topic: impl Into<String>, from: impl Into<String>, payload: T) -> Self {
        Self {
            topic: topic.into(),
            from: from.into(),
            payload: Arc::new(payload),
        }
    }

    /// Downcast the payload to its concrete type.
    pub fn downcast<T: Any + Send + Sync>(&self) -> Option<&T> {
        self.payload.downcast_ref::<T>()
    }
}

/// Typed publish/subscribe bus. Subscribers filter by topic prefix.
#[derive(Clone)]
pub struct AgentBus {
    tx: broadcast::Sender<Envelope>,
}

impl Default for AgentBus {
    fn default() -> Self {
        Self::new(256)
    }
}

impl AgentBus {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity.max(8));
        Self { tx }
    }

    /// Publish a typed message under a topic.
    pub fn publish<T: Any + Send + Sync>(&self, topic: &str, from: &str, payload: T) {
        let _ = self.tx.send(Envelope::new(topic, from, payload));
    }

    /// Subscribe to all envelopes; callers filter by topic.
    pub fn subscribe(&self) -> broadcast::Receiver<Envelope> {
        self.tx.subscribe()
    }

    /// Subscribe and receive only envelopes whose topic starts with `prefix`.
    pub async fn recv_topic(
        mut rx: broadcast::Receiver<Envelope>,
        prefix: &str,
    ) -> Option<Envelope> {
        let prefix = prefix.to_string();
        loop {
            match rx.recv().await {
                Ok(env) if env.topic.starts_with(&prefix) => return Some(env),
                Ok(_) => continue,
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    }
}

/// A managed agent that lives inside an `Environment`.
#[async_trait]
pub trait AgentParticipant: Send + Sync {
    fn id(&self) -> &str;
    /// Topics this participant is interested in (prefix-matched).
    fn subscriptions(&self) -> Vec<String>;
    /// Handle one incoming envelope. May publish replies on the bus.
    async fn handle(&self, env: Envelope, bus: AgentBus) -> Result<()>;
}

/// The environment: registers participants, routes the bus to them, and manages
/// their lifecycle.
pub struct Environment {
    bus: AgentBus,
    participants: RwLock<HashMap<String, Arc<dyn AgentParticipant>>>,
}

impl Environment {
    pub fn new(bus: AgentBus) -> Self {
        Self {
            bus,
            participants: RwLock::new(HashMap::new()),
        }
    }

    pub fn bus(&self) -> AgentBus {
        self.bus.clone()
    }

    pub async fn register(&self, participant: Arc<dyn AgentParticipant>) {
        self.participants
            .write()
            .await
            .insert(participant.id().to_string(), participant);
    }

    pub async fn unregister(&self, id: &str) {
        self.participants.write().await.remove(id);
    }

    pub async fn participant_count(&self) -> usize {
        self.participants.read().await.len()
    }

    /// Fan the bus out to all subscribed participants. Runs until the bus closes.
    pub async fn run(self: Arc<Self>) {
        let mut rx = self.bus.subscribe();
        loop {
            let env = match rx.recv().await {
                Ok(e) => e,
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            };
            let participants = self.participants.read().await.clone();
            for p in participants.values() {
                let interested = p
                    .subscriptions()
                    .iter()
                    .any(|t| env.topic.starts_with(t.as_str()));
                if !interested || p.id() == env.from {
                    continue;
                }
                let p = p.clone();
                let bus = self.bus.clone();
                let env = env.clone();
                tokio::spawn(async move {
                    if let Err(e) = p.handle(env, bus).await {
                        tracing::warn!("participant `{}` handle error: {e}", p.id());
                    }
                });
            }
        }
    }
}

/// Helper for participants to fetch a typed payload or return an error.
pub fn expect_payload<T: Any + Send + Sync>(env: &Envelope) -> Result<&T> {
    env.downcast::<T>()
        .ok_or_else(|| Error::Gateway("payload type mismatch".into()))
}
