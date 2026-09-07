# Changelog

All notable changes to Zegion are documented here. This project adheres to
[Semantic Versioning](https://semver.org) and
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added
- Cross-platform CI workflow (fmt, clippy, tests on Linux/Windows/macOS).
- Cross-platform release workflow building 5 targets (linux x86_64/aarch64, windows
  x86_64, macos x86_64/aarch64) and publishing archives on `v*` tags.
- Cross-platform installers: `install.sh` (Linux/macOS) and `install.ps1` (Windows).
- Professional documentation set under `docs/` (architecture, configuration, channels,
  tools, skills, plugins, MCP, memory) plus TODO, ROADMAP, and a registration guide.

## [0.1.0] - 2026-09-07

### Added
- Cargo workspace: `zegion-core`, `zegion-memory`, `zegion-channels`, `zegion-macros`,
  `zegion-cli`.
- Persona system (`PERSONAL.md`) injected into every system prompt.
- Local-first memory: SQLite (bundled) + sqlite-vec + FTS5, episodic/semantic/reflection
  layers, sliding-window context, and a background consolidation + reflection worker.
- Multi-provider LLM support via aisdk with runtime `DynamicModel` selection:
  OpenRouter, OpenAI, Anthropic, Google, Ollama, and any OpenAI-compatible endpoint.
- Channels behind one `Channel` trait: CLI REPL, HTTP/SSE gateway, Telegram (teloxide),
  Discord (serenity), Slack (socket mode), WhatsApp (Cloud API).
- Supervised autonomy with approval-gated sensitive tools and live-edit streaming replies.
- 20+ built-in tools plus a `#[zegion_tool]` derive macro.
- Skills system (markdown, hot-reload) with 16 skills; Rhai plugins (11) plus a
  fuel- and memory-capped sandboxed WASM tool runtime.
- MCP child-process bridging with 5 built-in presets and unified auto-registration.
- LLM optimization pipeline (retry with backoff, TTL response cache, guardrails) applied
  to every inference; input/output guardrails.
- Multi-agent typed pub/sub orchestrator with an environment manager.
- Interactive ratatui onboarding wizard (`zegion onboard`) and a FIGlet startup banner.

[Unreleased]: https://github.com/zakirkun/zegion/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/zakirkun/zegion/releases/tag/v0.1.0
