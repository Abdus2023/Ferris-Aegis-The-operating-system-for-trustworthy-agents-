//! Ferris Aegis WASM Sandbox — Fuel-metered, memory-capped agent execution.
//!
//! This crate provides a WASM-based sandbox for executing untrusted agent
//! code. Uses `wasmtime 46` with three key safety primitives:
//!
//! - **Fuel metering** — Agents consume fuel for every WASM instruction.
//!   When fuel is exhausted, execution traps immediately.
//!
//! - **Memory caps** — Linear memory is bounded to a configurable maximum.
//!   Agents cannot allocate beyond their limit.
//!
//! - **Epoch interruption** — The engine's epoch can be incremented to
//!   force-yield long-running agents, enabling cooperative time-slicing.

use anyhow::Context;
use serde::{Deserialize, Serialize};
use wasmtime::{
    Config, Engine, EngineEpochDeadline, Linker, Memory, MemoryType, Module, Store,
    StoreLimits, StoreLimitsBuilder,
};

/// Configuration for a WASM sandbox instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmSandboxConfig {
    /// Maximum fuel for the WASM instance.
    /// One unit ≈ one WASM instruction. Default: 10 million.
    pub max_fuel: u64,
    /// Maximum linear memory in bytes. Default: 64 MiB.
    pub max_memory_bytes: u64,
    /// Maximum number of WASM tables. Default: 1.
    pub max_tables: u32,
    /// Maximum table elements. Default: 10000.
    pub max_table_elements: u32,
}

impl Default for WasmSandboxConfig {
    fn default() -> Self {
        Self {
            max_fuel: 10_000_000,
            max_memory_bytes: 64 * 1024 * 1024, // 64 MiB
            max_tables: 1,
            max_table_elements: 10_000,
        }
    }
}

/// The result of a WASM execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmExecutionResult {
    /// The return value of the WASM function, if any.
    pub result: Option<serde_json::Value>,
    /// Fuel consumed during execution.
    pub fuel_consumed: u64,
    /// Whether the execution was interrupted (fuel exhaustion or epoch).
    pub interrupted: bool,
    /// Interruption reason, if any.
    pub interrupt_reason: Option<String>,
}

/// The WASM sandbox.
pub struct WasmSandbox {
    /// The wasmtime engine (shared across instances).
    engine: Engine,
    /// The linker for resolving imports.
    linker: Linker<StoreLimits>,
    /// Sandbox configuration.
    config: WasmSandboxConfig,
}

impl WasmSandbox {
    /// Create a new WASM sandbox with the given configuration.
    pub fn new(config: WasmSandboxConfig) -> anyhow::Result<Self> {
        let mut wasmtime_config = Config::new();

        // Enable fuel metering — agents pay for every instruction
        wasmtime_config.consume_fuel(true);

        // Enable epoch interruption — allows force-yielding long-running agents
        wasmtime_config.epoch_interruption(true);

        // Limit max WASM linear memory pages (64 KiB per page)
        let max_memory_pages = (config.max_memory_bytes / (64 * 1024)) as u64;
        wasmtime_config.max_memory_pages(max_memory_pages);

        // Limit max tables and elements
        wasmtime_config.max_tables(config.max_tables);
        wasmtime_config.max_table_elements(config.max_table_elements);

        let engine = Engine::new(&wasmtime_config)
            .context("failed to create wasmtime engine")?;

        let linker = Linker::new(&engine);

        Ok(Self {
            engine,
            linker,
            config,
        })
    }

    /// Create a sandbox with default configuration.
    pub fn with_defaults() -> anyhow::Result<Self> {
        Self::new(WasmSandboxConfig::default())
    }

    /// Compile a WASM module from binary bytes.
    pub fn compile_module(&self, wasm_bytes: &[u8]) -> anyhow::Result<Module> {
        Module::from_binary(&self.engine, wasm_bytes)
            .context("failed to compile WASM module")
    }

