# Contributing to Zegion

Thanks for helping build Zegion. This guide keeps contributions smooth for everyone.

## Getting started

```sh
git clone https://github.com/zakirkun/zegion
cd zegion
cargo build
cargo test --workspace
```

Requires Rust 1.85+ (we build on stable). For MCP tests you'll want `node`/`npx` and
`uv`/`uvx` installed.

## Workflow

1. Open an issue (or pick one) before large changes so we agree on direction.
2. Create a branch: `git checkout -b feat/short-name`.
3. Make your change with tests. Keep functions small and files focused.
4. Run the gates locally:
   ```sh
   cargo fmt --all
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   ```
5. Commit with a [Conventional Commit](https://www.conventionalcommits.org) message
   (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`).
6. Open a PR. CI must be green before merge.

## Code guidelines

- **Low-spec first** — avoid heavy dependencies; justify any that you add.
- **Safe autonomy** — destructive or sensitive actions go through the approval gate.
- **Local-first** — never exfiltrate memory, persona, or config; keep secrets out of logs.
- **Async** — don't block the runtime; use `spawn_blocking` for sync I/O.
- Document public APIs only where the intent isn't obvious from the code.

## Adding a channel

Implement the `Channel` trait (`zegion-core/src/channel.rs`), wire it in the CLI run
loop, and document it in `docs/channels.md`.

## Adding a tool

Prefer the `#[zegion_tool]` derive macro. Gate anything sensitive via `ApprovalGate`.
Add a test under the relevant crate's `tests/`.

## Documentation

Update `docs/` and `CHANGELOG.md` for user-facing changes. Keep the README's feature list
and roadmap in sync.

## Code of conduct

Be respectful and constructive. We follow the
[Contributor Covenant](https://www.contributor-covenant.org/version/2/1/code_of_conduct/).
