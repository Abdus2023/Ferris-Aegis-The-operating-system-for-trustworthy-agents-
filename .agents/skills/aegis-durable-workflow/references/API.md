# Ferris Aegis — Durable Execution API Reference

> Loaded on demand by the aegis-durable-workflow and aegis-crash-recovery skills.

## Core Types

### `WorkflowId`
- `WorkflowId::new()` — Random UUID
- `WorkflowId::named(name)` — Deterministic name
- `workflow_id.as_str()` — Get string representation

### `Step`
- `Step::new(name, |input| { StepOutcome::success(name, output) })` — Custom step
- `Step::success(name, output)` — Always-succeeds step
- `Step::failure(name, error)` — Always-fails step

### `StepOutcome`
- `StepOutcome::success(step_name, output)` — Success with JSON output
- `StepOutcome::failure(step_name, error)` — Failure with error message
- `outcome.is_success()` / `outcome.is_failure()` — Check result
- `outcome.content_hash()` — SHA-256 hash for tamper evidence

### `Workflow`
- `Workflow::new(name)` — Create with random ID
- `Workflow::with_id(name, id)` — Create with specific ID (for recovery)
- `workflow.add_step(step)` — Add step (builder pattern)
- `workflow.step_count()` — Number of steps
- `workflow.step_names()` — Get all step names

### `Checkpoint`
- `Checkpoint::new(workflow_id, name, step_index, total_steps, outcome, outcomes)` — Create
- `checkpoint.verify_hash()` — Verify SHA-256 integrity
- `checkpoint.is_workflow_complete()` — Check if workflow is done
- `checkpoint.next_step_index()` — Get next step to execute

### `DurableExecutor`
- `DurableExecutor::new(store, config)` — Create with store
- `DurableExecutor::with_defaults(store)` — Create with default config
- `executor.run(&workflow).await` — Execute workflow (auto-recovers from checkpoint)

### `DurableExecutorConfig`
- `checkpoint_enabled: bool` (default: true)
- `max_step_retries: u32` (default: 0)
- `step_timeout_ms: u64` (default: 0 = no timeout)
- `verify_hashes: bool` (default: true)

### `CrashRecovery`
- `CrashRecovery::new(store)` — Create scanner
- `recovery.scan().await` — Find incomplete workflows
- `recovery.get_checkpoint(workflow_id).await` — Inspect specific workflow

## Checkpoint Store Implementations

### `InMemoryCheckpointStore`
- `InMemoryCheckpointStore::new()` — For testing
- All data lost on process exit

### `SqliteCheckpointStore`
- `SqliteCheckpointStore::open(path).await` — Production storage
- `SqliteCheckpointStore::open_in_memory().await` — SQLite in-memory for testing

## Error Types

- `DurableError::StepFailed { step, error }` — Step failed after retries
- `DurableError::CheckpointError(anyhow)` — Store I/O error
- `DurableError::HashVerificationFailed { workflow_id }` — Tampered checkpoint
- `DurableError::EmptyWorkflow` — Workflow has 0 steps
- `DurableError::Cancelled` — Workflow was cancelled