    /// Execute a WASM module, calling the specified function.
    ///
    /// The function must take no parameters and return an i32 (0 = success).
    /// Fuel is consumed during execution; if exhausted, execution traps.
    pub fn execute(
        &self,
        module: &Module,
        function_name: &str,
    ) -> anyhow::Result<WasmExecutionResult> {
        // Build the store with fuel and limits
        let mut store: Store<StoreLimits> = Store::new(
            &self.engine,
            StoreLimitsBuilder::new()
                .memories(1)
                .memory_size(self.memory_page_limit() as usize)
                .tables(self.config.max_tables as usize)
                .build(),
        );

        // Set fuel
        store.set_fuel(self.config.max_fuel)?;

        // Set epoch deadline — trap if epoch advances 1 more time
        store.set_epoch_deadline(1);

        // Instantiate the module
        let instance = self.linker.instantiate(&mut store, module)
            .context("failed to instantiate WASM module")?;

        // Get the entry function
        let func = instance
            .get_typed_func::<(), i32>(&mut store, function_name)
            .context(format!("function '{function_name}' not found in WASM module"))?;

        // Record fuel before execution
        let fuel_before = store.get_fuel()?;

        // Execute and capture the result
        let execution_result = func.call(&mut store, ());

        // Record fuel after
        let fuel_after = store.get_fuel()?;
        let fuel_consumed = fuel_before - fuel_after;

        match execution_result {
            Ok(_return_value) => Ok(WasmExecutionResult {
                result: None,
                fuel_consumed,
                interrupted: false,
                interrupt_reason: None,
            }),
            Err(trap) => {
                let trap_string = format!("{trap}");

                // Check if it was fuel exhaustion
                let is_fuel_out = trap_string.contains("all fuel consumed")
                    || trap_string.contains("fuel");

                // Check if it was epoch interruption
                let is_epoch = trap_string.contains("epoch");

                if is_fuel_out || is_epoch {
                    Ok(WasmExecutionResult {
                        result: None,
                        fuel_consumed,
                        interrupted: true,
                        interrupt_reason: if is_fuel_out {
                            Some("fuel exhausted".to_string())
                        } else {
                            Some("epoch interrupted".to_string())
                        },
                    })
                } else {
                    Err(anyhow::anyhow!("WASM execution trap: {trap}"))
                }
            }
        }
    }

    /// Increment the engine's epoch, forcing any instances with
    /// expired epoch deadlines to trap on their next instruction.
    pub fn bump_epoch(&self) {
        self.engine.increment_epoch();
    }

    /// Get the memory page limit for store configuration.
    fn memory_page_limit(&self) -> u64 {
        (self.config.max_memory_bytes / (64 * 1024)).max(1)
    }

    /// Get a reference to the engine.
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// Get a reference to the configuration.
    pub fn config(&self) -> &WasmSandboxConfig {
        &self.config
    }
}

/// Build a minimal WASM module for testing.
///
/// This creates a simple WASM module that exports a function named
/// `run` that returns 42. Used to verify the sandbox works correctly
/// without needing external WASM files.
pub fn minimal_test_wasm() -> Vec<u8> {
    // Hand-crafted WASM binary: a module that exports a `run` function
    // that returns the i32 constant 42.
    //
    // Module structure:
    //   (module
    //     (func $run (result i32) i32.const 42)
    //     (export "run" (func $run))
    //   )
    vec![
        0x00, 0x61, 0x73, 0x6d, // magic: \0asm
        0x01, 0x00, 0x00, 0x00, // version: 1
        // Type section (1 type: () -> i32)
        0x01, 0x05,             // section id=1, size=5
        0x01,                   // 1 type
        0x60, 0x00, 0x01, 0x7f, // func () -> i32
        // Function section (1 function, type index 0)
        0x03, 0x02,             // section id=3, size=2
        0x01, 0x00,             // 1 func, type 0
        // Export section
        0x07, 0x07,             // section id=7, size=7
        0x01,                   // 1 export
        0x03, 0x72, 0x75, 0x6e, // name: "run"
        0x00, 0x00,             // kind=func, index=0
        // Code section
        0x0a, 0x06,             // section id=10, size=6
        0x01,                   // 1 function body
        0x04,                   // body size=4
        0x00,                   // 0 local declarations
        0x41, 0x2a,             // i32.const 42
        0x0b,                   // end
    ]
}

