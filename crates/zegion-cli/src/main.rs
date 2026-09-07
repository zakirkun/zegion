mod onboard;

use clap::{Parser, Subcommand};
use zegion_channels::CliChannel;
use zegion_core::channel::{Channel, OutgoingMessage};
use zegion_core::{Agent, Config};

#[derive(Parser)]
#[command(name = "zegion", version, about = "Zegion � low-spec autonomous agent")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Path to config file.
    #[arg(short, long, default_value = "zegion.toml")]
    config: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the agent (CLI + enabled channels/gateway).
    Run,
    /// Print current configuration summary.
    Config,
    /// Teach Zegion a durable fact (writes to semantic memory).
    Learn { fact: String },
    /// Search memory.
    Recall { query: String },
    /// Interactive setup wizard — writes a ready-to-use config file.
    Onboard,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "zegion=info".into()),
        )
        .with_writer(std::io::stderr)
        .init();

    print_banner();
    let cli = Cli::parse();

    // Onboarding runs before config load (it creates the config file).
    if matches!(cli.command, Some(Commands::Onboard)) {
        return onboard::run(&cli.config).await;
    }

    // Fall back to example config if the real one is missing, so first run works.
    let cfg_path = if std::path::Path::new(&cli.config).exists() {
        cli.config.clone()
    } else if std::path::Path::new("zegion.example.toml").exists() {
        eprintln!(
            "[zegion] {} not found; using zegion.example.toml",
            cli.config
        );
        "zegion.example.toml".to_string()
    } else {
        cli.config.clone()
    };

    let config = Config::load(&cfg_path).await?;

    match cli.command.unwrap_or(Commands::Run) {
        Commands::Config => {
            println!("{:#?}", config);
        }
        Commands::Learn { fact } => {
            let agent = Agent::new(config).await?;
            let agent = agent.read().await;
            agent
                .learn(&fact, 0.8)
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            println!("learned: {fact}");
        }
        Commands::Recall { query } => {
            let agent = Agent::new(config).await?;
            let agent = agent.read().await;
            let hits = agent.memory().recall_keyword(&query, 10).await?;
            for h in hits {
                println!("[{:.2}] {}", h.score, h.memory.content);
            }
        }
        Commands::Run => {
            run(config).await?;
        }
        Commands::Onboard => unreachable!("handled before config load"),
    }
    Ok(())
}

fn print_banner() {
    if let Ok(font) = figlet_rs::FIGlet::standard() {
        if let Some(figure) = font.convert("Zegion") {
            println!("{figure}");
        }
    }
}

