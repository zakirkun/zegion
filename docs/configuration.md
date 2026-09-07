# Configuration

Zegion is configured with a single TOML file (`zegion.toml`). Run `zegion onboard` to
generate one interactively, or copy `zegion.example.toml` and edit it.

`${ENV_VAR}` placeholders in the file are expanded from the process environment at load
time, so secrets never need to be written to disk.

## [agent]

| key            | default        | description                          |
| -------------- | -------------- | ------------------------------------ |
| `name`         | `Zegion`       | agent display name                   |
| `persona_path` | `PERSONAL.md`  | path to the persona file             |
| `max_steps`    | `12`           | hard cap on agent loop steps per turn |
| `temperature`  | `40`           | 0–100 (scaled to 0.0–1.0)            |

## [models]

| key                  | default                              | description                     |
| -------------------- | ------------------------------------ | ------------------------------- |
| `default_provider`   | `openrouter`                         | provider for the main loop      |
| `default_model`      | `google/gemini-2.0-flash-001`        | model for the main loop         |
| `worker_provider`    | `openrouter`                         | provider for background jobs    |
| `worker_model`       | `google/gemini-2.0-flash-lite-001`   | cheap model for consolidation   |
| `embedding_provider` | `openai`                             | provider for embeddings         |
| `embedding_model`    | `text-embedding-3-small`             | model for memory embeddings     |

## [providers.*]

Each provider is a table with optional `api_key` and `base_url`:

```toml
[providers.openrouter]
api_key = "${OPENROUTER_API_KEY}"

[providers.ollama]            # any OpenAI-compatible endpoint
base_url = "http://localhost:11434/v1"
api_key = "ollama"
```

Built-in provider names: `openrouter`, `openai`, `anthropic`, `google`. Any other name is
treated as a generic OpenAI-compatible endpoint (Ollama, LM Studio, vLLM, Groq, Together…).

## [memory]

| key               | default           | description                                  |
| ----------------- | ----------------- | -------------------------------------------- |
| `db_path`         | `data/zegion.db`  | SQLite database path                         |
| `recall_top_k`    | `8`               | memories injected into context               |
| `recall_min_score`| `0.35`            | minimum relevance score for recall           |
| `raw_turn_window` | `24`              | raw turns before consolidation kicks in      |

## [security]

| key                   | default                          | description                            |
| --------------------- | -------------------------------- | -------------------------------------- |
| `supervised`          | `true`                           | require approval for sensitive tools   |
| `require_approval`    | `["shell","write_file","http_post"]` | tools that always need approval    |
| `daily_spend_cap_usd` | `1.0`                            | soft daily spend estimate (0 = none)   |

## [channels.*]

See [channels.md](channels.md). CLI and gateway are enabled by default.

## [plugins]

| key            | default   | description                                       |
| -------------- | --------- | ------------------------------------------------- |
| `dir`          | `plugins` | directory of `.rhai` plugin scripts               |
| `auto_presets` | `false`   | also auto-connect built-in MCP presets            |
| `[[plugins.mcp]]` | —      | MCP servers: `name`, `command`, `args`            |
