# Channels

Channels are how users talk to Zegion. Each implements the `Channel` trait
(`zegion-core/src/channel.rs`) and is enabled via `[channels.<name>]` in the config.
All channels feed a single shared inbound queue; the dispatcher routes replies back to
the originating channel and builds a per-message approval gate bound to it.

## The Channel trait

```rust
async fn start(&self, incoming: mpsc::Sender<IncomingMessage>) -> Result<()>;
async fn send(&self, msg: OutgoingMessage) -> Result<()>;
async fn ask_approval(&self, user_id: &str, prompt: &str) -> Result<bool>;
async fn send_draft(&self, user_id: &str, initial: &str) -> Result<Option<String>>;
async fn update_draft(&self, user_id: &str, handle: &str, text: &str) -> Result<()>;
```

- `start` begins receiving inbound messages and pushes them into the shared queue.
- `send` delivers a final reply.
- `ask_approval` asks a yes/no question for sensitive tools (supervised mode).
- `send_draft` / `update_draft` implement **live streaming**: a draft message is created
  and edited in place as deltas arrive. Channels that can't edit fall back to a final send.

## CLI (built-in, default)

Interactive terminal REPL with line history. Streams deltas as live typing.

```toml
[channels.cli]
enabled = true
```

## Gateway (built-in, default)

Local HTTP/SSE server. Good for embedding Zegion in other apps.

```toml
[channels.gateway]
enabled = true
bind = "127.0.0.1:8787"
```

Endpoints: `GET /health`, `POST /v1/chat`, `GET /v1/events` (SSE).

## Telegram (teloxide)

Long-polling — **no public URL needed**. Supports live message edits for streaming.

```toml
[channels.telegram]
enabled = true
bot_token = "${TELEGRAM_BOT_TOKEN}"
```

Create a bot with [@BotFather](https://t.me/BotFather) to get a token.

## Discord (serenity)

Gateway websocket — **no public URL needed**. Supports live edits.

```toml
[channels.discord]
enabled = true
bot_token = "${DISCORD_BOT_TOKEN}"
```

Create an application + bot at the [Discord Developer Portal](https://discord.com/developers),
enable the **Message Content** intent, and invite the bot with `Send Messages` permission.

## Slack (socket mode)

Socket Mode — **no public URL needed**.

```toml
[channels.slack]
enabled = true
bot_token = "${SLACK_BOT_TOKEN}"    # xoxb-...
app_token = "${SLACK_APP_TOKEN}"    # xapp-... (socket mode)
```

Create a Slack app, enable Socket Mode, add the `chat:write` scope and message event
subscriptions, and install it to your workspace.

## WhatsApp (Cloud API)

Webhook receiver + Graph API sender — **requires a public URL** (or a tunnel such as
ngrok/cloudflared) for Meta to reach the webhook.

```toml
[channels.whatsapp]
enabled = true
phone_number_id = "${WA_PHONE_NUMBER_ID}"
access_token = "${WA_ACCESS_TOKEN}"
verify_token = "zegion-verify"
```

The webhook listens on `127.0.0.1:8790/webhook/whatsapp` (override with
`ZEGION_WHATSAPP_BIND`). Point the Meta app's webhook at that URL with the verify token.

## Approval flow

In supervised mode (`[security] supervised = true`), when the agent wants to run a
sensitive tool, it asks in the same channel the message came from. Reply `yes`/`no`.
Channels without an interactive reply path deny by default (safe).