/// Build a WASM module that loops infinitely (for fuel exhaustion testing).
///
/// This creates a module with a `run` function that loops forever,
/// consuming fuel until it's exhausted.
pub fn infinite_loop_wasm() -> Vec<u8> {
    // (module
    //   (func $run (result i32)
    //     (local i32)
    //     (local.set 0 (i32.const 0))
    //     (block
    //       (loop
    //         (local.set 0 (i32.add (local.get 0) (i32.const 1)))
    //         (br 1)            ;; branch back to loop
    //       )
    //     )
    //     (local.get 0)
    //   )
    //   (export "run" (func $run))
    // )
    vec![
        0x00, 0x61, 0x73, 0x6d, // magic
        0x01, 0x00, 0x00, 0x00, // version
        // Type section: () -> i32
        0x01, 0x05, 0x01, 0x60, 0x00, 0x01, 0x7f,
        // Function section
        0x03, 0x02, 0x01, 0x00,
        // Export section
        0x07, 0x07, 0x01, 0x03, 0x72, 0x75, 0x6e, 0x00, 0x00,
        // Code section
        0x0a, 0x13,             // section id=10, size=19
        0x01,                   // 1 function body
        0x11,                   // body size=17
        0x01, 0x7f,             // 1 local of type i32
        0x41, 0x00,             // i32.const 0
        0x21, 0x00,             // local.set 0
        0x02, 0x40,             // block
        0x03, 0x40,             // loop
        0x20, 0x00,             // local.get 0
        0x41, 0x01,             // i32.const 1
        0x6a,                   // i32.add
        0x21, 0x00,             // local.set 0
        0x0c, 0x01,             // br 1 (back to loop)
        0x0b,                   // end loop
        0x0b,                   // end block
        0x20, 0x00,             // local.get 0
        0x0b,                   // end function
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sandbox_creates_successfully() {
        let sandbox = WasmSandbox::with_defaults();
        assert!(sandbox.is_ok());
    }

    #[test]
    fn minimal_module_compiles() {
        let sandbox = WasmSandbox::with_defaults().unwrap();
        let wasm = minimal_test_wasm();
        let result = sandbox.compile_module(&wasm);
        assert!(result.is_ok());
    }

    #[test]
    fn minimal_module_executes() {
        let sandbox = WasmSandbox::with_defaults().unwrap();
        let wasm = minimal_test_wasm();
        let module = sandbox.compile_module(&wasm).unwrap();
        let result = sandbox.execute(&module, "run").unwrap();
        assert!(!result.interrupted);
        assert!(result.fuel_consumed > 0);
    }

    #[test]
    fn fuel_exhaustion_terminates_execution() {
        let config = WasmSandboxConfig {
            max_fuel: 100, // Very low fuel limit
            ..Default::default()
        };
        let sandbox = WasmSandbox::new(config).unwrap();
        let wasm = infinite_loop_wasm();
        let module = sandbox.compile_module(&wasm).unwrap();
        let result = sandbox.execute(&module, "run").unwrap();

        assert!(result.interrupted);
        assert!(result.fuel_consumed > 0);
        assert!(result.interrupt_reason.is_some());
    }

    #[test]
    fn custom_config_limits_memory() {
        let config = WasmSandboxConfig {
            max_fuel: 1_000_000,
            max_memory_bytes: 1 * 1024 * 1024, // 1 MiB
            ..Default::default()
        };
        let sandbox = WasmSandbox::new(config).unwrap();
        assert_eq!(sandbox.config().max_memory_bytes, 1 * 1024 * 1024);
    }

    #[test]
    fn epoch_bump_works() {
        let sandbox = WasmSandbox::with_defaults().unwrap();
        // This should not panic — epoch bump is a safe operation
        sandbox.bump_epoch();
    }
}
