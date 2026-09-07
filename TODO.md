# TODO

Near-term, actionable items. For the bigger picture see [ROADMAP.md](ROADMAP.md).

## Correctness / robustness

- [ ] `memory_forget` — implement actual deletion by id (currently lists candidates only).
- [ ] `todo` tool — implement `done` (mark task complete) instead of add/list only.
- [ ] `clipboard` tool — real clipboard on desktop (arboard crate), graceful headless fallback.
- [ ] `hash` tool — use a real sha256/md5 (e.g. `sha2` crate) instead of the default hasher.
- [ ] `regex_extract` — real regex via the `regex` crate (currently literal substring).
- [ ] Telegram/Discord `ask_approval` — poll for the actual yes/no reply (currently deny-by-default).

## Features

- [ ] Onboarding wizard: token-entry screen (paste secrets directly, not just `${ENV}`).
- [ ] Streaming: wire deltas into WhatsApp (progressive sends) — CLI/Telegram/Discord already live-edit.
- [ ] Skills: `skill_read` tool returns the real skill body from the registry.
- [ ] WASM sandbox: load guest tools from `plugins/*.wasm` at startup (currently compile-on-demand).
- [ ] `#[zegion_tool]` — emit full JSON output schema, not just the type name.
- [ ] Multi-agent orchestrator: an example two-agent setup in `examples/`.

## Release / packaging

- [ ] Verify `release.yml` produces all 5 artifacts on a real `v*` tag.
- [ ] Add `.deb` / `.rpm` / MSI packaging (cargo-bundle or cargo-dist).
- [ ] Publish to crates.io (`cargo install zegion`).
- [ ] Docker image (scratch/distroless) for the gateway.

## Performance / low-spec

- [ ] Measure release binary size; add `opt-level = "z"` + further feature trimming.
- [ ] Optional `pulley` interpreter backend for wasmtime to cut binary size on weak targets.
- [ ] Profile memory usage of the consolidation worker on very long sessions.

## Docs

- [ ] `examples/` for: custom channel, custom tool, custom skill, WASM guest, orchestrator.
- [ ] Video/GIF of the onboarding TUI in the README.
