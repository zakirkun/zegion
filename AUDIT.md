# Zegion Audit — Gaps, Not-Implemented, and Bugs

Audited: 2026-09-07 · scope: full workspace (`crates/`), config, channels, CI/CD, docs.

Legend — **Status**: 🔴 not implemented / stub · 🟡 partial / works-with-caveats · 🟢 done & verified.
**Severity**: P0 blocks core value · P1 degrades a feature · P2 polish.

---

## A. Stubbed / placeholder tools (P1)

| # | Item | Location | Detail | Status |
|---|------|----------|--------|--------|
| A1 | `skill_read` tool returns a canned string | `zegion-core/src/tools.rs:814` | Does not read the skill from the registry — `ToolContext` lacks the `SkillRegistry`. Should return the real skill body. | 🔴 |
| A2 | `plugin_run` tool returns a canned string | `zegion-core/src/tools.rs:837` | Does not execute the plugin — `ToolContext` lacks the `PluginRegistry`. Should call `PluginRegistry::run(name, input)`. | 🔴 |
| A3 | `hash` tool uses `DefaultHasher` | `zegion-core/src/tools.rs:391` | Not sha256/md5 as advertised. Use the `sha2` crate for real digests. | 🔴 |
| A4 | `regex_extract` does literal substring | `zegion-core/src/tools.rs:542` | Not a real regex. Use the `regex` crate; return capture groups. | 🔴 |
| A5 | `clipboard` tool is a stub | `zegion-core/src/tools.rs:954` | Always returns "unavailable". Use `arboard` on desktop with a headless fallback. | 🔴 |
| A6 | `memory_forget` only lists candidates | `zegion-core/src/tools.rs:802` | Says "deletion not yet supported". Implement `MemoryStore::delete(id)` + recall-driven delete. | 🔴 |
| A7 | `todo` tool `done` op unimplemented | `zegion-core/src/tools.rs:947` | Only add/list work. Implement mark-done (tag or delete). | 🔴 |

## B. Approval / supervised-autonomy (P0/P1)

| # | Item | Location | Detail | Status |
|---|------|----------|--------|--------|
| B1 | `ask_approval` always returns `false` on all channels | `zegion-channels/src/{telegram,discord,slack,whatsapp,cli}.rs` | The prompt is sent but the reply is **never read**, so sensitive tools are always denied. Supervised mode effectively disables `shell`/`write_file`/`http_post`/`download_file`. **P0** — core safety feature is non-functional. | 🔴 |
| B2 | Telegram had a polling impl, removed in teloxide rewrite | `zegion-channels/src/telegram.rs:131` | The pre-teloxide version polled `getUpdates` for the reply; the teloxide rewrite dropped it. Needs a pending-approval waiter. | 🔴 |
| B3 | No per-user approval allowlist / session memory | `zegion-core/src/hooks.rs` | Every sensitive call re-asks; no "approve for session" or per-user trust. | 🟡 |

## C. Auto-load / hot-reload (P1)

| # | Item | Location | Detail | Status |
|---|------|----------|--------|--------|
| C1 | `spawn_autoloader` is never called | `zegion-core/src/registry.rs:125`, `zegion-cli/src/main.rs` | The watcher exists but nothing spawns it — **hot-reload of skills/plugins does not happen at runtime** (only at startup). Wire it in `run()`. | 🔴 |
| C2 | `CapabilityRegistry` skills/plugins not shared with tools | `zegion-core/src/agent.rs:71` | Agent clones `caps.skills` + `caps.mcp_tools` but drops `caps.plugins`; reloads wouldn't propagate to the live agent anyway (no `Arc<RwLock>` sharing). Needed for A1/A2 and C1. | 🔴 |
| C3 | MCP hot-reload not attempted | `zegion-core/src/registry.rs` | MCP connects only at startup (documented as restart-only). Acceptable, but note as a known limit. | 🟡 |

## D. Pipeline / guardrails (P1/P2)

