# Plugins & WASM Sandbox

Zegion has two plugin runtimes: **Rhai scripts** (lightweight, low-spec friendly) and a
**sandboxed WASM runtime** (for untrusted or heavier plugins). Both are auto-loaded.

## Rhai script plugins

Drop a `.rhai` file into `plugins/`. It must define a `main(input)` function and call it.
The agent runs it via the `plugin_run` tool; you can also call it programmatically via
`PluginRegistry::run(name, input)`.

```rhai
// plugins/shout.rhai
fn main(input) {
    input + "!"
}

main(input)
```

### Included plugins (11)

| plugin          | description                                  |
| --------------- | -------------------------------------------- |
| `hello`         | example / smoke test                         |
| `word_count`    | count chars, words, lines                    |
| `slugify`       | title → URL-friendly slug                    |
| `reverse`       | reverse a string                             |
| `title_case`    | capitalize each word                         |
| `dedupe_lines`  | remove duplicate lines (order-preserving)    |
| `sort_lines`    | sort lines alphabetically                    |
| `is_palindrome` | palindrome check                             |
| `wrap`          | hard-wrap text to 60 columns                 |
| `censor`        | mask emails / long tokens                    |
| `csv_to_table`  | CSV → aligned text table                     |

### Rhai notes (this build)

- Variables are declared with `let` (reassignable); `let mut` is not used.
- Build strings with `s += c.to_string()`; iterate with `for c in s.chars()`.
- `split`, `sort`, `contains`, `to_lower`, `to_upper().to_string()` work; `pad`, `trim`,
  and string `ends_with` are not available in this build.

## MCP plugins

MCP servers run as child processes and are bridged into tools. See [mcp.md](mcp.md).

## WASM sandbox

`zegion-core/src/sandbox.rs` runs guest WASM modules in wasmtime with hard limits:

- **Fuel cap** (CPU) — infinite loops are killed.
- **Memory cap** (RAM) — guests can't exhaust the host.
- Fresh store per call — no state leaks between invocations.

### Guest ABI

Guests export three symbols and pass strings through linear memory:

```wat
(memory (export "memory") 1)
(func (export "alloc") (param i32) (result i32))      ;; allocate n bytes -> ptr
(func (export "run") (param i32 i32) (result i64))    ;; ptr,len in -> packed ptr,len out
```

`run` returns a packed `i64`: high 32 bits = output pointer, low 32 bits = output length.

### Exposing a WASM tool

```rust
let tool = WasmTool::compile(wat_or_wasm_bytes, SandboxLimits::default())?;
let aisdk_tool = wasm_aisdk_tool("my_tool", "what it does", input_schema, Arc::new(tool));
```

A minimal `host_log(ptr, len)` host function is provided for guest-side logging.
