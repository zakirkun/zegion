//! Zegion core: persona, config, agent loop, skills, tools, plugins, gateway.

pub mod agent;
pub mod channel;
pub mod config;
pub mod error;
pub mod gateway;
pub mod guardrails;
pub mod hooks;
pub mod mcp;
pub mod orchestrator;
pub mod persona;
pub mod pipeline;
pub mod plugins;
pub mod provider;
pub mod react;
pub mod registry;
pub mod sandbox;
pub mod skills;
pub mod tools;
pub mod worker;

pub use agent::Agent;
pub use channel::{Channel, IncomingMessage, OutgoingMessage};
pub use config::Config;
pub use error::{Error, Result};
pub use persona::Persona;
