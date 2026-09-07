//! End-to-end MCP bridging test: spawns a real MCP server as a child process,
//! discovers its tools, and invokes one through the aisdk Tool wrapper.

use zegion_core::config::McpServerConfig;
use zegion_core::mcp::McpRegistry;

#[tokio::test(flavor = "multi_thread")]
async fn discovers_and_calls_mcp_tool() {
    let cfg = McpServerConfig {
        name: "everything".into(),
        command: "npx".into(),
        args: vec![
            "-y".into(),
            "@modelcontextprotocol/server-everything".into(),
        ],
    };

    let registry = McpRegistry::connect_all(&[cfg]).await;
    assert!(!registry.is_empty(), "MCP server should connect");

    let tools = registry.discover_tools().await;
    assert!(!tools.is_empty(), "should discover at least one tool");
    for t in &tools {
        assert!(
            t.name.starts_with("everything__"),
            "tool namespaced: {}",
            t.name
        );
    }

    // The reference "everything" server exposes an `echo` tool.
    let echo = tools
        .iter()
        .find(|t| t.name == "everything__echo")
        .expect("everything server exposes echo");
    let out = echo
        .execute
        .call(serde_json::json!({ "message": "zegion-mcp-ok" }))
        .expect("echo call succeeds");
    assert!(out.contains("zegion-mcp-ok"), "echo output: {out}");
}
