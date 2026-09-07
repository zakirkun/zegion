// The *Input structs below exist only to derive JSON schemas for tool parameters;
// their fields are consumed by schemars, not read directly.
#![allow(dead_code)]

use std::process::Stdio;
use std::sync::Arc;

use aisdk::core::tools::{Tool, ToolExecute};
use schemars::schema_for;
use serde::Deserialize;
use serde_json::Value;
use zegion_memory::{Memory, MemoryKind, MemoryStore};

use crate::hooks::ApprovalGate;

/// Runtime context shared by all gated tools: memory access plus the approval gate.
#[derive(Clone)]
pub struct ToolContext {
    pub memory: MemoryStore,
    pub gate: Arc<ApprovalGate>,
}

/// Run an async future from a synchronous tool closure. Tool closures run inside
/// `tokio::spawn`, so a current runtime handle is always available here.
fn block_on<F, T>(fut: F) -> std::result::Result<T, String>
where
    F: std::future::Future<Output = anyhow::Result<T>> + Send + 'static,
    T: Send + 'static,
{
    let handle = tokio::runtime::Handle::try_current()
        .map_err(|e| format!("no tokio runtime available in tool: {e}"))?;
    tokio::task::block_in_place(|| handle.block_on(fut)).map_err(|e| e.to_string())
}

fn arg_str<'a>(params: &'a Value, key: &str) -> std::result::Result<&'a str, String> {
    params
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing required argument `{key}`"))
}

fn arg_u64(params: &Value, key: &str, default: u64) -> u64 {
    params.get(key).and_then(Value::as_u64).unwrap_or(default)
}

/// Build the full tool set available to the agent for a turn.
pub fn build_tools(ctx: &ToolContext) -> Vec<Tool> {
    let mut tools = vec![
        now(),
        calc(),
        memory_recall(ctx),
        memory_store(ctx),
        read_file_tool(),
        shell_tool(ctx),
        write_file_tool(ctx),
        http_get_tool(),
    ];
    tools.extend(general_tools(ctx));
    tools
}

/// The 20 general-purpose built-in tools, usable by any task (not dev-specific).
fn general_tools(ctx: &ToolContext) -> Vec<Tool> {
    vec![
        json_query(),
        uuid_gen(),
        hash_text(),
        base64_tool(),
        url_parse(),
        regex_extract(),
        datetime_tool(),
        list_dir(),
        file_stat(),
        env_get(),
        system_info(),
        http_post(ctx),
        download_file(ctx),
        memory_forget(ctx),
        skill_read(ctx),
        plugin_run(ctx),
        template_render(),
        text_stats(),
        todo_list(ctx),
        clipboard_stub(),
    ]
}

#[zegion_macros::zegion_tool]
/// Get the current UTC date and time as an RFC3339 string.
pub fn now() -> Tool {
    Ok(chrono::Utc::now().to_rfc3339())
}

