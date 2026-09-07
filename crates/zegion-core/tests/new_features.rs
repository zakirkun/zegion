use std::sync::Arc;

use zegion_core::guardrails::{default_guardrails, Direction, Guardrail, SensitiveDataGuardrail, Verdict};
use zegion_core::orchestrator::{AgentBus, AgentParticipant, Environment};
use zegion_core::sandbox::{SandboxLimits, WasmTool};
use zegion_memory::SlidingWindow;

// ---- Sliding window ----

#[test]
fn sliding_window_evicts_oldest() {
    let mut w = SlidingWindow::new(3);
    w.push("user", "one");
    w.push("assistant", "two");
    w.push("user", "three");
    w.push("assistant", "four");
    assert_eq!(w.len(), 3);
    let items: Vec<_> = w.items().collect();
    assert_eq!(items[0].1, "two");
    assert_eq!(items[2].1, "four");
}

// ---- Guardrails ----

#[test]
fn sensitive_data_guardrail_blocks_private_key() {
    let g = SensitiveDataGuardrail::default();
    let v = g.inspect("here is -----BEGIN PRIVATE KEY----- abc", Direction::Input);
    assert!(matches!(v, Verdict::Block(_)));
}

#[test]
fn guardrail_set_allows_clean_text() {
    let set = default_guardrails();
    assert!(set.check_input("hello, how are you?").is_ok());
    assert!(set.check_output("a normal helpful answer").is_ok());
}

#[test]
fn guardrail_set_blocks_secret_in_output() {
    let set = default_guardrails();
    assert!(set.check_output("your key is -----BEGIN RSA PRIVATE abc").is_err());
}

// ---- WASM sandbox ----

const WAT_ECHO: &str = r#"
(module
  (memory (export "memory") 1)
  (global $heap (mut i32) (i32.const 1024))
  (func (export "alloc") (param $n i32) (result i32)
    (local $p i32)
    (local.set $p (global.get $heap))
    (global.set $heap (i32.add (global.get $heap) (local.get $n)))
    (local.get $p))
  (func (export "run") (param $ptr i32) (param $len i32) (result i64)
    ;; echo: output ptr,len == input ptr,len
    (i64.or
      (i64.shl (i64.extend_i32_u (local.get $ptr)) (i64.const 32))
      (i64.extend_i32_u (local.get $len))))
)
"#;

#[test]
fn wasm_sandbox_echoes_input() {
    let tool = WasmTool::compile(WAT_ECHO, SandboxLimits::default()).expect("compiles");
    let out = tool.invoke("zegion-wasm-ok").expect("runs");
    assert_eq!(out, "zegion-wasm-ok");
}

const WAT_INFINITE_LOOP: &str = r#"
(module
  (memory (export "memory") 1)
  (func (export "alloc") (param i32) (result i32) (i32.const 0))
  (func (export "run") (param i32 i32) (result i64)
    (loop br 0)
    (i64.const 0))
)
"#;

#[test]
fn wasm_sandbox_fuel_cap_stops_infinite_loop() {
    let tool = WasmTool::compile(
        WAT_INFINITE_LOOP,
        SandboxLimits { fuel: 10_000, max_memory_bytes: 1024 * 1024 },
    )
    .expect("compiles");
    let result = tool.invoke("x");
    assert!(result.is_err(), "fuel-capped loop should trap");
}

// ---- Multi-agent bus ----

#[derive(Debug, Clone, PartialEq)]
struct Ping(u32);

struct Echo {
    id: String,
    got: Arc<std::sync::Mutex<Vec<u32>>>,
}

#[async_trait::async_trait]
impl AgentParticipant for Echo {
    fn id(&self) -> &str {
        &self.id
    }
    fn subscriptions(&self) -> Vec<String> {
        vec!["ping".into()]
    }
    async fn handle(
        &self,
        env: zegion_core::orchestrator::Envelope,
        _bus: AgentBus,
    ) -> zegion_core::Result<()> {
        if let Some(p) = env.downcast::<Ping>() {
            self.got.lock().unwrap().push(p.0);
        }
        Ok(())
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn bus_routes_typed_messages_to_subscribers() {
    let bus = AgentBus::new(64);
    let env = Arc::new(Environment::new(bus.clone()));
    let got = Arc::new(std::sync::Mutex::new(Vec::new()));
    let echo = Arc::new(Echo {
        id: "echo".into(),
        got: got.clone(),
    });
    env.register(echo).await;

    let runner = env.clone();
    let handle = tokio::spawn(async move { runner.run().await });

    // Let the environment register its broadcast receiver before publishing.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    bus.publish("ping", "tester", Ping(42));
    bus.publish("other", "tester", Ping(99));

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert_eq!(*got.lock().unwrap(), vec![42]);

    drop(env);
    handle.abort();
}