| # | Item | Location | Detail | Status |
|---|------|----------|--------|--------|
| D1 | Streaming bypasses cache/retry/guardrails | `zegion-core/src/pipeline.rs:stream_text` | `stream_text` passes straight through, so guardrail input/output checks and retry do not apply to streamed turns. Document or wrap. | 🟡 |
| D2 | Guardrail input check skipped when no user message | `zegion-core/src/pipeline.rs:last_user_text` | If options lack a user message, input guardrail is skipped silently. Edge case; mostly fine in practice. | 🟡 |
| D3 | Response cache keyed only on system+last-user | `zegion-core/src/pipeline.rs:fingerprint` | Multi-turn context isn't part of the fingerprint — two identical last-messages with different history collide. Low risk but worth noting. | 🟡 |
| D4 | Spend cap is config-only, not enforced | `zegion-core/src/config.rs:daily_spend_cap_usd` | No usage accounting exists; the cap is never checked. | 🔴 |

## E. WASM sandbox (P2)

| # | Item | Location | Detail | Status |
|---|------|----------|--------|--------|
| E1 | `host_log` doesn't read guest memory | `zegion-core/src/sandbox.rs:57` | Receives ptr/len but logs only the numbers, not the message. Needs `caller.get_export("memory")` to read the string. | 🟡 |
| E2 | No auto-load of `plugins/*.wasm` at startup | `zegion-core/src/registry.rs` | WASM tools are compile-on-demand; no directory scan registers them as tools. | 🔴 |
| E3 | No WASI support | `zegion-core/src/sandbox.rs` | Bare `env` host surface only; guests can't use stdio/fs. By design for low-spec, but limits real plugins. | 🟡 |

## F. Multi-agent orchestrator (P2)

| # | Item | Location | Detail | Status |
|---|------|----------|--------|--------|
| F1 | Not wired into the CLI run loop | `zegion-core/src/orchestrator.rs` | Bus + Environment are built and tested, but no participants are registered in `main.rs`; effectively unused by default. | 🟡 |
| F2 | No backpressure / participant error isolation beyond logging | `orchestrator.rs` | A panicking participant task only logs; no circuit breaker. | 🟡 |

## G. Channels (P1/P2)

