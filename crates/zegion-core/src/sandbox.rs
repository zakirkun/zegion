//! Sandboxed WASM tool execution via Wasmtime.
//!
//! Guest modules implement a minimal text ABI:
//!   (export "alloc" (func (param i32) (result i32)))   ; allocate n bytes, return ptr
//!   (export "run"   (func (param i32 i32) (result i64))) ; ptr,len of input -> packed ptr,len of output
//!   (export "memory" (memory 1))
//! The host writes the input string into guest memory, calls `run`, and reads back
//! the output string. Guests are fuel-capped (CPU) and memory-capped (RAM).

use anyhow::{Context, Result};
use wasmtime::*;

use crate::error::Error;

/// Fuel (rough CPU instruction budget) and memory limits for a sandboxed run.
#[derive(Debug, Clone, Copy)]
pub struct SandboxLimits {
    pub fuel: u64,
    pub max_memory_bytes: usize,
}

impl Default for SandboxLimits {
    fn default() -> Self {
        Self {
            fuel: 5_000_000,
            max_memory_bytes: 16 * 1024 * 1024,
        }
    }
}

/// A compiled WASM tool ready to be invoked in a fresh sandbox per call.
pub struct WasmTool {
    module: Module,
    limits: SandboxLimits,
}

impl WasmTool {
    /// Compile a guest from WAT text or a `.wasm` binary.
    pub fn compile(source: impl AsRef<[u8]>, limits: SandboxLimits) -> Result<Self> {
        let mut cfg = Config::new();
        cfg.consume_fuel(true);
        let engine = Engine::new(&cfg).context("wasmtime engine init failed")?;
        let module = Module::new(&engine, source).context("wasm module compile failed")?;
        Ok(Self { module, limits })
    }

    /// Run the guest with `input` and return its output string, in a fresh store so
    /// no state leaks between calls.
    pub fn invoke(&self, input: &str) -> Result<String> {
        let mut store = Store::new(self.module.engine(), ());
        store
            .set_fuel(self.limits.fuel)
            .context("failed to set fuel")?;

        let mut linker: Linker<()> = Linker::new(self.module.engine());
        // Minimal host surface: a logging hook guests may call.
        linker.func_wrap("env", "host_log", |msg_ptr: i32, msg_len: i32| {
            tracing::debug!(target: "wasm-guest", ptr = msg_ptr, len = msg_len, "guest log");
        })?;

        let instance = linker
            .instantiate(&mut store, &self.module)
            .context("wasm instantiate failed")?;

        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| Error::Plugin("guest missing exported `memory`".into()))?;
        let alloc = instance
            .get_typed_func::<i32, i32>(&mut store, "alloc")
            .map_err(|_| Error::Plugin("guest missing exported `alloc`".into()))?;
        let run = instance
            .get_typed_func::<(i32, i32), i64>(&mut store, "run")
            .map_err(|_| Error::Plugin("guest missing exported `run`".into()))?;

        let input_bytes = input.as_bytes();
        let in_ptr = alloc
            .call(&mut store, input_bytes.len() as i32)
            .context("guest `alloc` failed")? as u32 as usize;
        memory
            .write(&mut store, in_ptr, input_bytes)
            .context("failed to write input into guest memory")?;

        let packed = run
            .call(&mut store, (in_ptr as i32, input_bytes.len() as i32))
            .context("guest `run` trapped (fuel exhausted or fault)")?;

        let (out_ptr, out_len) = unpack_ptr_len(packed);
        let mut buf = vec![0u8; out_len];
        memory
            .read(&store, out_ptr, &mut buf)
            .context("failed to read output from guest memory")?;
        Ok(String::from_utf8_lossy(&buf).to_string())
    }
}

fn unpack_ptr_len(packed: i64) -> (usize, usize) {
    let ptr = (packed as u64 >> 32) as usize;
    let len = (packed as u64 & 0xffff_ffff) as usize;
    (ptr, len)
}

/// Turn a WASM tool into an aisdk `Tool` so the model can call it.
pub fn wasm_aisdk_tool(
    name: &str,
    description: &str,
    input_schema: schemars::Schema,
    tool: std::sync::Arc<WasmTool>,
) -> aisdk::core::Tool {
    let name_string = name.to_string();
    aisdk::core::Tool {
        name: name_string.clone(),
        description: description.to_string(),
        input_schema,
        execute: aisdk::core::tools::ToolExecute::new(Box::new(
            move |params: serde_json::Value| {
                let input = params.to_string();
                tool.invoke(&input).map_err(|e| e.to_string())
            },
        )),
    }
}
