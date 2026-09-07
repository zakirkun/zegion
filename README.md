# Zegion

A low-spec-friendly autonomous agent written in Rust on top of [`aisdk`](https://aisdk.rs).
Zegion has a persona (`PERSONAL.md`), strong local memory (SQLite + sqlite-vec + FTS5),
a skills system, rich tooling, a plugin system, and a multi-channel gateway.

## Features

- **Persona-driven** — `PERSONAL.md` is loaded at startup and injected into the system prompt.
- **Local-first memory** — episodic, semantic, and reflection layers in SQLite; vector
  recall via `sqlite-vec`, keyword recall via FTS5. Nothing leaves your machine.
- **Multi-provider** — OpenRouter, OpenAI, Anthropic, Google, Ollama, and any
  OpenAI-compatible endpoint, selectable via config (runtime `DynamicModel`).
- **Skills system** — drop markdown files into `skills/` to teach reusable procedures.
- **Tools** — built-in `now`, `calc`; gated `shell` / `write_file` / `http_get` behind
  supervised approval.
- **Plugins** — Rhai scripts in `plugins/` plus MCP servers (via `rmcp`) launched as child
  processes; their tools are auto-discovered and exposed to the agent namespaced as
  `<server>__<tool>`.
- **Channels** — CLI REPL, a local HTTP/SSE gateway, Telegram (long polling), Discord
  (gateway websocket), Slack (socket mode), and WhatsApp (Cloud API webhook), all behind
  the same `Channel` trait.
- **Streaming** — `Agent::turn_stream` emits text deltas as they arrive via aisdk
  `stream_text`, for live-typing channel UX.

## Layout

```
crates/
  zegion-core      persona, config, agent loop, skills, tools, plugins, gateway
  zegion-memory    SQLite + sqlite-vec + FTS5 store (episodic/semantic/reflection)
  zegion-channels  CLI channel + Channel trait impls
  zegion-cli       the `zegion` binary
PERSONAL.md        the agent's persona
zegion.example.toml config template (copy to zegion.toml)
skills/            markdown skills
plugins/           rhai script plugins
data/              runtime SQLite db (gitignored)
```

## Quick start

```sh
cp zegion.example.toml zegion.toml
# set API keys via env (see zegion.example.toml) or edit zegion.toml
$env:OPENROUTER_API_KEY="..."

cargo run -- run                 # start CLI + gateway
cargo run -- recall "dark mode"  # search memory
cargo run -- learn "user prefers concise answers"
```

The gateway listens on `http://127.0.0.1:8787`:
- `GET  /health`
- `POST /v1/chat`   `{ "text": "hi", "user_id": "me" }`
- `GET  /v1/events` (SSE stream of agent replies)

## Roadmap

- [x] Sensitive tools (`shell`, `write_file`) gated by per-channel approval in the agent loop
- [x] `memory_recall` / `memory_store` agent-facing tools
- [x] Memory consolidation + reflection background worker (uses `worker` model)
- [x] Telegram channel (long polling, no public URL needed)
- [x] MCP client tool bridging (child-process servers, tool discovery + namespaced calls)
- [x] Discord channel (gateway websocket + REST send)
- [x] Slack channel (socket mode + chat.postMessage)
- [x] WhatsApp channel (Cloud API webhook + Graph API send)
- [x] Streaming responses (`Agent::turn_stream` text deltas via aisdk `stream_text`)
- [x] Stream deltas wired into live channel edits (Telegram/Discord message edit)
- [x] ReAct + structured-output executors (`zegion-core/react.rs`)
- [x] `#[zegion_tool]` derive macro (tool + output-schema) (`zegion-macros`)
- [x] Sandboxed WASM tool execution (wasmtime, fuel + memory capped) (`zegion-core/sandbox.rs`)
- [x] Extensible memory backends + sliding-window memory (`zegion-memory/backend.rs`)
- [x] LLM guardrails (input/output, sensitive-data + length) (`zegion-core/guardrails.rs`)
- [x] LLM optimization pipeline (retry, response cache, guardrail stage) (`zegion-core/pipeline.rs`)
- [x] Multi-agent orchestration: typed pub/sub bus + environment (`zegion-core/orchestrator.rs`)
- [x] Pipeline (retry + cache + guardrails) wired into every inference (main + worker)
- [ ] Release binary size tuning for low-spec targets
