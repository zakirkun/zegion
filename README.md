<div align="center">

```
 _____          _
|__  /___  __ _(_) ___  _ __
  / // _ \/ _` | |/ _ \| '_ \
 / /|  __/ (_| | | (_) | | | |
/____\___|\__, |_|\___/|_| |_|
          |___/
```

# Zegion

**A low-spec, local-first autonomous agent written in Rust.**

Persona-driven, persistent memory, multi-channel, extensible via tools, skills, plugins, and MCP.

[![CI](https://github.com/zakirkun/zegion/actions/workflows/ci.yml/badge.svg)](https://github.com/zakirkun/zegion/actions/workflows/ci.yml)
[![Release](https://github.com/zakirkun/zegion/actions/workflows/release.yml/badge.svg)](https://github.com/zakirkun/zegion/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

</div>

---

## Overview

Zegion is a general-purpose autonomous agent designed to run comfortably on modest hardware.
It pairs a single persistent persona (`PERSONAL.md`) with a local memory store (SQLite +
sqlite-vec + FTS5), a rich tool set, and a pluggable channel system so it can meet users where
they already are — terminal, Telegram, Discord, Slack, WhatsApp, or a local HTTP/SSE gateway.

Every LLM call flows through an optimization pipeline (retry, response cache, guardrails), and
capabilities (skills, tools, plugins, MCP servers) are auto-discovered and hot-reloaded.

## Key features

- **Persona-driven** — `PERSONAL.md` defines identity, tone, and boundaries; injected into every prompt.
- **Local-first memory** — episodic, semantic, and reflection layers; vector + keyword recall; sliding-window context; background consolidation worker. Nothing leaves the machine.
- **Multi-provider** — OpenRouter, OpenAI, Anthropic, Google, Ollama, and any OpenAI-compatible endpoint, selected at runtime via config.
- **Multi-channel** — CLI REPL, HTTP/SSE gateway, Telegram (teloxide), Discord (serenity), Slack (socket mode), WhatsApp (Cloud API).
- **Rich tooling** — 20+ built-in tools, gated sensitive actions with in-chat approval.
- **Skills** — markdown procedures auto-loaded from `skills/` (16 included).
- **Plugins** — Rhai scripts in `plugins/` (11 included) plus a sandboxed WASM tool runtime.
- **MCP** — connect MCP servers as child processes; tools auto-discovered and namespaced (5 presets).
- **Optimization & safety** — LLM pipeline (retry, cache, guardrails) on every inference; multi-agent pub/sub orchestrator.

## Install

**Prebuilt binaries** (Linux, macOS, Windows):

```sh
# Linux / macOS
curl -fsSL https://raw.githubusercontent.com/zakirkun/zegion/main/install.sh | bash

# Windows (PowerShell)
irm https://raw.githubusercontent.com/zakirkun/zegion/main/install.ps1 | iex
```

**From source** (Rust 1.85+):

```sh
git clone https://github.com/zakirkun/zegion
cd zegion
cargo build --release
```

## Quick start

```sh
zegion onboard     # interactive TUI wizard → writes zegion.toml
# set your provider key, e.g.:
export OPENROUTER_API_KEY=...        # Windows: $env:OPENROUTER_API_KEY="..."
zegion run         # start the agent (CLI + enabled channels)
```

Other commands:

```sh
zegion run                        # chat + gateway + enabled channels
zegion recall "dark mode"         # search memory
zegion learn "user prefers terse answers"
zegion config                     # print resolved configuration
```

The local gateway listens on `http://127.0.0.1:8787`:

- `GET  /health`
- `POST /v1/chat`  `{ "text": "hi", "user_id": "me" }`
- `GET  /v1/events` (SSE stream of agent replies)

## Documentation

- [Architecture](docs/architecture.md)
- [Configuration](docs/configuration.md)
- [Channels](docs/channels.md)
- [Tools](docs/tools.md)
- [Skills](docs/skills.md)
- [Plugins & WASM](docs/plugins.md)
- [MCP](docs/mcp.md)
- [Memory](docs/memory.md)
- [Extending: register your own skills, plugins & MCP](docs/extending.md)

Project: [TODO](TODO.md) · [Roadmap](ROADMAP.md) · [Changelog](CHANGELOG.md) · [Contributing](CONTRIBUTING.md)

## Workspace layout

```
crates/
  zegion-core      persona, config, agent loop, tools, skills, plugins, MCP,
                   guardrails, pipeline, orchestrator, sandbox, gateway
  zegion-memory    SQLite + sqlite-vec + FTS5 store, backend trait, sliding window
  zegion-channels  CLI + Telegram/Discord/Slack/WhatsApp channel impls
  zegion-macros    #[zegion_tool] derive macro
  zegion-cli       the `zegion` binary (run / onboard / learn / recall / config)
PERSONAL.md        the agent's persona
skills/            markdown skills (auto-loaded)
plugins/           rhai plugin scripts (auto-loaded)
docs/              documentation
```

## Contributing & license

Contributions welcome — see [CONTRIBUTING.md](CONTRIBUTING.md). Licensed under [MIT](LICENSE).
