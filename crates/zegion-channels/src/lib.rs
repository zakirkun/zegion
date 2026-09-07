//! Zegion chat channels: CLI REPL plus external surfaces (Telegram, ...).

pub mod cli;
pub mod discord;
pub mod slack;
pub mod telegram;
pub mod whatsapp;

pub use cli::CliChannel;
pub use discord::DiscordChannel;
pub use slack::SlackChannel;
pub use telegram::TelegramChannel;
pub use whatsapp::WhatsAppChannel;