async fn run(config: Config) -> anyhow::Result<()> {
    let agent = Agent::new(config.clone()).await?;

    // Channel: inbound messages from all surfaces.
    let (incoming_tx, mut incoming_rx) =
        tokio::sync::mpsc::channel::<zegion_core::channel::IncomingMessage>(64);

    // Start enabled channels.
    let cli = CliChannel::default();
    // Registry of active channels by name, used both to send replies and to
    // build per-channel approval gates for sensitive tools.
    let mut channels: std::collections::HashMap<String, std::sync::Arc<dyn Channel>> =
        std::collections::HashMap::new();

    if config
        .channels
        .get("cli")
        .map(|c| c.enabled)
        .unwrap_or(true)
    {
        cli.start(incoming_tx.clone()).await?;
        channels.insert("cli".into(), std::sync::Arc::new(CliChannel::default()));
    }

    // Gateway (HTTP/SSE) channel.
    let mut gateway_tx: Option<tokio::sync::broadcast::Sender<OutgoingMessage>> = None;
    if let Some(gw) = config.channels.get("gateway") {
        if gw.enabled {
            let bind = gw
                .extra
                .get("bind")
                .and_then(|v| v.as_str())
                .unwrap_or("127.0.0.1:8787")
                .to_string();
            let (gw_in_tx, gw_in_rx) = tokio::sync::mpsc::unbounded_channel();
            let broadcast = zegion_core::gateway::serve(&bind, gw_in_tx).await?;
            gateway_tx = Some(broadcast);
            let shared = incoming_tx.clone();
            tokio::spawn(async move {
                let mut rx = gw_in_rx;
                while let Some(msg) = rx.recv().await {
                    if shared.send(msg).await.is_err() {
                        break;
                    }
                }
            });
        }
    }

    // Telegram channel (long polling, no public URL needed).
    if let Some(tg) = config.channels.get("telegram") {
        if tg.enabled {
            if let Some(token) = tg.extra.get("bot_token").and_then(|v| v.as_str()) {
                let chan = zegion_channels::TelegramChannel::new(token.to_string());
                chan.start(incoming_tx.clone()).await?;
                channels.insert("telegram".into(), std::sync::Arc::new(chan));
                println!("telegram channel enabled");
            } else {
                eprintln!("telegram enabled but no bot_token configured");
            }
        }
    }

    // Discord channel (gateway websocket).
    if let Some(dc) = config.channels.get("discord") {
        if dc.enabled {
            if let Some(token) = dc.extra.get("bot_token").and_then(|v| v.as_str()) {
                let chan = zegion_channels::DiscordChannel::new(token.to_string());
                chan.start(incoming_tx.clone()).await?;
                channels.insert("discord".into(), std::sync::Arc::new(chan));
                println!("discord channel enabled");
            } else {
                eprintln!("discord enabled but no bot_token configured");
            }
        }
    }

    // Slack channel (socket mode).
    if let Some(sl) = config.channels.get("slack") {
        if sl.enabled {
            let bot = sl.extra.get("bot_token").and_then(|v| v.as_str());
            let app = sl.extra.get("app_token").and_then(|v| v.as_str());
            match (bot, app) {
                (Some(bot), Some(app)) => {
                    let chan = zegion_channels::SlackChannel::new(bot.to_string(), app.to_string());
                    chan.start(incoming_tx.clone()).await?;
                    channels.insert("slack".into(), std::sync::Arc::new(chan));
                    println!("slack channel enabled");
                }
                _ => eprintln!("slack enabled but bot_token/app_token missing"),
            }
        }
    }

    // WhatsApp channel (cloud API webhook).
    if let Some(wa) = config.channels.get("whatsapp") {
        if wa.enabled {
            let pnid = wa.extra.get("phone_number_id").and_then(|v| v.as_str());
            let token = wa.extra.get("access_token").and_then(|v| v.as_str());
            let verify = wa.extra.get("verify_token").and_then(|v| v.as_str());
            match (pnid, token, verify) {
                (Some(pnid), Some(token), Some(verify)) => {
                    let chan = zegion_channels::WhatsAppChannel::new(
                        pnid.to_string(),
                        token.to_string(),
                        verify.to_string(),
                    );
                    chan.start(incoming_tx.clone()).await?;
                    channels.insert("whatsapp".into(), std::sync::Arc::new(chan));
                    println!("whatsapp channel enabled");
                }
                _ => eprintln!(
                    "whatsapp enabled but phone_number_id/access_token/verify_token missing"
                ),
            }
        }
    }

    // Background memory consolidation worker.
    let consolidate_every = std::time::Duration::from_secs(300);
    let threshold = config.memory.raw_turn_window;
    zegion_core::worker::spawn_consolidation_loop(agent.clone(), consolidate_every, threshold);

    println!("zegion is running. Type to chat. Ctrl+C to quit.");

    // Main dispatch loop: route inbound messages to the agent, send replies back.
    while let Some(msg) = incoming_rx.recv().await {
        let agent = agent.clone();
        let channels = channels.clone();
        let gateway_tx = gateway_tx.clone();
        let security = config.security.clone();
        tokio::spawn(async move {
            // Build an approval gate bound to the originating channel so sensitive
            // tools ask the user in the same chat they came from.
            let chan = channels.get(&msg.channel).cloned();
            let gate = std::sync::Arc::new(zegion_core::hooks::ApprovalGate::new(
                security,
                chan.clone(),
                msg.user_id.clone(),
            ));

            // Stream the reply, editing a draft message in place as deltas arrive.
            let reply_target = msg.reply_to.clone().unwrap_or_else(|| msg.user_id.clone());
            let draft_handle: Option<String> = if let Some(ch) = chan.clone() {
                ch.send_draft(&reply_target, "…").await.ok().flatten()
            } else {
                None
            };

            let accumulated = std::sync::Arc::new(tokio::sync::Mutex::new(String::new()));
            let last_edit = std::sync::Arc::new(tokio::sync::Mutex::new(
                std::time::Instant::now() - std::time::Duration::from_secs(2),
            ));
            let acc_cb = accumulated.clone();
            let le_cb = last_edit.clone();
            let chan_cb = chan.clone();
            let handle_cb = draft_handle.clone();
            let target_cb = reply_target.clone();

            let reply = {
                let mut guard = agent.write().await;
                guard
                    .turn_stream(&msg.text, gate, move |delta| {
                        let acc = acc_cb.clone();
                        let le = le_cb.clone();
                        let ch = chan_cb.clone();
                        let handle = handle_cb.clone();
                        let target = target_cb.clone();
                        tokio::spawn(async move {
                            let mut a = acc.lock().await;
                            a.push_str(&delta);
                            let full = a.clone();
                            drop(a);
                            // Throttle edits to ~1.5s to respect channel rate limits.
                            let mut last = le.lock().await;
                            if last.elapsed() >= std::time::Duration::from_millis(1500) {
                                if let (Some(ch), Some(h)) = (ch, handle) {
                                    let _ = ch.update_draft(&target, &h, &full).await;
                                }
                                *last = std::time::Instant::now();
                            }
                        });
                    })
                    .await
            };

            let text = match reply {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("zegion error: {e}");
                    format!("sorry, I hit an error: {e}")
                }
            };

            // Settle the draft to the final text so edit-based channels show the full
            // reply and CLI closes the streamed line.
            if let (Some(ch), Some(h)) = (chan.clone(), draft_handle.clone()) {
                let _ = ch.update_draft(&reply_target, &h, &text).await;
            }

            let out = OutgoingMessage {
                channel: msg.channel.clone(),
                user_id: reply_target.clone(),
                text,
                is_final: true,
            };
            match msg.channel.as_str() {
                "gateway" => {
                    if let Some(tx) = gateway_tx {
                        let _ = tx.send(out);
                    }
                }
                "cli" => {
                    if let Some(ch) = channels.get("cli") {
                        let _ = ch.send(out).await;
                    }
                }
                other => {
                    // Edit-based channels already show the final text via the draft; only
                    // send a fresh message when no draft was created.
                    if draft_handle.is_none() {
                        if let Some(ch) = channels.get(other) {
                            let _ = ch.send(out).await;
                        }
                    }
                }
            }
        });
    }
    Ok(())
}
