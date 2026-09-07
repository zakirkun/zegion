# Roadmap

Zegion's north star: a **general-purpose, low-spec, local-first autonomous agent** that is
genuinely useful out of the box and trivially extensible.

## Milestones

### v0.1 — Foundation (done)

- Workspace: core / memory / channels / macros / cli.
- Persona (`PERSONAL.md`) injected into the system prompt.
- Local memory: SQLite + sqlite-vec + FTS5, episodic/semantic/reflection, sliding window.
- Multi-provider via aisdk (OpenRouter/OpenAI/Anthropic/Google/Ollama/compatible).
- CLI REPL + HTTP/SSE gateway.

### v0.2 — Channels & autonomy (done)

- Telegram, Discord, Slack, WhatsApp channels behind one `Channel` trait.
- Supervised autonomy: approval-gated sensitive tools, per-channel.
- Streaming replies with live message edits.
- Background memory consolidation + reflection worker.

### v0.3 — Capabilities (done)

- 20+ built-in tools; `#[zegion_tool]` derive macro.
- Skills system (markdown, hot-reload) with 16 skills.
- Rhai plugins (11) + sandboxed WASM tool runtime.
- MCP child-process bridging with 5 presets + auto-registration.
- LLM pipeline (retry, cache, guardrails) on every inference.
- Multi-agent typed pub/sub orchestrator.

### v0.4 — Polish & distribution (in progress)

- Cross-platform CI (fmt/clippy/test) and release builds (5 targets).
- Cross-platform installers (shell + PowerShell).
- Professional documentation set.
- crates.io + Docker + OS package installers.
- Examples directory.

### v0.5 — Depth

- Real regex/hash/clipboard implementations.
- Interactive approval replies on all channels.
- `skill_read` returns real skill bodies; WASM guest auto-loading from `plugins/`.
- Onboarding token entry; richer TUI.

### v0.6 — Scale

- Optional remote/Postgres memory backend.
- Multi-agent orchestrator examples + scheduling/cron.
- Per-channel personas and per-user memory namespaces.
- Cost tracking + hard spend caps enforced in the pipeline.

### Later / ideas

- Voice I/O channel.
- Web dashboard for memory/skill/plugin management.
- A curated community skill/plugin registry with one-command install.

## Guiding principles

1. **Low-spec first** — every feature is weighed against binary size and RAM.
2. **Local-first** — memory and persona stay on the user's machine.
3. **Safe autonomy** — the agent assists; destructive actions ask first.
4. **Batteries included, swappable** — sensible defaults, everything replaceable.
