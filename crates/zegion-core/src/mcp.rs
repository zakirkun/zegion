use std::sync::Arc;

use aisdk::core::tools::{Tool, ToolExecute};
use rmcp::model::{CallToolRequestParams, ClientInfo, JsonObject};
use rmcp::service::{Peer, RoleClient, ServiceExt};
use rmcp::transport::{ConfigureCommandExt, TokioChildProcess};
use serde_json::Value;

use crate::config::McpServerConfig;
use crate::error::{Error, Result};

/// A live connection to one MCP server child process.
pub struct McpConnection {
    pub name: String,
    peer: Peer<RoleClient>,
    _running: rmcp::service::RunningService<RoleClient, ClientInfo>,
}

/// Registry of all MCP connections, used to discover and call tools.
pub struct McpRegistry {
    connections: Vec<Arc<McpConnection>>,
}

impl McpRegistry {
    /// Launch every configured MCP server as a child process and handshake it.
    /// Servers that fail to start are logged and skipped so one bad server
    /// doesn't take the whole agent down.
    pub async fn connect_all(configs: &[McpServerConfig]) -> Self {
        let mut connections = Vec::new();
        for cfg in configs {
            match connect_one(cfg).await {
                Ok(conn) => {
                    tracing::info!("mcp server `{}` connected", cfg.name);
                    connections.push(Arc::new(conn));
                }
                Err(e) => {
                    tracing::warn!("mcp server `{}` failed to start: {e}", cfg.name);
                }
            }
        }
        Self { connections }
    }

    pub fn is_empty(&self) -> bool {
        self.connections.is_empty()
    }

    /// Discover every tool across all connected servers and wrap each as an
    /// aisdk `Tool`, namespaced as `<server>__<tool>`.
    pub async fn discover_tools(&self) -> Vec<Tool> {
        let mut out = Vec::new();
        for conn in &self.connections {
            match conn.peer.list_all_tools().await {
                Ok(tools) => {
                    for t in tools {
                        out.push(wrap_mcp_tool(conn.clone(), t));
                    }
                }
                Err(e) => {
                    tracing::warn!("mcp `{}` list_tools failed: {e}", conn.name);
                }
            }
        }
        out
    }
}

async fn connect_one(cfg: &McpServerConfig) -> Result<McpConnection> {
    let command = rmcp::transport::which_command(&cfg.command)
        .map_err(|e| Error::Plugin(format!("`{}` not found on PATH: {e}", cfg.command)))?
        .configure(|c| {
            c.args(&cfg.args);
        });

    // Isolate the child's stderr so it doesn't corrupt the stdio protocol stream.
    let transport = TokioChildProcess::new(command)
        .map_err(|e| Error::Plugin(format!("failed to spawn `{}`: {e}", cfg.command)))?;

    let client_info = ClientInfo::default();
    let running = client_info
        .serve(transport)
        .await
        .map_err(|e| Error::Plugin(format!("mcp handshake with `{}` failed: {e}", cfg.name)))?;

    let peer = running.peer().clone();
    Ok(McpConnection {
        name: cfg.name.clone(),
        peer,
        _running: running,
    })
}

fn wrap_mcp_tool(conn: Arc<McpConnection>, tool: rmcp::model::Tool) -> Tool {
    let namespaced = format!("{}__{}", conn.name, tool.name);
    let description = tool
        .description
        .as_deref()
        .unwrap_or("MCP tool")
        .to_string();
    let input_schema = mcp_schema_to_schemars(&tool.input_schema);

    let server_tool_name = tool.name.to_string();
    let execute = ToolExecute::new(Box::new(move |params: Value| {
        let conn = conn.clone();
        let server_tool_name = server_tool_name.clone();
        let handle = match tokio::runtime::Handle::try_current() {
            Ok(h) => h,
            Err(e) => return Err(format!("no tokio runtime for mcp tool: {e}")),
        };
        tokio::task::block_in_place(|| {
            handle.block_on(async move {
                call_mcp_tool(&conn, &server_tool_name, params).await
            })
        })
    }));

    Tool {
        name: namespaced,
        description,
        input_schema,
        execute,
    }
}

async fn call_mcp_tool(
    conn: &Arc<McpConnection>,
    tool_name: &str,
    params: Value,
) -> std::result::Result<String, String> {
    let arguments = value_to_json_object(params)?;
    let call = CallToolRequestParams::new(tool_name.to_string()).with_arguments(arguments);
    let result = conn
        .peer
        .call_tool(call)
        .await
        .map_err(|e| format!("mcp `{}`/`{tool_name}` call failed: {e}", conn.name))?;

    if result.is_error == Some(true) {
        let msg = result_text(&result);
        return Err(format!("mcp tool error: {msg}"));
    }
    Ok(result_text(&result))
}

fn result_text(result: &rmcp::model::CallToolResult) -> String {
    if let Some(structured) = &result.structured_content {
        return serde_json::to_string_pretty(structured).unwrap_or_default();
    }
    let text = result
        .content
        .iter()
        .filter_map(|c| c.as_text().map(|t| t.text.clone()))
        .collect::<Vec<_>>()
        .join("\n");
    if text.is_empty() {
        "(no output)".to_string()
    } else {
        text
    }
}

fn value_to_json_object(v: Value) -> std::result::Result<JsonObject, String> {
    match v {
        Value::Object(map) => Ok(map),
        other => serde_json::from_value(other).map_err(|e| format!("invalid tool arguments: {e}")),
    }
}

fn mcp_schema_to_schemars(schema: &JsonObject) -> schemars::Schema {
    let value = Value::Object(schema.clone());
    schemars::Schema::try_from(value).unwrap_or_else(|_| schemars::Schema::from(true))
}
