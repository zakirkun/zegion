use std::sync::Arc;

use serde_json::json;
use zegion_core::config::SecurityConfig;
use zegion_core::hooks::ApprovalGate;
use zegion_core::tools::{build_tools, ToolContext};
use zegion_memory::{MemoryConfig, MemoryStore};

async fn make_ctx(supervised: bool, require_approval: Vec<String>) -> ToolContext {
    let memory = MemoryStore::open(MemoryConfig {
        db_path: ":memory:".into(),
        ..Default::default()
    })
    .await
    .unwrap();
    let gate = Arc::new(ApprovalGate::new(
        SecurityConfig {
            supervised,
            require_approval,
            daily_spend_cap_usd: 0.0,
        },
        None,
        "test".into(),
    ));
    ToolContext { memory, gate }
}

fn find<'a>(tools: &'a [aisdk::core::Tool], name: &str) -> &'a aisdk::core::Tool {
    tools.iter().find(|t| t.name == name).expect("tool exists")
}

#[tokio::test(flavor = "multi_thread")]
async fn calc_executes() {
    let ctx = make_ctx(true, vec![]).await;
    let tools = build_tools(&ctx);
    let out = find(&tools, "calc")
        .execute
        .call(json!({"expression": "2 + 2 * 10"}))
        .unwrap();
    assert_eq!(out, "22");
}

#[tokio::test(flavor = "multi_thread")]
async fn memory_store_then_recall() {
    let ctx = make_ctx(true, vec![]).await;
    let tools = build_tools(&ctx);

    find(&tools, "memory_store")
        .execute
        .call(json!({"fact": "the user prefers dark mode", "importance": 0.8}))
        .unwrap();

    let out = find(&tools, "memory_recall")
        .execute
        .call(json!({"query": "dark mode"}))
        .unwrap();
    assert!(out.contains("dark mode"));
}

#[tokio::test(flavor = "multi_thread")]
async fn shell_denied_without_channel_approval() {
    // Supervised with shell requiring approval, but no channel -> deny.
    let ctx = make_ctx(true, vec!["shell".into()]).await;
    let tools = build_tools(&ctx);
    let out = find(&tools, "shell")
        .execute
        .call(json!({"command": "echo hi"}))
        .unwrap();
    assert!(out.contains("denied"));
}

#[tokio::test(flavor = "multi_thread")]
async fn shell_runs_when_unsupervised() {
    let ctx = make_ctx(false, vec![]).await;
    let tools = build_tools(&ctx);
    let out = find(&tools, "shell")
        .execute
        .call(json!({"command": "echo zegion-ok"}))
        .unwrap();
    assert!(out.contains("zegion-ok"));
}
