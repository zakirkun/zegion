# Extending Zegion: register your own Skills, Plugins, and MCP

Everything is auto-discovered — no code changes or recompiles needed. Drop files in the
right folder (or add a config block) and Zegion registers them at startup and hot-reloads
on change.

## Skills (markdown)

**Where:** `skills/<name>.md`

**Format:**

```markdown
# Skill Name

Use this when <trigger condition>.

## Procedure
1. Ordered, atomic steps.
2. ...

## Output
- The exact format to return.

## Rules
- Hard constraints and edge cases.
```

**Tips:** one procedure per file; name the trigger explicitly; define the output; keep it
short. The filename (without `.md`) becomes the skill name in the catalog injected into
the system prompt.

**Reload:** save the file — the autoloader picks it up on the next interval.

## Plugins (Rhai)

**Where:** `plugins/<name>.rhai`

**Contract:** define `main(input)` and call it at the end. `input` is a string; the
return value becomes the tool output.

```rhai
fn main(input) {
    "echo: " + input
}

main(input)
```

**Rhai notes for this build:** declare variables with `let`; build strings with
`s += c.to_string()`; iterate with `for c in s.chars()`; `split`, `sort`, `contains`,
`to_lower`, `to_upper().to_string()` are available; `pad`, `trim`, string `ends_with` are not.

**Run it:** the agent can call it with the `plugin_run` tool, or programmatically via
`PluginRegistry::run("name", "input")`.

## Plugins (WASM)

**Where:** `plugins/*.wasm` (or compile-on-demand from Rust).

**ABI:** export `memory`, `alloc(n)->ptr`, and `run(ptr,len)->packed_ptr_len`. Runs in a
fuel- and memory-capped wasmtime sandbox. See [plugins.md](plugins.md#wasm-sandbox).

## MCP servers

**Where:** `zegion.toml` under `[plugins]`.

```toml
[[plugins.mcp]]
name = "myserver"
command = "npx"
args = ["-y", "@org/my-mcp-server", "--flag"]
```

Zegion spawns the process, lists its tools, and registers them as `<name>__<tool>`.
A failing server is skipped without taking the agent down.

**Presets:** set `auto_presets = true` to also connect the 5 built-in presets
(filesystem, git, sqlite, fetch, memory). Explicit entries override presets by name.

**Requirements:** `npx` for Node servers, `uvx` for Python servers.

## Quick checklist

| capability | drop-in location | auto-reload | tool name            |
| ---------- | ---------------- | ----------- | -------------------- |
| Skill      | `skills/*.md`    | yes         | (in system prompt)   |
| Rhai       | `plugins/*.rhai` | yes         | via `plugin_run`     |
| WASM       | `plugins/*.wasm` | no          | registered tool      |
| MCP        | `zegion.toml`    | on restart  | `<server>__<tool>`   |