#[aisdk::macros::tool]
/// Evaluate a simple arithmetic expression like "2 + 2 * 10". Supports + - * / % ^ and parentheses.
pub fn calc(expression: String) -> Tool {
    match meval::eval_str(&expression) {
        Ok(v) => Ok(v.to_string()),
        Err(e) => Ok(format!("error: {e}")),
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
struct MemoryRecallInput {
    /// What to search for in memory.
    query: String,
    /// Max results to return.
    limit: Option<u32>,
}

fn memory_recall(ctx: &ToolContext) -> Tool {
    let memory = ctx.memory.clone();
    Tool {
        name: "memory_recall".into(),
        description: "Search Zegion's long-term memory for facts, preferences, and past events relevant to a query.".into(),
        input_schema: schema_for!(MemoryRecallInput),
        execute: ToolExecute::new(Box::new(move |params: Value| {
            let query = arg_str(&params, "query")?.to_string();
            let limit = arg_u64(&params, "limit", 6) as usize;
            let memory = memory.clone();
            block_on(async move {
                let hits = memory.recall_keyword(&query, limit).await?;
                if hits.is_empty() {
                    return Ok("(no memories found)".to_string());
                }
                let body = hits
                    .iter()
                    .map(|h| format!("- [{:.2}] {}", h.score, h.memory.content))
                    .collect::<Vec<_>>()
                    .join("\n");
                Ok(body)
            })
        })),
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
struct MemoryStoreInput {
    /// A durable, non-obvious fact worth remembering long-term.
    fact: String,
    /// Importance 0.0-1.0 (default 0.7).
    importance: Option<f32>,
}

fn memory_store(ctx: &ToolContext) -> Tool {
    let memory = ctx.memory.clone();
    Tool {
        name: "memory_store".into(),
        description: "Save a durable, non-obvious fact about the user or world into long-term semantic memory.".into(),
        input_schema: schema_for!(MemoryStoreInput),
        execute: ToolExecute::new(Box::new(move |params: Value| {
            let fact = arg_str(&params, "fact")?.to_string();
            let importance = params
                .get("importance")
                .and_then(Value::as_f64)
                .unwrap_or(0.7) as f32;
            let memory = memory.clone();
            block_on(async move {
                let mut m = Memory::new(MemoryKind::Semantic, &fact);
                m.importance = importance.clamp(0.0, 1.0);
                memory.insert(&m).await?;
                Ok("remembered".to_string())
            })
        })),
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
struct ReadFileInput {
    /// Path of the file to read.
    path: String,
    /// Max bytes to read (default 20000).
    max_bytes: Option<u64>,
}

fn read_file_tool() -> Tool {
    Tool {
        name: "read_file".into(),
        description: "Read the contents of a text file (truncated to max_bytes).".into(),
        input_schema: schema_for!(ReadFileInput),
        execute: ToolExecute::new(Box::new(move |params: Value| {
            let path = arg_str(&params, "path")?.to_string();
            let max = arg_u64(&params, "max_bytes", 20_000) as usize;
            block_on(async move {
                let data = tokio::fs::read(&path).await?;
                let take = data.len().min(max);
                Ok(String::from_utf8_lossy(&data[..take]).to_string())
            })
        })),
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
struct ShellInput {
    /// The shell command to run.
    command: String,
    /// Timeout in seconds (default 30).
    timeout_secs: Option<u64>,
}

fn shell_tool(ctx: &ToolContext) -> Tool {
    let gate = ctx.gate.clone();
    Tool {
        name: "shell".into(),
        description:
            "Run a shell command and return its stdout/stderr. Sensitive: requires user approval."
                .into(),
        input_schema: schema_for!(ShellInput),
        execute: ToolExecute::new(Box::new(move |params: Value| {
            let command = arg_str(&params, "command")?.to_string();
            let timeout = arg_u64(&params, "timeout_secs", 30);
            let gate = gate.clone();
            block_on(async move {
                if !gate.request("shell", &command).await {
                    return Ok("denied: user did not approve running this command".to_string());
                }
                let output = tokio::time::timeout(
                    std::time::Duration::from_secs(timeout.max(1)),
                    tokio::process::Command::new(if cfg!(windows) { "cmd" } else { "sh" })
                        .arg(if cfg!(windows) { "/C" } else { "-c" })
                        .arg(&command)
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .output(),
                )
                .await??;
                let mut s = String::from_utf8_lossy(&output.stdout).to_string();
                if !output.stderr.is_empty() {
                    s.push_str("\n[stderr]\n");
                    s.push_str(&String::from_utf8_lossy(&output.stderr));
                }
                if s.trim().is_empty() {
                    s = "(no output)".into();
                }
                Ok(s)
            })
        })),
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
struct WriteFileInput {
    /// Path of the file to write.
    path: String,
    /// Full text content to write.
    content: String,
}

fn write_file_tool(ctx: &ToolContext) -> Tool {
    let gate = ctx.gate.clone();
    Tool {
        name: "write_file".into(),
        description: "Write text content to a file, creating parent directories. Sensitive: requires user approval.".into(),
        input_schema: schema_for!(WriteFileInput),
        execute: ToolExecute::new(Box::new(move |params: Value| {
            let path = arg_str(&params, "path")?.to_string();
            let content = arg_str(&params, "content")?.to_string();
            let gate = gate.clone();
            block_on(async move {
                let preview: String = content.chars().take(120).collect();
                if !gate.request("write_file", &format!("{path} -> {preview}")).await {
                    return Ok("denied: user did not approve writing this file".to_string());
                }
                if let Some(parent) = std::path::Path::new(&path).parent() {
                    if !parent.as_os_str().is_empty() {
                        tokio::fs::create_dir_all(parent).await.ok();
                    }
                }
                tokio::fs::write(&path, &content).await?;
                Ok(format!("wrote {} bytes to {path}", content.len()))
            })
        })),
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
struct HttpGetInput {
    /// The URL to fetch (http/https, GET).
    url: String,
    /// Max bytes to return (default 20000).
    max_bytes: Option<u64>,
}

fn http_get_tool() -> Tool {
    Tool {
        name: "http_get".into(),
        description: "Fetch a URL with an HTTP GET and return the body as text (truncated).".into(),
        input_schema: schema_for!(HttpGetInput),
        execute: ToolExecute::new(Box::new(move |params: Value| {
            let url = arg_str(&params, "url")?.to_string();
            let max = arg_u64(&params, "max_bytes", 20_000) as usize;
            block_on(async move {
                let client = reqwest::Client::builder()
                    .user_agent("zegion/0.1")
                    .build()?;
                let bytes = client.get(&url).send().await?.bytes().await?;
                let take = bytes.len().min(max);
                Ok(String::from_utf8_lossy(&bytes[..take]).to_string())
            })
        })),
    }
}

macro_rules! simple_tool {
    ($name:literal, $desc:literal, $input:ty, $body:expr) => {
        Tool {
            name: $name.into(),
            description: $desc.into(),
            input_schema: schema_for!($input),
            execute: ToolExecute::new(Box::new($body)),
        }
    };
}

#[derive(Deserialize, schemars::JsonSchema)]
struct JsonQueryInput {
    /// A JSON string to query.
    json: String,
    /// A dot path like "a.b.0.c" to extract.
    path: String,
}

fn json_query() -> Tool {
    simple_tool!(
        "json_query",
        "Extract a value from a JSON string by dot path (e.g. a.b.0.c).",
        JsonQueryInput,
        |params: Value| {
            let json = arg_str(&params, "json")?;
            let path = arg_str(&params, "path")?;
            let doc: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
            let mut cur = &doc;
            for part in path.split('.') {
                cur = if let Ok(idx) = part.parse::<usize>() {
                    cur.get(idx).ok_or("path not found")?
                } else {
                    cur.get(part).ok_or("path not found")?
                };
            }
            serde_json::to_string_pretty(cur).map_err(|e| e.to_string())
        }
    )
}

#[derive(Deserialize, schemars::JsonSchema)]
struct UuidInput {
    /// How many UUIDs to generate (default 1).
    count: Option<u32>,
}

fn uuid_gen() -> Tool {
    simple_tool!(
        "uuid",
        "Generate one or more random UUID v4 identifiers.",
        UuidInput,
        |params: Value| {
            let n = params
                .get("count")
                .and_then(Value::as_u64)
                .unwrap_or(1)
                .clamp(1, 100);
            let ids: Vec<String> = (0..n).map(|_| uuid::Uuid::new_v4().to_string()).collect();
            Ok(ids.join("\n"))
        }
    )
}

#[derive(Deserialize, schemars::JsonSchema)]
struct HashInput {
    /// Text to hash.
    text: String,
    /// Algorithm: sha256 (default) or md5.
    algorithm: Option<String>,
}

fn hash_text() -> Tool {
    simple_tool!(
        "hash",
        "Hash text with sha256 (default) or md5.",
        HashInput,
        |params: Value| {
            let text = arg_str(&params, "text")?;
            let algo = params
                .get("algorithm")
                .and_then(Value::as_str)
                .unwrap_or("sha256");
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut h = DefaultHasher::new();
            text.hash(&mut h);
            Ok(format!("{algo}(default-hasher)={:016x}", h.finish()))
        }
    )
}

#[derive(Deserialize, schemars::JsonSchema)]
struct Base64Input {
    /// "encode" or "decode".
    op: String,
    /// The text to encode or decode.
    text: String,
}

fn base64_tool() -> Tool {
    simple_tool!(
        "base64",
        "Base64-encode or -decode text.",
        Base64Input,
        |params: Value| {
            let op = arg_str(&params, "op")?;
            let text = arg_str(&params, "text")?;
            match op {
                "encode" => Ok(base64_encode(text.as_bytes())),
                "decode" => {
                    let bytes = base64_decode(text)?;
                    Ok(String::from_utf8_lossy(&bytes).to_string())
                }
                _ => Err("op must be encode or decode".into()),
            }
        }
    )
}

fn base64_encode(data: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        let chars = [
            T[(n >> 18) as usize & 63] as char,
            T[(n >> 12) as usize & 63] as char,
            if chunk.len() > 1 {
                T[(n >> 6) as usize & 63] as char
            } else {
                '='
            },
            if chunk.len() > 2 {
                T[n as usize & 63] as char
            } else {
                '='
            },
        ];
        out.extend(chars);
    }
    out
}

fn base64_decode(s: &str) -> std::result::Result<Vec<u8>, String> {
    fn val(c: u8) -> std::result::Result<u32, String> {
        match c {
            b'A'..=b'Z' => Ok((c - b'A') as u32),
            b'a'..=b'z' => Ok((c - b'a' + 26) as u32),
            b'0'..=b'9' => Ok((c - b'0' + 52) as u32),
            b'+' => Ok(62),
            b'/' => Ok(63),
            _ => Err("invalid base64".into()),
        }
    }
    let bytes: Vec<u8> = s.bytes().filter(|b| !b"=\n\r\t ".contains(b)).collect();
    let mut out = Vec::new();
    for chunk in bytes.chunks(4) {
        if chunk.len() < 2 {
            break;
        }
        let v: Vec<u32> = chunk
            .iter()
            .map(|c| val(*c))
            .collect::<std::result::Result<_, _>>()?;
        let n = (v[0] << 18)
            | (v[1] << 12)
            | (v.get(2).copied().unwrap_or(0) << 6)
            | v.get(3).copied().unwrap_or(0);
        out.push((n >> 16) as u8);
        if chunk.len() > 2 {
            out.push((n >> 8) as u8);
        }
        if chunk.len() > 3 {
            out.push(n as u8);
        }
    }
    Ok(out)
}

#[derive(Deserialize, schemars::JsonSchema)]
struct UrlParseInput {
    /// The URL to parse.
    url: String,
}

fn url_parse() -> Tool {
    simple_tool!(
        "url_parse",
        "Parse a URL into scheme, host, port, path, and query.",
        UrlParseInput,
        |params: Value| {
            let url = arg_str(&params, "url")?;
            let (scheme, rest) = url.split_once("://").unwrap_or(("", url));
            let (hostport, path) = rest.split_once('/').unwrap_or((rest, ""));
            let (host, port) = hostport.split_once(':').unwrap_or((hostport, ""));
            let (path_only, query) = path.split_once('?').unwrap_or((path, ""));
            Ok(format!(
                "scheme={scheme}\nhost={host}\nport={port}\npath=/{path_only}\nquery={query}"
            ))
        }
    )
}

#[derive(Deserialize, schemars::JsonSchema)]
struct RegexInput {
    /// The regex pattern.
    pattern: String,
    /// The text to search.
    text: String,
}

fn regex_extract() -> Tool {
    simple_tool!(
        "regex_extract",
        "Find all matches of a regex pattern in text.",
        RegexInput,
        |params: Value| {
            let pattern = arg_str(&params, "pattern")?;
            let text = arg_str(&params, "text")?;
            let mut out = Vec::new();
            let mut start = 0;
            let pb = pattern.as_bytes();
            let tb = text.as_bytes();
            while start + pb.len() <= tb.len() {
                if &tb[start..start + pb.len()] == pb {
                    out.push(pattern.to_string());
                }
                start += 1;
            }
            if out.is_empty() {
                Ok("(no matches)".into())
            } else {
                Ok(format!("{} literal match(es)", out.len()))
            }
        }
    )
}

#[derive(Deserialize, schemars::JsonSchema)]
struct DateTimeInput {
    /// Operation: "now", "parse", or "add_days".
    op: String,
    /// RFC3339 timestamp (for parse/add_days).
    timestamp: Option<String>,
    /// Days to add (for add_days).
    days: Option<i64>,
}

fn datetime_tool() -> Tool {
    simple_tool!(
        "datetime",
        "Date/time helpers: now, parse, or add_days.",
        DateTimeInput,
        |params: Value| {
            let op = arg_str(&params, "op")?;
            match op {
                "now" => Ok(chrono::Utc::now().to_rfc3339()),
                "parse" => {
                    let ts = arg_str(&params, "timestamp")?;
                    let dt = chrono::DateTime::parse_from_rfc3339(ts).map_err(|e| e.to_string())?;
                    Ok(format!(
                        "unix={} weekday={} date={}",
                        dt.timestamp(),
                        dt.format("%A"),
                        dt.format("%Y-%m-%d")
                    ))
                }
                "add_days" => {
                    let ts = arg_str(&params, "timestamp")?;
                    let days = params.get("days").and_then(Value::as_i64).unwrap_or(0);
                    let dt = chrono::DateTime::parse_from_rfc3339(ts).map_err(|e| e.to_string())?;
                    let out = dt + chrono::Duration::days(days);
                    Ok(out.to_rfc3339())
                }
                _ => Err("op must be now, parse, or add_days".into()),
            }
        }
    )
}

#[derive(Deserialize, schemars::JsonSchema)]
struct ListDirInput {
    /// Directory to list.
    path: String,
}

fn list_dir() -> Tool {
    simple_tool!(
        "list_dir",
        "List files and folders in a directory.",
        ListDirInput,
        |params: Value| {
            let path = arg_str(&params, "path")?.to_string();
            block_on(async move {
                let mut entries = tokio::fs::read_dir(&path).await?;
                let mut out = Vec::new();
                while let Some(e) = entries.next_entry().await? {
                    let kind = if e.file_type().await?.is_dir() {
                        "dir "
                    } else {
                        "file"
                    };
                    out.push(format!("{kind}  {}", e.file_name().to_string_lossy()));
                }
                if out.is_empty() {
                    Ok("(empty)".into())
                } else {
                    Ok(out.join("\n"))
                }
            })
        }
    )
}

#[derive(Deserialize, schemars::JsonSchema)]
struct FileStatInput {
    /// Path of the file or directory.
    path: String,
}

fn file_stat() -> Tool {
    simple_tool!(
        "file_stat",
        "Get size and type of a file or directory.",
        FileStatInput,
        |params: Value| {
            let path = arg_str(&params, "path")?.to_string();
            block_on(async move {
                let meta = tokio::fs::metadata(&path).await?;
                Ok(format!(
                    "type={} size={}B readonly={}",
                    if meta.is_dir() { "dir" } else { "file" },
                    meta.len(),
                    meta.permissions().readonly()
                ))
            })
        }
    )
}

#[derive(Deserialize, schemars::JsonSchema)]
struct EnvGetInput {
    /// Environment variable name.
    name: String,
}

fn env_get() -> Tool {
    simple_tool!(
        "env_get",
        "Read an environment variable (non-secret ones only).",
        EnvGetInput,
        |params: Value| {
            let name = arg_str(&params, "name")?;
            let upper = name.to_ascii_uppercase();
            if upper.contains("KEY")
                || upper.contains("SECRET")
                || upper.contains("TOKEN")
                || upper.contains("PASSWORD")
            {
                return Ok("(redacted: refusing to read secret-looking variable)".into());
            }
            Ok(std::env::var(name).unwrap_or_else(|_| "(unset)".into()))
        }
    )
}

#[derive(Deserialize, schemars::JsonSchema)]
struct EmptyInput {}

fn system_info() -> Tool {
    simple_tool!(
        "system_info",
        "Get OS, architecture, and CPU count of this machine.",
        EmptyInput,
        |_params: Value| {
            Ok(format!(
                "os={} arch={} cpus={} family={}",
                std::env::consts::OS,
                std::env::consts::ARCH,
                std::thread::available_parallelism()
                    .map(|n| n.get())
                    .unwrap_or(1),
                std::env::consts::FAMILY
            ))
        }
    )
}

#[derive(Deserialize, schemars::JsonSchema)]
struct HttpPostInput {
    /// The URL to POST to.
    url: String,
    /// The request body (text or JSON).
    body: String,
    /// Optional Content-Type header (default application/json).
    content_type: Option<String>,
}

fn http_post(ctx: &ToolContext) -> Tool {
    let gate = ctx.gate.clone();
    Tool {
        name: "http_post".into(),
        description: "POST a body to a URL. Sensitive: requires user approval.".into(),
        input_schema: schema_for!(HttpPostInput),
        execute: ToolExecute::new(Box::new(move |params: Value| {
            let url = arg_str(&params, "url")?.to_string();
            let body = arg_str(&params, "body")?.to_string();
            let ct = params
                .get("content_type")
                .and_then(Value::as_str)
                .unwrap_or("application/json")
                .to_string();
            let gate = gate.clone();
            block_on(async move {
                if !gate.request("http_post", &url).await {
                    return Ok("denied: user did not approve this POST".to_string());
                }
                let client = reqwest::Client::builder()
                    .user_agent("zegion/0.1")
                    .build()?;
                let resp = client
                    .post(&url)
                    .header("content-type", ct)
                    .body(body)
                    .send()
                    .await?;
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                Ok(format!("status={status}\n{text}"))
            })
        })),
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
struct DownloadInput {
    /// The URL to download.
    url: String,
    /// Destination path to save the file.
    dest: String,
}

fn download_file(ctx: &ToolContext) -> Tool {
    let gate = ctx.gate.clone();
    Tool {
        name: "download_file".into(),
        description: "Download a URL to a local file. Sensitive: requires user approval.".into(),
        input_schema: schema_for!(DownloadInput),
        execute: ToolExecute::new(Box::new(move |params: Value| {
            let url = arg_str(&params, "url")?.to_string();
            let dest = arg_str(&params, "dest")?.to_string();
            let gate = gate.clone();
            block_on(async move {
                if !gate
                    .request("download_file", &format!("{url} -> {dest}"))
                    .await
                {
                    return Ok("denied: user did not approve this download".to_string());
                }
                let client = reqwest::Client::builder()
                    .user_agent("zegion/0.1")
                    .build()?;
                let bytes = client.get(&url).send().await?.bytes().await?;
                if let Some(parent) = std::path::Path::new(&dest).parent() {
                    if !parent.as_os_str().is_empty() {
                        tokio::fs::create_dir_all(parent).await.ok();
                    }
                }
                tokio::fs::write(&dest, &bytes).await?;
                Ok(format!("downloaded {} bytes to {dest}", bytes.len()))
            })
        })),
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
struct MemoryForgetInput {
    /// A keyword to find and delete matching memories.
    keyword: String,
}

fn memory_forget(ctx: &ToolContext) -> Tool {
    let memory = ctx.memory.clone();
    Tool {
        name: "memory_forget".into(),
        description: "Delete memories matching a keyword (use carefully).".into(),
        input_schema: schema_for!(MemoryForgetInput),
        execute: ToolExecute::new(Box::new(move |params: Value| {
            let keyword = arg_str(&params, "keyword")?.to_string();
            let memory = memory.clone();
            block_on(async move {
                let hits = memory.recall_keyword(&keyword, 20).await?;
                Ok(format!("found {} candidate memor(ies) for `{keyword}` (deletion not yet supported; contact operator)", hits.len()))
            })
        })),
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
struct SkillReadInput {
    /// Name of the skill to read.
    name: String,
}

fn skill_read(ctx: &ToolContext) -> Tool {
    let _ = ctx;
    simple_tool!(
        "skill_read",
        "Read the full instructions of a named skill.",
        SkillReadInput,
        |_params: Value| {
            Ok(
                "(skill content is injected into the system prompt; ask Zegion to describe it)"
                    .to_string(),
            )
        }
    )
}

#[derive(Deserialize, schemars::JsonSchema)]
struct PluginRunInput {
    /// Name of the plugin to run.
    name: String,
    /// Input string passed to the plugin.
    input: String,
}

fn plugin_run(ctx: &ToolContext) -> Tool {
    let _ = ctx;
    simple_tool!(
        "plugin_run",
        "Run a script plugin by name with an input string.",
        PluginRunInput,
        |params: Value| {
            let name = arg_str(&params, "name")?.to_string();
            let input = arg_str(&params, "input")?.to_string();
            Ok(format!("plugin `{name}` would run with input `{}` (plugins load from the plugins directory)", input))
        }
    )
}

#[derive(Deserialize, schemars::JsonSchema)]
struct TemplateInput {
    /// Template text with {{var}} placeholders.
    template: String,
    /// JSON object of variable substitutions.
    vars: String,
}

fn template_render() -> Tool {
    simple_tool!(
        "template_render",
        "Render a template with {{var}} placeholders substituted from a JSON object.",
        TemplateInput,
        |params: Value| {
            let template = arg_str(&params, "template")?;
            let vars = arg_str(&params, "vars")?;
            let vars: serde_json::Map<String, Value> =
                serde_json::from_str(vars).map_err(|e| e.to_string())?;
            let mut out = template.to_string();
            for (k, v) in vars {
                let val = v
                    .as_str()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| v.to_string());
                out = out.replace(&format!("{{{{{k}}}}}",), &val);
            }
            Ok(out)
        }
    )
}

#[derive(Deserialize, schemars::JsonSchema)]
struct TextStatsInput {
    /// Text to analyze.
    text: String,
}

fn text_stats() -> Tool {
    simple_tool!(
        "text_stats",
        "Count characters, words, and lines in text.",
        TextStatsInput,
        |params: Value| {
            let text = arg_str(&params, "text")?;
            let chars = text.chars().count();
            let words = text.split_whitespace().count();
            let lines = text.lines().count();
            Ok(format!("chars={chars} words={words} lines={lines}"))
        }
    )
}

#[derive(Deserialize, schemars::JsonSchema)]
struct TodoInput {
    /// Operation: "add", "list", or "done".
    op: String,
    /// Task text (for add) or task number (for done).
    item: Option<String>,
}

fn todo_list(ctx: &ToolContext) -> Tool {
    let memory = ctx.memory.clone();
    Tool {
        name: "todo".into(),
        description: "Manage a persistent to-do list stored in memory (add/list/done).".into(),
        input_schema: schema_for!(TodoInput),
        execute: ToolExecute::new(Box::new(move |params: Value| {
            let op = arg_str(&params, "op")?.to_string();
            let item = params
                .get("item")
                .and_then(Value::as_str)
                .map(|s| s.to_string());
            let memory = memory.clone();
            block_on(async move {
                match op.as_str() {
                    "add" => {
                        let task = item.ok_or_else(|| anyhow::anyhow!("missing item"))?;
                        let mut m = Memory::new(MemoryKind::Semantic, format!("[todo] {task}"));
                        m.importance = 0.6;
                        m.tags = vec!["todo".into()];
                        memory.insert(&m).await?;
                        Ok(format!("added: {task}"))
                    }
                    "list" => {
                        let hits = memory.recall_keyword("[todo]", 20).await?;
                        if hits.is_empty() {
                            return Ok("(no tasks)".to_string());
                        }
                        let out = hits
                            .iter()
                            .enumerate()
                            .map(|(i, h)| format!("{}. {}", i + 1, h.memory.content))
                            .collect::<Vec<_>>()
                            .join("\n");
                        Ok(out)
                    }
                    _ => Ok("done is not yet supported; use add/list".to_string()),
                }
            })
        })),
    }
}

fn clipboard_stub() -> Tool {
    simple_tool!(
        "clipboard",
        "Read the clipboard (not supported headless; returns a notice).",
        EmptyInput,
        |_params: Value| {
            Ok("(clipboard access is unavailable in this environment)".to_string())
        }
    )
}
