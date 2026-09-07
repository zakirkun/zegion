# MCP (Model Context Protocol)

Zegion connects to [MCP](https://modelcontextprotocol.io) servers as child processes and
bridges their tools into the agent, namespaced as `<server>__<tool>`.

## Configuration

Add servers under `[plugins]` in `zegion.toml`:

```toml
[[plugins.mcp]]
name = "filesystem"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "."]
```

At startup Zegion spawns each server, handshakes it, lists its tools, and registers them
as aisdk tools. A server that fails to start is logged and skipped — one bad server never
takes the agent down.

## Built-in presets (5)

Enable all of them at once with `auto_presets = true` under `[plugins]`, or configure
individually:

| preset       | command | provides                                   |
| ------------ | ------- | ------------------------------------------ |
| `filesystem` | `npx`   | file read/write/search within a root       |
| `git`        | `uvx`   | git status/log/diff/commit                 |
| `sqlite`     | `uvx`   | query/inspect SQLite databases             |
| `fetch`      | `uvx`   | fetch and read web pages                   |
| `memory`     | `npx`   | a knowledge-graph memory server            |

Explicit `[[plugins.mcp]]` entries always take precedence over presets with the same name.

## Requirements

- `npx` for Node-based servers (filesystem, memory).
- `uvx` (from [uv](https://docs.astral.sh/uv/)) for Python-based servers (git, sqlite, fetch).

## How calls work

1. The model calls `<server>__<tool>` with a JSON argument object.
2. Zegion maps the arguments into the MCP `inputSchema` and issues a `call_tool` request.
3. The result's `structuredContent` (preferred) or text blocks are returned to the model.

Tool errors from the server surface as tool errors, so the model can react to them.
