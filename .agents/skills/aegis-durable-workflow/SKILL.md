---
name: aegis-durable-workflow
description: >
  Creates and executes durable workflows with checkpoint persistence in Ferris Aegis.
  Use when the user says "durable execution", "checkpoint durability", "workflow steps",
  "step outcome", "DurableExecutor", or "durable workflow". Do NOT use for crash
  recovery scanning (use aegis-crash-recovery instead).
license: "MIT OR Apache-2.0"
compatibility: Requires Rust 1.82+ and the ferris-aegis-durable crate
metadata:
  aegis-crate: "ferris-aegis-durable"
  aegis-phase: "5.1"
  aegis-depends: "aegis-trust-kernel"
  aegis-invariants: "INV-013 INV-014"
  version: "0.4.0"
  author: "ferris-aegis"
  tags: "durable checkpoint workflow step execution"
allowed-tools: Bash(cargo:*) Read Write
---

# Ferris Aegis — Durable Workflow

Create and execute durable workflows with checkpoint-after-every-step guarantees.

## When to Use

- Building multi-step agent workflows that must survive crashes
- Persisting step outcomes to SQLite or in-memory store
- Chaining step outputs (step N output → step N+1 input)
- Verifying checkpoint integrity with SHA-256 hashes

## Core Model

```
Workflow = ordered sequence of Steps
Step     = name + execution function (StepFn)
Outcome  = StepOutcome (success/failure + output data + hash)
Checkpoint = snapshot of workflow state at step boundary

After each step: Checkpoint written → hash verified → next step begins
```

## Workflow

1. Define steps with `Step::new(name, |input| { ... })` or `Step::success(name, output)`
2. Compose a `Workflow::new(name).add_step(step1).add_step(step2)...`
3. Create a `CheckpointStore` (InMemoryCheckpointStore for tests, SqliteCheckpointStore for production)
4. Create a `DurableExecutor::new(store, config)`
5. Execute with `executor.run(&workflow).await`
6. Check `WorkflowResult::status` (Completed / Failed)
7. If resuming after crash: same `WorkflowId` → executor auto-recovers from checkpoint

## Code Pattern — Simple Workflow

```rust
use ferris_aegis_durable::{
    DurableExecutor, DurableExecutorConfig, Step, StepOutcome,
    InMemoryCheckpointStore, Workflow,
};

let store = InMemoryCheckpointStore::new();
let executor = DurableExecutor::with_defaults(store);

let workflow = Workflow::new("data-pipeline")
    .add_step(Step::success("fetch", serde_json::json!({"rows": 42})))
    .add_step(Step::new("transform", |input| {
        let rows = input.get("rows").and_then(|r| r.as_u64()).unwrap_or(0);
        StepOutcome::success("transform", serde_json::json!({"processed": rows * 2}))
    }))
    .add_step(Step::new("finalize", |input| {
        let processed = input.get("processed").and_then(|r| r.as_u64()).unwrap_or(0);
        StepOutcome::success("finalize", serde_json::json!({"result": processed + 6}))
    }));

let result = executor.run(&workflow).await.unwrap();
assert!(result.is_completed());
// Output: {"result": 90}  (42 * 2 + 6)
```

## Code Pattern — Crash Recovery Resume

```rust
// Use a named WorkflowId so the executor can find the checkpoint
let workflow = Workflow::with_id("recovery-workflow", WorkflowId::named("wf-1"))
    .add_step(Step::success("step-1", serde_json::json!(1)))
    .add_step(Step::success("step-2", serde_json::json!(2)))
    .add_step(Step::success("step-3", serde_json::json!(3)));

// If a checkpoint exists for "wf-1", the executor resumes from there.
// If not, execution starts from step 0.
let result = executor.run(&workflow).await.unwrap();
```

## Code Pattern — SQLite Checkpoint Store

```rust
use ferris_aegis_durable::SqliteCheckpointStore;

let store = SqliteCheckpointStore::open("aegis-checkpoints.db").await?;
// Or in-memory for testing:
let store = SqliteCheckpointStore::open_in_memory().await?;
```

## DurableExecutorConfig

| Field | Default | Purpose |
|-------|---------|---------|
| `checkpoint_enabled` | `true` | Toggle checkpoint persistence |
| `max_step_retries` | `0` | Retry transient step failures |
| `step_timeout_ms` | `0` | Per-step timeout (0 = no timeout) |
| `verify_hashes` | `true` | Verify checkpoint hashes on load |

## Invariants

- **INV-013**: Checkpoints verify content hash on load. `Checkpoint::verify_hash()` uses SHA-256.
- **INV-014**: Every step writes a checkpoint before proceeding. At most one step re-executed after crash.

## Edge Cases

- Empty workflow (0 steps) returns `DurableError::EmptyWorkflow`
- Step failure halts the workflow — `WorkflowStatus::Failed`
- Chained steps: if step N fails, step N+1 never executes
- Checkpoint hash tampering detected by `verify_hash()` returning `false`
