# Tools

Tools are functions the model can call during a turn. Zegion ships 20+ built-in tools,
plus any tools discovered from MCP servers and WASM sandbox tools. Sensitive tools are
gated by the approval system in supervised mode.

## Core tools

| tool           | description                                        | gated |
| -------------- | -------------------------------------------------- | ----- |
| `now`          | current UTC time (RFC3339)                         | no    |
| `calc`         | arithmetic expressions (`2 + 2 * 10`)              | no    |
| `memory_recall`| search long-term memory                            | no    |
| `memory_store` | save a durable fact to semantic memory             | no    |
| `read_file`    | read a text file (truncated)                       | no    |
| `shell`        | run a shell command                                | **yes** |
| `write_file`   | write a file                                       | **yes** |
| `http_get`     | HTTP GET a URL                                     | no    |

## General-purpose tools (20)

| tool            | description                                      | gated |
| --------------- | ------------------------------------------------ | ----- |
| `json_query`    | extract a value from JSON by dot path            | no    |
| `uuid`          | generate UUID v4(s)                              | no    |
| `hash`          | hash text                                        | no    |
| `base64`        | base64 encode/decode                             | no    |
| `url_parse`     | split a URL into parts                           | no    |
| `regex_extract` | regex matches in text                            | no    |
| `datetime`      | now / parse / add_days                           | no    |
| `list_dir`      | list a directory                                 | no    |
| `file_stat`     | file/dir size and type                           | no    |
| `env_get`       | read a non-secret env var                        | no    |
| `system_info`   | OS, arch, CPU count                              | no    |
| `http_post`     | HTTP POST a body                                 | **yes** |
| `download_file` | download a URL to disk                           | **yes** |
| `memory_forget` | find memories to delete                          | no    |
| `skill_read`    | read a skill's instructions                      | no    |
| `plugin_run`    | run a Rhai plugin                                | no    |
| `template_render`| render `{{var}}` templates                      | no    |
| `text_stats`    | count chars/words/lines                          | no    |
| `todo`          | persistent to-do list in memory                  | no    |
| `clipboard`     | clipboard access (stub headless)                 | no    |

## MCP tools

Tools from connected MCP servers appear namespaced as `<server>__<tool>` (e.g.
`filesystem__read_file`). See [mcp.md](mcp.md).

## WASM sandbox tools

Guest WASM modules run in a fuel- and memory-capped wasmtime sandbox and are exposed as
tools. See [plugins.md](plugins.md#wasm-sandbox).

## Writing your own tool

Use the derive macro (generates the tool plus an output-schema helper):

```rust
#[zegion_macros::zegion_tool]
/// Get the weather for a location.
pub fn get_weather(location: String) -> Tool {
    Ok(format!("sunny in {location}"))
}
```

Register it in `build_tools()` (`zegion-core/src/tools.rs`).
