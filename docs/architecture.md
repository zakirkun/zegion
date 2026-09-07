# Architecture

Zegion is a Cargo workspace of five crates with a clear dependency direction:

```
zegion-cli ──► zegion-channels ──► zegion-core ──► zegion-memory
                    │                    ▲
                    └────────────────────┘
zegion-macros (proc macros, used by zegion-core)
```

## zegion-core

The heart of the agent. Owns the perceive → recall → think → act → remember loop.

- **agent.rs** — `Agent`: persona, memory, skills, model handles, sliding-window context.
  `turn_with_gate` (buffered) and `turn_stream` (delta callbacks).
- **persona.rs** — loads `PERSONAL.md` into the system prompt.
- **provider.rs** — `AnyLanguageModel` / `AnyEmbeddingModel`: runtime-erased wrappers over
  aisdk's typed providers, so any configured provider is interchangeable.
- **pipeline.rs** — `Pipeline<M>`: an aisdk `LanguageModel` that wraps a model with retry,
  a TTL response cache, and guardrails. Every inference goes through it.
- **guardrails.rs** — `Guardrail` trait + `GuardrailSet`; input/output inspection.
- **tools.rs** — built-in tool set; sensitive tools (`shell`, `write_file`, `http_post`,
  `download_file`) routed through the `ApprovalGate`.
- **hooks.rs** — `ApprovalGate`: supervised-autonomy decisions per channel.
- **skills.rs / plugins.rs / mcp.rs / registry.rs** — capability loading and auto-registration.
- **sandbox.rs** — wasmtime-based sandboxed WASM tool execution (fuel + memory capped).
- **orchestrator.rs** — typed pub/sub `AgentBus` + `Environment` for multi-agent setups.
- **worker.rs** — background memory consolidation + reflection using the cheap worker model.
- **gateway.rs** — axum HTTP/SSE gateway.

## zegion-memory

Local-first persistence.

- **store.rs** — `MemoryStore`: SQLite (bundled) + sqlite-vec (vector recall) + FTS5
  (keyword recall), wrapped in async `spawn_blocking`.
- **backend.rs** — `MemoryBackend` trait (open to other stores) and `SlidingWindow`
  conversation policy.
- **types.rs** — `Memory`, `MemoryKind` (episodic/semantic/reflection), `ScoredMemory`.

## zegion-channels

Implements the `Channel` trait per surface: `start` (inbound) and `send`/`send_draft`/
`update_draft` (outbound, with live-edit streaming where supported). Channels: CLI,
Telegram (teloxide), Discord (serenity), Slack (socket mode), WhatsApp (Cloud API).

## zegion-cli

The `zegion` binary: subcommands `run`, `onboard`, `learn`, `recall`, `config`. The `run`
loop owns the shared inbound queue, builds a per-message `ApprovalGate` bound to the
originating channel, streams replies, and starts the consolidation worker.

## Data flow (one turn)

1. A channel pushes an `IncomingMessage` onto the shared queue.
2. The dispatcher builds an `ApprovalGate` for that channel.
3. `Agent::turn_stream` recalls memory, composes the system prompt
   (persona + skill catalog + recalled memories), and runs the model via the pipeline
   with the full tool set (built-ins + MCP).
4. Sensitive tool calls ask the user in-channel for approval.
5. Deltas stream to the channel (live edit); the final reply is recorded to the
   sliding window and episodic memory.
6. The consolidation worker periodically distills episodic turns into semantic memory.
