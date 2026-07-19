---
name: aegis-crash-recovery
description: >
  Scans for and recovers incomplete Ferris Aegis durable workflows after crashes.
  Use when the user says "crash recovery", "resume workflow", "incomplete workflow",
  "recovery scan", "CrashRecovery", or "resume from checkpoint". Do NOT use for
  creating new workflows (use aegis-durable-workflow instead).
license: "MIT OR Apache-2.0"
compatibility: Requires Rust 1.82+ and the ferris-aegis-durable crate
metadata:
  aegis-crate: "ferris-aegis-durable"
  aegis-phase: "5.1"
  aegis-depends: "aegis-durable-workflow"
  aegis-invariants: "INV-013 INV-014 INV-015"
  version: "0.4.0"
  author: "ferris-aegis"
  tags: "crash recovery resume incomplete checkpoint"
allowed-tools: Bash(cargo:*) Read Write
---

# Ferris Aegis — Crash Recovery

Scan for incomplete workflows and prepare recovery after process crashes.

## When to Use

- After a process crash, find workflows that were interrupted
- Determine which step to resume from for each incomplete workflow
- Verify checkpoint integrity before resuming
- Bulk-recovery of multiple interrupted workflows

## Recovery Model

```
Process Crash
     │
     ▼
CrashRecovery.scan()
     │
     ├── Find incomplete workflows (step_index + 1 < total_steps)
     ├── Skip completed workflows
     ├── Skip failed workflows (no auto-retry)
     │
     ▼
RecoveryResult { found, recovered, details[] }
     │
     ▼
For each RecoveryDetail:
  1. Reconstruct Workflow with same WorkflowId
  2. Pass to DurableExecutor::run()
  3. Executor auto-recovers from last checkpoint
```

## Workflow

1. Create `CrashRecovery::new(store)` with the same CheckpointStore
2. Call `recovery.scan().await` to find incomplete workflows
3. For each `RecoveryDetail`, reconstruct the `Workflow` with the same `WorkflowId`
4. Pass each workflow to `DurableExecutor::run()` — it resumes from checkpoint
5. Verify each result's `WorkflowResult::status`

## Code Pattern

```rust
use ferris_aegis_durable::{
    CrashRecovery, DurableExecutor, DurableExecutorConfig,
    InMemoryCheckpointStore, Workflow, WorkflowId,
};

let store = InMemoryCheckpointStore::new();
// ... workflows were running, process crashed ...

// Scan for incomplete workflows
let recovery = CrashRecovery::new(store.clone());
let result = recovery.scan().await.unwrap();

println!("Found {} incomplete workflows", result.found);
for detail in &result.details {
    println!("  {} — resume from step {}", detail.workflow_id, detail.resume_from_step);
}

// Recover each incomplete workflow
let executor = DurableExecutor::with_defaults(store);
for detail in &result.details {
    let workflow = Workflow::with_id(&detail.workflow_name, WorkflowId::named(&detail.workflow_id))
        .add_step(/* ... rebuild steps ... */);

    let result = executor.run(&workflow).await.unwrap();
    assert!(result.is_completed());
}
```

## Code Pattern — Manual Checkpoint Inspection

```rust
let recovery = CrashRecovery::new(store);
let checkpoint = recovery.get_checkpoint("wf-1").await.unwrap();
if let Some(cp) = checkpoint {
    assert!(cp.verify_hash()); // INV-013
    println!("Workflow {} at step {}/{}", cp.workflow_id, cp.step_index + 1, cp.total_steps);
    println!("Next step: {}", cp.next_step_index());
}
```

## RecoveryResult Fields

| Field | Type | Meaning |
|-------|------|---------|
| `found` | usize | Total incomplete workflows found |
| `recovered` | usize | Successfully prepared for recovery |
| `failed` | usize | Failed to prepare (hash mismatch, etc.) |
| `details` | Vec | Per-workflow RecoveryDetail |

## Invariants

- **INV-013**: Checkpoint hash verified before recovery. Tampered checkpoints fail.
- **INV-014**: Resumes from `next_step_index()`, not from the beginning.
- **INV-015**: CrashRecovery only finds workflows that are incomplete AND not failed.

## Edge Cases

- Failed workflows (last step outcome = failure) are NOT included in recovery scan
- If checkpoint hash verification fails, recovery fails for that workflow
- Empty store → `found: 0, recovered: 0`
- Multiple crashes on same workflow → each recovery picks up from last good checkpoint