| # | Item | Location | Detail | Status |
|---|------|----------|--------|--------|
| G1 | Slack/Discord/WhatsApp not tested against live APIs | `zegion-channels/` | Logic is complete and compiles; only Telegram (teloxide) and MCP have live-verified paths. Needs real-token e2e. | 🟡 |
| G2 | WhatsApp streaming falls back to final send | `zegion-channels/src/whatsapp.rs` | `update_draft` is a no-op (Cloud API can't edit); deltas only render on CLI/Telegram/Discord. Documented limitation. | 🟡 |
| G3 | No reconnect/backoff on Discord gateway / Slack socket | `discord.rs`, `slack.rs` | A dropped socket ends the task; no resume/reconnect loop. | 🔴 |
| G4 | `split_message` may split mid-word/multibyte boundary | all channels | Splits by bytes; can break UTF-8 chars at the edge. Use char-boundary-aware splitting. | 🟡 |

## H. Memory (P2)

| # | Item | Location | Detail | Status |
|---|------|----------|--------|--------|
| H1 | No delete-by-id API | `zegion-memory/src/store.rs` | Blocks A6. Add `delete(id)` + cascade to FTS/vec tables. | 🔴 |
| H2 | `clear_episodic` FTS cleanup is a no-op-ish query | `store.rs:clear_episodic` | The `DELETE FROM memories_fts WHERE content NOT IN (SELECT content FROM memories)` is fragile; better to delete by rowid join or rebuild FTS. | 🟡 |
| H3 | Embedding dim is hardcoded (384) | `zegion-memory/src/store.rs:EMBEDDING_DIM` | Changing embedding models with other dims requires a code change + migration. Make it configurable. | 🟡 |
| H4 | No memory namespacing per user/channel | `store.rs` | All users share one memory space. Multi-user deployments need a `namespace`/`owner` column. | 🔴 |

## I. Config / onboarding (P2)

| # | Item | Location | Detail | Status |
|---|------|----------|--------|--------|
| I1 | Onboarding can't paste real secrets | `zegion-cli/src/onboard.rs` | Only `${ENV}` placeholders are written; no token-entry screen. | 🔴 |
| I2 | `${ENV}` expansion is global, not per-field | `zegion-core/src/config.rs:expand_env` | Expands across the whole file before parsing — a literal `${...}` in a non-secret value would also be expanded. Acceptable but note. | 🟡 |
| I3 | No config validation | `config.rs` | Unknown keys are silently ignored (`#[serde(default)]`); typos pass unnoticed. Add a `deny_unknown_fields` option or a `zegion config --check`. | 🟡 |

## J. CI/CD & release (P1) — current run

| # | Item | Location | Detail | Status |
|---|------|----------|--------|--------|
| J1 | Publish release job failing on `v0.1.0` | `.github/workflows/release.yml` | Builds all 5 artifacts ✓, but `gh release create` fails (glob/artifact path). Latest fix (download into `dist/`, `ls` verify) is in-flight; not yet confirmed green. | 🟡 |
| J2 | linux-aarch64 required 2 fixes | `release.yml` | `cross` (Docker) failed; switched to native `ubuntu-24.04-arm`. Now green. | 🟢 |
| J3 | No crates.io / Docker / OS-package publishing | `release.yml` | Only raw archives. Tracked in TODO. | 🔴 |
| J4 | No release gating on CI | `release.yml` | Release runs independently of CI; a red main can still be tagged. Add `needs:` or a branch-protection rule. | 🟡 |

## K. Docs vs reality (P2)

| # | Item | Location | Detail | Status |
|---|------|----------|--------|--------|
| K1 | README/docs claim features that are stubbed | `README.md`, `docs/` | `hash`, `regex_extract`, `clipboard`, `skill_read`, `plugin_run`, approval replies, hot-reload are presented as working but are stubs/unwired (see A/B/C). Align docs or implement. | 🟡 |
| K2 | ROADMAP marks some stubbed items "done" | `ROADMAP.md` | v0.3 lists capability areas as done; several underlying tools are stubs. Reconcile. | 🟡 |
| K3 | `zegion.example.toml` temperature note vs code | `zegion.example.toml` | Comment says scaled 0.0–1.0 by aisdk; verify aisdk actually scales 0–100 → 0–1 (it does divide by 100, but confirm). | 🟡 |

## L. Test coverage gaps (P2)

| # | Item | Location | Detail | Status |
|---|------|----------|--------|--------|
| L1 | No test for the live approval flow | — | Because B1 is stubbed, there's no test proving a user can approve a sensitive tool. Add once B1 is fixed. | 🔴 |
| L2 | No test for hot-reload | — | C1 unwired means no test covers `spawn_autoloader`. | 🔴 |
| L3 | No e2e test through a real provider | — | All model tests use fakes; a paid/sandbox e2e would catch provider drift. | 🟡 |
| L4 | `unimplemented!()` in test fakes | `zegion-core/tests/pipeline.rs:54,100` | `stream_text` in `FakeModel`/`Capture` panics if ever called. Acceptable for these tests, but fragile if reused. | 🟡 |

---

## Priority shortlist (do first)

1. **B1** — make `ask_approval` actually read the user's yes/no on at least CLI + Telegram. (P0)
2. **J1** — confirm the release publish job goes green; re-tag `v0.1.0`. (P1)
3. **A1 + A2 + C2** — thread `SkillRegistry`/`PluginRegistry` into `ToolContext` so `skill_read`/`plugin_run` work. (P1)
4. **C1** — call `spawn_autoloader` in the run loop for real hot-reload. (P1)
5. **A3/A4/A6** — real `sha2`, real `regex`, and memory delete-by-id. (P1)
6. **K1/K2** — reconcile README/ROADMAP with actual status. (P2)

## Confirmed working (no action)

- Memory store (SQLite + sqlite-vec + FTS5): vector + keyword recall tested.
- Gated tool execution model (deny-without-approval path tested).
- Pipeline retry/cache/guardrail (integration-tested with fake models).
- MCP bridging (live `server-everything` e2e).
- Orchestrator pub/sub routing (typed-message test).
- WASM sandbox execution + fuel-cap termination (WAT tests).
- Onboarding config round-trip; FIGlet banner.
- CI: fmt/clippy/tests green on Linux/Windows/macOS. Release builds all 5 targets.
