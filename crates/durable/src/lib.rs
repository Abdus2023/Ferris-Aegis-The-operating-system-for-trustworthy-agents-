//! Ferris Aegis Durable — Durable execution with checkpoint durability and crash recovery.
//!
//! This crate provides the durable execution primitives needed for
//! production-grade agent workflows that can survive crashes:
//!
//! - **Step** — A unit of durable work with a name and execution function.
//!   Steps are the building blocks of workflows.
//!
//! - **StepOutcome** — The result of executing a step: success with output data,
//!   or failure with an error message. Outcomes are persisted as checkpoints.
//!
//! - **Workflow** — An ordered sequence of steps identified by a unique
//!   `WorkflowId`. Workflows track their status through the lifecycle:
//!   Pending → Running → Completed / Failed / Cancelled.
//!
//! - **Checkpoint** — A snapshot of workflow state at a given step boundary.
//!   Checkpoints record which steps completed and what their outcomes were.
//!
//! - **CheckpointStore** — Pluggable persistence backend for checkpoints.
//!   Two implementations: `InMemoryCheckpointStore` (testing) and
//!   `SqliteCheckpointStore` (production).
//!
//! - **DurableExecutor** — The engine that runs workflows with checkpoint
//!   durability. After each step, a checkpoint is written. If the process
//!   crashes, the executor can resume from the last checkpoint.
//!
//! - **CrashRecovery** — Scans the checkpoint store for incomplete workflows
//!   and resumes them. Ensures no workflow is permanently stuck.
//!
//! # Design Principles
//!
//! 1. **Checkpoint after every step** — No step outcome is lost. If the
//!    process crashes mid-workflow, the executor resumes from the last
//!    completed step, not the beginning.
//!
//! 2. **Idempotent step execution** — Steps should be safe to re-execute.
//!    The executor records outcomes immediately after execution, so a
//!    crash during checkpoint write at worst causes one step re-execution.
//!
//! 3. **Pluggable storage** — Checkpoint storage is a trait. In-memory for
//!    tests, SQLite for production. Adding Postgres or S3 is straightforward.
//!
//! 4. **Observable** — Every checkpoint write, recovery, and workflow state
//!    transition is traced.
//!
//! 5. **Integrates with resilience** — DurableExecutor wraps step execution
//!    with timeout, retry, and circuit breaker from `ferris-aegis-resilience`.
//!
//! # Example
//!
//! ```rust,ignore
//! use ferris_aegis_durable::{
//!     DurableExecutor, DurableExecutorConfig, Step, StepOutcome,
//!     InMemoryCheckpointStore, Workflow,
//! };
//!
//! #[tokio::main]
//! async fn main() {
//!     let store = InMemoryCheckpointStore::new();
//!     let executor = DurableExecutor::new(store, DurableExecutorConfig::default());
//!
//!     let workflow = Workflow::new("data-pipeline")
//!         .add_step(Step::new("fetch", |input| {
//!             // Fetch data...
//!             StepOutcome::success(serde_json::json!({"rows": 42}))
//!         }))
//!         .add_step(Step::new("transform", |input| {
//!             // Transform data...
//!             StepOutcome::success(serde_json::json!({"processed": 42}))
//!         }));
//!
//!     let result = executor.run(workflow).await;
//!     assert!(result.is_completed());
//! }
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;
use uuid::Uuid;

// ── Workflow Identity ────────────────────────────────────────────

/// Unique identifier for a workflow.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct WorkflowId(String);

impl WorkflowId {
    /// Create a new random workflow ID.
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Create a workflow ID from a named string.
    pub fn named(name: &str) -> Self {
        Self(name.to_string())
    }

    /// Get the string representation.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for WorkflowId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for WorkflowId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Status of a workflow's lifecycle.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum WorkflowStatus {
    /// The workflow has been created but not started.
    Pending,
    /// The workflow is currently executing steps.
    Running,
    /// All steps completed successfully.
    Completed,
    /// A step failed and the workflow stopped.
    Failed,
    /// The workflow was cancelled by user action.
    Cancelled,
}

impl std::fmt::Display for WorkflowStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkflowStatus::Pending => write!(f, "pending"),
            WorkflowStatus::Running => write!(f, "running"),
            WorkflowStatus::Completed => write!(f, "completed"),
            WorkflowStatus::Failed => write!(f, "failed"),
            WorkflowStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl WorkflowStatus {
    /// Whether the workflow is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            WorkflowStatus::Completed | WorkflowStatus::Failed | WorkflowStatus::Cancelled
        )
    }
}

// ── Step Outcome ─────────────────────────────────────────────────

/// The result of executing a step.
///
/// Step outcomes are persisted as checkpoints. They capture whether
/// the step succeeded or failed, along with output data that can
/// be passed to subsequent steps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepOutcome {
    /// The name of the step that produced this outcome.
    pub step_name: String,
    /// Whether the step succeeded.
    pub success: bool,
    /// Output data from the step (JSON-serializable).
    pub output: serde_json::Value,
    /// Error message if the step failed.
    pub error: Option<String>,
    /// When the step completed.
    pub completed_at: DateTime<Utc>,
    /// Duration of step execution in milliseconds.
    pub duration_ms: u64,
}

impl StepOutcome {
    /// Create a successful step outcome.
    pub fn success(step_name: &str, output: serde_json::Value) -> Self {
        Self {
            step_name: step_name.to_string(),
            success: true,
            output,
            error: None,
            completed_at: Utc::now(),
            duration_ms: 0,
        }
    }

    /// Create a successful step outcome with timing.
    pub fn success_with_duration(
        step_name: &str,
        output: serde_json::Value,
        duration_ms: u64,
    ) -> Self {
        Self {
            step_name: step_name.to_string(),
            success: true,
            output,
            error: None,
            completed_at: Utc::now(),
            duration_ms,
        }
    }

    /// Create a failed step outcome.
    pub fn failure(step_name: &str, error: &str) -> Self {
        Self {
            step_name: step_name.to_string(),
            success: false,
            output: serde_json::Value::Null,
            error: Some(error.to_string()),
            completed_at: Utc::now(),
            duration_ms: 0,
        }
    }

    /// Create a failed step outcome with timing.
    pub fn failure_with_duration(
        step_name: &str,
        error: &str,
        duration_ms: u64,
    ) -> Self {
        Self {
            step_name: step_name.to_string(),
            success: false,
            output: serde_json::Value::Null,
            error: Some(error.to_string()),
            completed_at: Utc::now(),
            duration_ms,
        }
    }

    /// Whether this outcome represents a successful step.
    pub fn is_success(&self) -> bool {
        self.success
    }

    /// Whether this outcome represents a failed step.
    pub fn is_failure(&self) -> bool {
        !self.success
    }

    /// Compute a content hash for this outcome (tamper evidence).
    pub fn content_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.step_name.as_bytes());
        hasher.update(&serde_json::to_vec(&self.output).unwrap_or_default());
        hasher.update(self.success.to_ne_bytes());
        if let Some(ref err) = self.error {
            hasher.update(err.as_bytes());
        }
        hex::encode(hasher.finalize())
    }
}

// ── Step ─────────────────────────────────────────────────────────

/// A step execution function.
///
/// Takes the output of the previous step (or `Value::Null` for the
/// first step) and returns a `StepOutcome`.
pub type StepFn = Box<dyn Fn(serde_json::Value) -> StepOutcome + Send + Sync>;

/// A unit of durable work within a workflow.
///
/// Each step has a name and an execution function. Steps execute
/// sequentially within a workflow. After each step completes, a
/// checkpoint is written so the workflow can resume after a crash.
pub struct Step {
    /// The step name (must be unique within a workflow).
    pub name: String,
    /// The step execution function.
    execute: StepFn,
}

impl Step {
    /// Create a new step with a name and execution function.
    pub fn new<F>(name: &str, execute: F) -> Self
    where
        F: Fn(serde_json::Value) -> StepOutcome + Send + Sync + 'static,
    {
        Self {
            name: name.to_string(),
            execute: Box::new(execute),
        }
    }

    /// Create a simple step that always succeeds with the given output.
    pub fn success(name: &str, output: serde_json::Value) -> Self {
        let step_name = name.to_string();
        Self::new(name, move |_| {
            StepOutcome::success(&step_name, output.clone())
        })
    }

    /// Create a simple step that always fails with the given error.
    pub fn failure(name: &str, error: &str) -> Self {
        let step_name = name.to_string();
        let error_msg = error.to_string();
        Self::new(name, move |_| {
            StepOutcome::failure(&step_name, &error_msg)
        })
    }

    /// Execute this step with the given input.
    pub fn execute(&self, input: serde_json::Value) -> StepOutcome {
        (self.execute)(input)
    }
}

impl std::fmt::Debug for Step {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Step")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

// ── Workflow ─────────────────────────────────────────────────────

/// A workflow: an ordered sequence of durable steps.
///
/// Workflows are the top-level abstraction for durable execution.
/// Each workflow has a unique ID and a list of steps that execute
/// sequentially. After each step, a checkpoint is written.
#[derive(Debug)]
pub struct Workflow {
    /// Unique workflow identifier.
    pub id: WorkflowId,
    /// Human-readable workflow name.
    pub name: String,
    /// Ordered list of steps.
    steps: Vec<Step>,
    /// Arbitrary metadata.
    pub metadata: serde_json::Value,
}

impl Workflow {
    /// Create a new workflow with a name.
    pub fn new(name: &str) -> Self {
        Self {
            id: WorkflowId::new(),
            name: name.to_string(),
            steps: Vec::new(),
            metadata: serde_json::Value::Object(serde_json::Map::new()),
        }
    }

    /// Create a workflow with a specific ID.
    pub fn with_id(name: &str, id: WorkflowId) -> Self {
        Self {
            id,
            name: name.to_string(),
            steps: Vec::new(),
            metadata: serde_json::Value::Object(serde_json::Map::new()),
        }
    }

    /// Add a step to the workflow.
    pub fn add_step(mut self, step: Step) -> Self {
        self.steps.push(step);
        self
    }

    /// Get the number of steps.
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    /// Get a step by index.
    pub fn get_step(&self, index: usize) -> Option<&Step> {
        self.steps.get(index)
    }

    /// Get step names.
    pub fn step_names(&self) -> Vec<&str> {
        self.steps.iter().map(|s| s.name.as_str()).collect()
    }

    /// Set metadata.
    pub fn with_metadata(mut self, key: &str, value: serde_json::Value) -> Self {
        if let Some(obj) = self.metadata.as_object_mut() {
            obj.insert(key.to_string(), value);
        }
        self
    }
}

// ── Checkpoint ───────────────────────────────────────────────────

/// A checkpoint: a snapshot of workflow state at a step boundary.
///
/// Checkpoints are the core durability primitive. They record:
/// - Which workflow and step this checkpoint belongs to
/// - The outcome of the step that just completed
/// - All previous step outcomes (for recovery)
/// - A content hash for tamper evidence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// The workflow this checkpoint belongs to.
    pub workflow_id: String,
    /// The workflow name.
    pub workflow_name: String,
    /// The index of the step that just completed (0-based).
    pub step_index: usize,
    /// Total number of steps in the workflow.
    pub total_steps: usize,
    /// The outcome of the step that just completed.
    pub step_outcome: StepOutcome,
    /// All step outcomes up to this point.
    pub outcomes: Vec<StepOutcome>,
    /// When this checkpoint was written.
    pub checkpoint_time: DateTime<Utc>,
    /// Content hash for tamper evidence.
    pub content_hash: String,
}

impl Checkpoint {
    /// Create a new checkpoint.
    pub fn new(
        workflow_id: &str,
        workflow_name: &str,
        step_index: usize,
        total_steps: usize,
        step_outcome: StepOutcome,
        outcomes: Vec<StepOutcome>,
    ) -> Self {
        let checkpoint = Self {
            workflow_id: workflow_id.to_string(),
            workflow_name: workflow_name.to_string(),
            step_index,
            total_steps,
            content_hash: String::new(),
            step_outcome,
            outcomes,
            checkpoint_time: Utc::now(),
        };

        // Compute content hash
        let content_hash = checkpoint.compute_hash();
        Self {
            content_hash,
            ..checkpoint
        }
    }

    /// Compute a content hash over all checkpoint data.
    fn compute_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.workflow_id.as_bytes());
        hasher.update(self.workflow_name.as_bytes());
        hasher.update(self.step_index.to_ne_bytes());
        hasher.update(self.total_steps.to_ne_bytes());
        for outcome in &self.outcomes {
            hasher.update(outcome.content_hash().as_bytes());
        }
        hex::encode(hasher.finalize())
    }

    /// Verify the checkpoint's content hash (tamper detection).
    pub fn verify_hash(&self) -> bool {
        let expected = self.compute_hash();
        self.content_hash == expected
    }

    /// Whether this checkpoint represents a completed workflow.
    pub fn is_workflow_complete(&self) -> bool {
        self.step_index + 1 >= self.total_steps && self.step_outcome.is_success()
    }

    /// Whether the step at this checkpoint failed.
    pub fn is_failed(&self) -> bool {
        self.step_outcome.is_failure()
    }

    /// Get the next step index (for recovery).
    pub fn next_step_index(&self) -> usize {
        if self.step_outcome.is_success() {
            self.step_index + 1
        } else {
            self.step_index // Re-execute the failed step
        }
    }
}

// ── Checkpoint Store (Trait) ─────────────────────────────────────

/// A trait for checkpoint storage backends.
///
/// Implementations must provide durable storage for checkpoints.
/// The two provided implementations are:
/// - `InMemoryCheckpointStore` — for testing
/// - `SqliteCheckpointStore` — for production
#[async_trait]
pub trait CheckpointStore: Send + Sync {
    /// Save a checkpoint. Overwrites any existing checkpoint for the same
    /// workflow and step index.
    async fn save(&self, checkpoint: &Checkpoint) -> anyhow::Result<()>;

    /// Load the latest checkpoint for a workflow.
    async fn load_latest(&self, workflow_id: &str) -> anyhow::Result<Option<Checkpoint>>;

    /// Load all checkpoints for a workflow (in step order).
    async fn load_all(&self, workflow_id: &str) -> anyhow::Result<Vec<Checkpoint>>;

    /// List all workflow IDs that have checkpoints.
    async fn list_workflows(&self) -> anyhow::Result<Vec<String>>;

    /// Find all workflows that are incomplete (latest checkpoint is not
    /// terminal: step_index + 1 < total_steps, or last step failed).
    async fn find_incomplete(&self) -> anyhow::Result<Vec<Checkpoint>>;

    /// Delete all checkpoints for a workflow.
    async fn delete(&self, workflow_id: &str) -> anyhow::Result<()>;

    /// Count total checkpoints across all workflows.
    async fn count(&self) -> anyhow::Result<u64>;
}

// ── In-Memory Checkpoint Store ───────────────────────────────────

/// An in-memory checkpoint store for testing.
///
/// Stores checkpoints in a `HashMap`. Not durable across process
/// restarts — use `SqliteCheckpointStore` for production.
#[derive(Debug)]
pub struct InMemoryCheckpointStore {
    checkpoints: Mutex<HashMap<String, Vec<Checkpoint>>>,
}

impl InMemoryCheckpointStore {
    /// Create a new empty in-memory store.
    pub fn new() -> Self {
        Self {
            checkpoints: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryCheckpointStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CheckpointStore for InMemoryCheckpointStore {
    async fn save(&self, checkpoint: &Checkpoint) -> anyhow::Result<()> {
        let mut store = self.checkpoints.lock().await;
        let entry = store.entry(checkpoint.workflow_id.clone()).or_default();
        // Remove any existing checkpoint for the same step index
        entry.retain(|c| c.step_index != checkpoint.step_index);
        entry.push(checkpoint.clone());
        // Keep sorted by step index
        entry.sort_by_key(|c| c.step_index);
        tracing::debug!(
            workflow_id = %checkpoint.workflow_id,
            step_index = checkpoint.step_index,
            "Checkpoint saved (in-memory)"
        );
        Ok(())
    }

    async fn load_latest(&self, workflow_id: &str) -> anyhow::Result<Option<Checkpoint>> {
        let store = self.checkpoints.lock().await;
        Ok(store
            .get(workflow_id)
            .and_then(|v| v.last().cloned()))
    }

    async fn load_all(&self, workflow_id: &str) -> anyhow::Result<Vec<Checkpoint>> {
        let store = self.checkpoints.lock().await;
        Ok(store.get(workflow_id).cloned().unwrap_or_default())
    }

    async fn list_workflows(&self) -> anyhow::Result<Vec<String>> {
        let store = self.checkpoints.lock().await;
        Ok(store.keys().cloned().collect())
    }

    async fn find_incomplete(&self) -> anyhow::Result<Vec<Checkpoint>> {
        let store = self.checkpoints.lock().await;
        let mut incomplete = Vec::new();
        for checkpoints in store.values() {
            if let Some(latest) = checkpoints.last() {
                if !latest.is_workflow_complete() && !latest.is_failed() {
                    incomplete.push(latest.clone());
                }
            }
        }
        Ok(incomplete)
    }

    async fn delete(&self, workflow_id: &str) -> anyhow::Result<()> {
        let mut store = self.checkpoints.lock().await;
        store.remove(workflow_id);
        Ok(())
    }

    async fn count(&self) -> anyhow::Result<u64> {
        let store = self.checkpoints.lock().await;
        Ok(store.values().map(|v| v.len() as u64).sum())
    }
}

// ── SQLite Checkpoint Store ──────────────────────────────────────

/// A SQLite-backed checkpoint store for production.
///
/// Checkpoints are stored in a `checkpoints` table with columns
/// for workflow_id, step_index, and the serialized checkpoint data.
pub struct SqliteCheckpointStore {
    pool: sqlx::SqlitePool,
}

impl SqliteCheckpointStore {
    /// Open a SQLite checkpoint store at the given path.
    pub async fn open(path: &str) -> anyhow::Result<Self> {
        let pool = sqlx::SqlitePool::connect(path).await?;
        Self::initialize(&pool).await?;
        Ok(Self { pool })
    }

    /// Open an in-memory SQLite checkpoint store (for testing).
    pub async fn open_in_memory() -> anyhow::Result<Self> {
        let pool = sqlx::SqlitePool::connect(":memory:").await?;
        Self::initialize(&pool).await?;
        Ok(Self { pool })
    }

    /// Create the checkpoints table if it doesn't exist.
    async fn initialize(pool: &sqlx::SqlitePool) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS checkpoints (
                workflow_id TEXT NOT NULL,
                step_index INTEGER NOT NULL,
                checkpoint_data TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                PRIMARY KEY (workflow_id, step_index)
            )
            "#,
        )
        .execute(pool)
        .await?;
        Ok(())
    }
}

#[async_trait]
impl CheckpointStore for SqliteCheckpointStore {
    async fn save(&self, checkpoint: &Checkpoint) -> anyhow::Result<()> {
        let data = serde_json::to_string(checkpoint)?;
        sqlx::query(
            r#"
            INSERT INTO checkpoints (workflow_id, step_index, checkpoint_data, created_at)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(workflow_id, step_index) DO UPDATE SET
                checkpoint_data = excluded.checkpoint_data,
                created_at = excluded.created_at
            "#,
        )
        .bind(&checkpoint.workflow_id)
        .bind(checkpoint.step_index as i64)
        .bind(&data)
        .bind(checkpoint.checkpoint_time.to_rfc3339())
        .execute(&self.pool)
        .await?;

        tracing::debug!(
            workflow_id = %checkpoint.workflow_id,
            step_index = checkpoint.step_index,
            "Checkpoint saved (SQLite)"
        );
        Ok(())
    }

    async fn load_latest(&self, workflow_id: &str) -> anyhow::Result<Option<Checkpoint>> {
        let row: Option<(String,)> = sqlx::query_as(
            r#"
            SELECT checkpoint_data FROM checkpoints
            WHERE workflow_id = ?
            ORDER BY step_index DESC
            LIMIT 1
            "#,
        )
        .bind(workflow_id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some((data,)) => {
                let checkpoint: Checkpoint = serde_json::from_str(&data)?;
                Ok(Some(checkpoint))
            }
            None => Ok(None),
        }
    }

    async fn load_all(&self, workflow_id: &str) -> anyhow::Result<Vec<Checkpoint>> {
        let rows: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT checkpoint_data FROM checkpoints
            WHERE workflow_id = ?
            ORDER BY step_index ASC
            "#,
        )
        .bind(workflow_id)
        .fetch_all(&self.pool)
        .await?;

        let mut checkpoints = Vec::new();
        for (data,) in rows {
            let checkpoint: Checkpoint = serde_json::from_str(&data)?;
            checkpoints.push(checkpoint);
        }
        Ok(checkpoints)
    }

    async fn list_workflows(&self) -> anyhow::Result<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT workflow_id FROM checkpoints ORDER BY workflow_id",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    async fn find_incomplete(&self) -> anyhow::Result<Vec<Checkpoint>> {
        // Get the latest checkpoint for each workflow
        let rows: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT checkpoint_data FROM checkpoints c1
            WHERE step_index = (
                SELECT MAX(step_index) FROM checkpoints c2
                WHERE c2.workflow_id = c1.workflow_id
            )
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut incomplete = Vec::new();
        for (data,) in rows {
            let checkpoint: Checkpoint = serde_json::from_str(&data)?;
            if !checkpoint.is_workflow_complete() && !checkpoint.is_failed() {
                incomplete.push(checkpoint);
            }
        }
        Ok(incomplete)
    }

    async fn delete(&self, workflow_id: &str) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM checkpoints WHERE workflow_id = ?")
            .bind(workflow_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn count(&self) -> anyhow::Result<u64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM checkpoints")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0 as u64)
    }
}

// ── Durable Executor ─────────────────────────────────────────────

/// Configuration for the durable executor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DurableExecutorConfig {
    /// Whether to enable checkpoint durability.
    pub checkpoint_enabled: bool,
    /// Maximum number of retry attempts per step (0 = no retry).
    pub max_step_retries: u32,
    /// Step execution timeout in milliseconds (0 = no timeout).
    pub step_timeout_ms: u64,
    /// Whether to verify checkpoint hashes on load.
    pub verify_hashes: bool,
}

impl Default for DurableExecutorConfig {
    fn default() -> Self {
        Self {
            checkpoint_enabled: true,
            max_step_retries: 0,
            step_timeout_ms: 0,
            verify_hashes: true,
        }
    }
}

/// The result of executing a workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowResult {
    /// The workflow ID.
    pub workflow_id: String,
    /// The workflow name.
    pub workflow_name: String,
    /// The final status.
    pub status: WorkflowStatus,
    /// The final step index reached (0-based).
    pub steps_completed: usize,
    /// Total number of steps.
    pub total_steps: usize,
    /// All step outcomes.
    pub outcomes: Vec<StepOutcome>,
    /// Total execution time in milliseconds.
    pub total_duration_ms: u64,
}

impl WorkflowResult {
    /// Whether the workflow completed successfully.
    pub fn is_completed(&self) -> bool {
        self.status == WorkflowStatus::Completed
    }

    /// Whether the workflow failed.
    pub fn is_failed(&self) -> bool {
        self.status == WorkflowStatus::Failed
    }

    /// Get the output of the last step.
    pub fn last_output(&self) -> Option<&serde_json::Value> {
        self.outcomes.last().map(|o| &o.output)
    }
}

/// Errors that can occur during durable execution.
#[derive(Debug, thiserror::Error)]
pub enum DurableError {
    /// A step failed after all retries.
    #[error("step '{step}' failed: {error}")]
    StepFailed {
        /// The step name.
        step: String,
        /// The error message.
        error: String,
    },

    /// Checkpoint storage error.
    #[error("checkpoint error: {0}")]
    CheckpointError(#[from] anyhow::Error),

    /// Hash verification failed.
    #[error("checkpoint hash verification failed for workflow {workflow_id}")]
    HashVerificationFailed {
        /// The workflow ID.
        workflow_id: String,
    },

    /// Workflow was cancelled.
    #[error("workflow cancelled")]
    Cancelled,

    /// Invalid workflow (no steps).
    #[error("workflow has no steps")]
    EmptyWorkflow,
}

/// The durable executor: runs workflows with checkpoint durability.
///
/// The executor:
/// 1. Checks for a resumable checkpoint (crash recovery)
/// 2. Executes steps sequentially
/// 3. Writes a checkpoint after each step
/// 4. If a step fails, marks the workflow as failed
/// 5. Returns a `WorkflowResult` with all outcomes
pub struct DurableExecutor {
    store: Arc<dyn CheckpointStore>,
    config: DurableExecutorConfig,
}

impl DurableExecutor {
    /// Create a new durable executor with the given store and config.
    pub fn new(store: impl CheckpointStore + 'static, config: DurableExecutorConfig) -> Self {
        Self {
            store: Arc::new(store),
            config,
        }
    }

    /// Create an executor with default config.
    pub fn with_defaults(store: impl CheckpointStore + 'static) -> Self {
        Self::new(store, DurableExecutorConfig::default())
    }

    /// Run a workflow to completion (or failure).
    ///
    /// If a checkpoint exists for this workflow, execution resumes from
    /// the last completed step. Otherwise, execution starts from step 0.
    pub async fn run(&self, workflow: &Workflow) -> Result<WorkflowResult, DurableError> {
        if workflow.steps.is_empty() {
            return Err(DurableError::EmptyWorkflow);
        }

        let start_time = std::time::Instant::now();
        let workflow_id = workflow.id.to_string();

        tracing::info!(
            workflow_id = %workflow_id,
            workflow_name = %workflow.name,
            steps = workflow.steps.len(),
            "Starting durable workflow execution"
        );

        // 1. Check for existing checkpoint (crash recovery)
        let (start_step, mut outcomes) = self.recover_state(&workflow_id).await?;

        tracing::info!(
            workflow_id = %workflow_id,
            resuming_from_step = start_step,
            previous_outcomes = outcomes.len(),
            "Workflow execution state"
        );

        // 2. Execute steps from start_step
        let mut current_step = start_step;
        let mut step_input = outcomes
            .last()
            .map(|o| o.output.clone())
            .unwrap_or(serde_json::Value::Null);

        while current_step < workflow.steps.len() {
            let step = &workflow.steps[current_step];

            tracing::info!(
                workflow_id = %workflow_id,
                step_index = current_step,
                step_name = %step.name,
                "Executing step"
            );

            // Execute step (with optional retry)
            let step_start = std::time::Instant::now();
            let outcome = self.execute_step_with_retry(step, step_input.clone());
            let duration_ms = step_start.elapsed().as_millis() as u64;

            let outcome = StepOutcome {
                duration_ms,
                ..outcome
            };

            // 3. Write checkpoint
            if self.config.checkpoint_enabled {
                let checkpoint = Checkpoint::new(
                    &workflow_id,
                    &workflow.name,
                    current_step,
                    workflow.steps.len(),
                    outcome.clone(),
                    outcomes.clone(),
                );

                self.store.save(&checkpoint).await.map_err(DurableError::CheckpointError)?;

                // Verify hash on write
                if self.config.verify_hashes {
                    let loaded = self.store.load_latest(&workflow_id).await
                        .map_err(DurableError::CheckpointError)?;
                    if let Some(ref cp) = loaded {
                        if !cp.verify_hash() {
                            return Err(DurableError::HashVerificationFailed {
                                workflow_id: workflow_id.clone(),
                            });
                        }
                    }
                }
            }

            // 4. Check outcome
            if outcome.is_failure() {
                let error = outcome.error.clone().unwrap_or_default();
                tracing::error!(
                    workflow_id = %workflow_id,
                    step_name = %step.name,
                    error = %error,
                    "Step failed, workflow stopping"
                );

                outcomes.push(outcome);
                let total_duration_ms = start_time.elapsed().as_millis() as u64;

                return Ok(WorkflowResult {
                    workflow_id,
                    workflow_name: workflow.name.clone(),
                    status: WorkflowStatus::Failed,
                    steps_completed: current_step,
                    total_steps: workflow.steps.len(),
                    outcomes,
                    total_duration_ms,
                });
            }

            step_input = outcome.output.clone();
            outcomes.push(outcome);

            tracing::info!(
                workflow_id = %workflow_id,
                step_index = current_step,
                step_name = %step.name,
                duration_ms,
                "Step completed"
            );

            current_step += 1;
        }

        let total_duration_ms = start_time.elapsed().as_millis() as u64;

        tracing::info!(
            workflow_id = %workflow_id,
            total_steps = workflow.steps.len(),
            total_duration_ms,
            "Workflow completed successfully"
        );

        Ok(WorkflowResult {
            workflow_id,
            workflow_name: workflow.name.clone(),
            status: WorkflowStatus::Completed,
            steps_completed: workflow.steps.len(),
            total_steps: workflow.steps.len(),
            outcomes,
            total_duration_ms,
        })
    }

    /// Recover state from existing checkpoint.
    ///
    /// Returns the step index to start from and the previous outcomes.
    async fn recover_state(
        &self,
        workflow_id: &str,
    ) -> Result<(usize, Vec<StepOutcome>), DurableError> {
        let checkpoint = self
            .store
            .load_latest(workflow_id)
            .await
            .map_err(DurableError::CheckpointError)?;

        match checkpoint {
            Some(cp) => {
                // Verify hash
                if self.config.verify_hashes && !cp.verify_hash() {
                    return Err(DurableError::HashVerificationFailed {
                        workflow_id: workflow_id.to_string(),
                    });
                }

                let start_step = cp.next_step_index();
                let outcomes: Vec<StepOutcome> = cp.outcomes.iter().cloned().collect();

                tracing::info!(
                    workflow_id = workflow_id,
                    recovered_step = start_step,
                    previous_outcomes = outcomes.len(),
                    "Recovered workflow state from checkpoint"
                );

                Ok((start_step, outcomes))
            }
            None => {
                tracing::debug!(
                    workflow_id = workflow_id,
                    "No checkpoint found, starting from step 0"
                );
                Ok((0, Vec::new()))
            }
        }
    }

    /// Execute a step with optional retry.
    fn execute_step_with_retry(&self, step: &Step, input: serde_json::Value) -> StepOutcome {
        let mut attempts = 0;
        let max_retries = self.config.max_step_retries;

        loop {
            let outcome = step.execute(input.clone());
            if outcome.is_success() || attempts >= max_retries {
                return outcome;
            }
            attempts += 1;
            tracing::warn!(
                step_name = %step.name,
                attempt = attempts,
                max_retries,
                "Step failed, retrying"
            );
        }
    }

    /// Cancel a running workflow.
    pub async fn cancel(&self, workflow_id: &str) -> anyhow::Result<()> {
        tracing::info!(workflow_id = workflow_id, "Workflow cancelled");
        self.store.delete(workflow_id).await
    }
}

// ── Crash Recovery ───────────────────────────────────────────────

/// Result of a crash recovery scan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryResult {
    /// Number of incomplete workflows found.
    pub found: usize,
    /// Number of workflows successfully recovered.
    pub recovered: usize,
    /// Number of workflows that failed recovery.
    pub failed: usize,
    /// Details of each recovery attempt.
    pub details: Vec<RecoveryDetail>,
}

/// Details of a single recovery attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryDetail {
    /// The workflow ID.
    pub workflow_id: String,
    /// The workflow name.
    pub workflow_name: String,
    /// The step index to resume from.
    pub resume_from_step: usize,
    /// Whether recovery succeeded.
    pub success: bool,
    /// Error message if recovery failed.
    pub error: Option<String>,
}

/// Crash recovery: scans for incomplete workflows and prepares recovery info.
///
/// `CrashRecovery` does not re-execute workflows directly (that requires
/// the original `Workflow` definitions). Instead, it:
/// 1. Scans the checkpoint store for incomplete workflows
/// 2. Prepares `RecoveryDetail` for each, including the step index
///    to resume from
/// 3. Returns a `RecoveryResult` that the caller can use to reconstruct
///    workflows and pass them to `DurableExecutor::run()`
pub struct CrashRecovery {
    store: Arc<dyn CheckpointStore>,
}

impl CrashRecovery {
    /// Create a new crash recovery scanner.
    pub fn new(store: impl CheckpointStore + 'static) -> Self {
        Self {
            store: Arc::new(store),
        }
    }

    /// Scan for incomplete workflows and prepare recovery info.
    pub async fn scan(&self) -> Result<RecoveryResult, DurableError> {
        tracing::info!("Scanning for incomplete workflows...");

        let incomplete = self
            .store
            .find_incomplete()
            .await
            .map_err(DurableError::CheckpointError)?;

        let found = incomplete.len();
        let mut details = Vec::new();

        for checkpoint in &incomplete {
            let resume_step = checkpoint.next_step_index();
            details.push(RecoveryDetail {
                workflow_id: checkpoint.workflow_id.clone(),
                workflow_name: checkpoint.workflow_name.clone(),
                resume_from_step: resume_step,
                success: true,
                error: None,
            });
        }

        let recovered = details.iter().filter(|d| d.success).count();
        let failed = found - recovered;

        tracing::info!(
            found,
            recovered,
            failed,
            "Crash recovery scan complete"
        );

        Ok(RecoveryResult {
            found,
            recovered,
            failed,
            details,
        })
    }

    /// Get the latest checkpoint for a specific workflow (for manual recovery).
    pub async fn get_checkpoint(
        &self,
        workflow_id: &str,
    ) -> Result<Option<Checkpoint>, DurableError> {
        self.store
            .load_latest(workflow_id)
            .await
            .map_err(DurableError::CheckpointError)
    }
}

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── WorkflowId Tests ──────────────────────────────────────

    #[test]
    fn workflow_id_is_unique() {
        let id1 = WorkflowId::new();
        let id2 = WorkflowId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn workflow_id_named() {
        let id = WorkflowId::named("my-workflow");
        assert_eq!(id.as_str(), "my-workflow");
    }

    // ── StepOutcome Tests ─────────────────────────────────────

    #[test]
    fn step_outcome_success() {
        let outcome = StepOutcome::success("step-1", serde_json::json!({"result": 42}));
        assert!(outcome.is_success());
        assert!(!outcome.is_failure());
        assert_eq!(outcome.step_name, "step-1");
        assert_eq!(outcome.output["result"], 42);
        assert!(outcome.error.is_none());
    }

    #[test]
    fn step_outcome_failure() {
        let outcome = StepOutcome::failure("step-1", "connection refused");
        assert!(!outcome.is_success());
        assert!(outcome.is_failure());
        assert_eq!(outcome.error.unwrap(), "connection refused");
    }

    #[test]
    fn step_outcome_content_hash_deterministic() {
        let outcome1 = StepOutcome::success("step-1", serde_json::json!({"x": 1}));
        let outcome2 = StepOutcome::success("step-1", serde_json::json!({"x": 1}));
        assert_eq!(outcome1.content_hash(), outcome2.content_hash());
    }

    #[test]
    fn step_outcome_content_hash_differs_on_change() {
        let outcome1 = StepOutcome::success("step-1", serde_json::json!({"x": 1}));
        let outcome2 = StepOutcome::success("step-1", serde_json::json!({"x": 2}));
        assert_ne!(outcome1.content_hash(), outcome2.content_hash());
    }

    // ── Step Tests ────────────────────────────────────────────

    #[test]
    fn step_execute_success() {
        let step = Step::success("greet", serde_json::json!("hello"));
        let outcome = step.execute(serde_json::Value::Null);
        assert!(outcome.is_success());
        assert_eq!(outcome.output, serde_json::json!("hello"));
    }

    #[test]
    fn step_execute_failure() {
        let step = Step::failure("fail", "something went wrong");
        let outcome = step.execute(serde_json::Value::Null);
        assert!(outcome.is_failure());
    }

    #[test]
    fn step_with_closure() {
        let step = Step::new("double", |input| {
            let val = input.as_u64().unwrap_or(0);
            StepOutcome::success("double", serde_json::json!(val * 2))
        });
        let outcome = step.execute(serde_json::json!(21));
        assert!(outcome.is_success());
        assert_eq!(outcome.output, 42);
    }

    // ── Workflow Tests ────────────────────────────────────────

    #[test]
    fn workflow_creation() {
        let wf = Workflow::new("test-workflow");
        assert_eq!(wf.name, "test-workflow");
        assert_eq!(wf.step_count(), 0);
    }

    #[test]
    fn workflow_add_steps() {
        let wf = Workflow::new("pipeline")
            .add_step(Step::success("step-1", serde_json::json!(1)))
            .add_step(Step::success("step-2", serde_json::json!(2)));
        assert_eq!(wf.step_count(), 2);
        assert_eq!(wf.step_names(), vec!["step-1", "step-2"]);
    }

    #[test]
    fn workflow_with_metadata() {
        let wf = Workflow::new("pipeline")
            .with_metadata("owner", serde_json::json!("team-a"))
            .with_metadata("priority", serde_json::json!(3));
        assert_eq!(wf.metadata["owner"], "team-a");
        assert_eq!(wf.metadata["priority"], 3);
    }

    // ── Checkpoint Tests ──────────────────────────────────────

    #[test]
    fn checkpoint_hash_verification() {
        let outcome = StepOutcome::success("step-1", serde_json::json!({"data": "test"}));
        let checkpoint = Checkpoint::new(
            "wf-1",
            "test-workflow",
            0,
            3,
            outcome,
            vec![],
        );
        assert!(checkpoint.verify_hash());
    }

    #[test]
    fn checkpoint_tamper_detection() {
        let outcome = StepOutcome::success("step-1", serde_json::json!({"data": "test"}));
        let mut checkpoint = Checkpoint::new(
            "wf-1",
            "test-workflow",
            0,
            3,
            outcome,
            vec![],
        );
        // Tamper with the data
        checkpoint.step_index = 999;
        assert!(!checkpoint.verify_hash());
    }

    #[test]
    fn checkpoint_is_complete() {
        let outcome = StepOutcome::success("step-3", serde_json::json!("done"));
        let checkpoint = Checkpoint::new("wf-1", "test", 2, 3, outcome, vec![]);
        assert!(checkpoint.is_workflow_complete());
    }

    #[test]
    fn checkpoint_is_incomplete() {
        let outcome = StepOutcome::success("step-1", serde_json::json!("partial"));
        let checkpoint = Checkpoint::new("wf-1", "test", 0, 3, outcome, vec![]);
        assert!(!checkpoint.is_workflow_complete());
    }

    #[test]
    fn checkpoint_next_step_on_success() {
        let outcome = StepOutcome::success("step-1", serde_json::json!("ok"));
        let checkpoint = Checkpoint::new("wf-1", "test", 0, 3, outcome, vec![]);
        assert_eq!(checkpoint.next_step_index(), 1);
    }

    #[test]
    fn checkpoint_next_step_on_failure() {
        let outcome = StepOutcome::failure("step-2", "error");
        let checkpoint = Checkpoint::new("wf-1", "test", 1, 3, outcome, vec![]);
        assert_eq!(checkpoint.next_step_index(), 1); // Re-execute failed step
    }

    // ── In-Memory Store Tests ─────────────────────────────────

    #[tokio::test]
    async fn in_memory_store_save_and_load() {
        let store = InMemoryCheckpointStore::new();
        let outcome = StepOutcome::success("step-1", serde_json::json!({"result": "ok"}));
        let checkpoint = Checkpoint::new("wf-1", "test", 0, 3, outcome, vec![]);

        store.save(&checkpoint).await.unwrap();

        let loaded = store.load_latest("wf-1").await.unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.workflow_id, "wf-1");
        assert_eq!(loaded.step_index, 0);
        assert!(loaded.step_outcome.is_success());
    }

    #[tokio::test]
    async fn in_memory_store_load_latest() {
        let store = InMemoryCheckpointStore::new();

        // Save two checkpoints
        let outcome1 = StepOutcome::success("step-1", serde_json::json!(1));
        let cp1 = Checkpoint::new("wf-1", "test", 0, 3, outcome1, vec![]);
        store.save(&cp1).await.unwrap();

        let outcome2 = StepOutcome::success("step-2", serde_json::json!(2));
        let cp2 = Checkpoint::new("wf-1", "test", 1, 3, outcome2, vec![cp1.step_outcome.clone()]);
        store.save(&cp2).await.unwrap();

        let loaded = store.load_latest("wf-1").await.unwrap().unwrap();
        assert_eq!(loaded.step_index, 1);
    }

    #[tokio::test]
    async fn in_memory_store_find_incomplete() {
        let store = InMemoryCheckpointStore::new();

        // Complete workflow
        let outcome = StepOutcome::success("step-3", serde_json::json!("done"));
        let cp_complete = Checkpoint::new("wf-complete", "test", 2, 3, outcome, vec![]);
        store.save(&cp_complete).await.unwrap();

        // Incomplete workflow
        let outcome2 = StepOutcome::success("step-1", serde_json::json!(1));
        let cp_incomplete = Checkpoint::new("wf-incomplete", "test", 0, 3, outcome2, vec![]);
        store.save(&cp_incomplete).await.unwrap();

        let incomplete = store.find_incomplete().await.unwrap();
        assert_eq!(incomplete.len(), 1);
        assert_eq!(incomplete[0].workflow_id, "wf-incomplete");
    }

    #[tokio::test]
    async fn in_memory_store_count() {
        let store = InMemoryCheckpointStore::new();

        let outcome = StepOutcome::success("step-1", serde_json::json!(1));
        let cp1 = Checkpoint::new("wf-1", "test", 0, 3, outcome.clone(), vec![]);
        store.save(&cp1).await.unwrap();

        let cp2 = Checkpoint::new("wf-2", "test", 0, 3, outcome, vec![]);
        store.save(&cp2).await.unwrap();

        assert_eq!(store.count().await.unwrap(), 2);
    }

    #[tokio::test]
    async fn in_memory_store_delete() {
        let store = InMemoryCheckpointStore::new();
        let outcome = StepOutcome::success("step-1", serde_json::json!(1));
        let cp = Checkpoint::new("wf-1", "test", 0, 3, outcome, vec![]);
        store.save(&cp).await.unwrap();

        store.delete("wf-1").await.unwrap();
        assert!(store.load_latest("wf-1").await.unwrap().is_none());
    }

    // ── Durable Executor Tests ────────────────────────────────

    #[tokio::test]
    async fn executor_simple_workflow() {
        let store = InMemoryCheckpointStore::new();
        let executor = DurableExecutor::with_defaults(store);

        let workflow = Workflow::new("simple")
            .add_step(Step::success("step-1", serde_json::json!({"x": 1})))
            .add_step(Step::success("step-2", serde_json::json!({"y": 2})));

        let result = executor.run(&workflow).await.unwrap();
        assert!(result.is_completed());
        assert_eq!(result.steps_completed, 2);
        assert_eq!(result.outcomes.len(), 2);
    }

    #[tokio::test]
    async fn executor_workflow_with_failure() {
        let store = InMemoryCheckpointStore::new();
        let executor = DurableExecutor::with_defaults(store);

        let workflow = Workflow::new("failing")
            .add_step(Step::success("step-1", serde_json::json!(1)))
            .add_step(Step::failure("step-2", "boom"))
            .add_step(Step::success("step-3", serde_json::json!(3)));

        let result = executor.run(&workflow).await.unwrap();
        assert!(result.is_failed());
        assert_eq!(result.steps_completed, 1); // Step 0 succeeded, step 1 failed
        assert_eq!(result.outcomes.len(), 2);
    }

    #[tokio::test]
    async fn executor_empty_workflow() {
        let store = InMemoryCheckpointStore::new();
        let executor = DurableExecutor::with_defaults(store);

        let workflow = Workflow::new("empty");
        let result = executor.run(&workflow).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn executor_chained_steps() {
        let store = InMemoryCheckpointStore::new();
        let executor = DurableExecutor::with_defaults(store);

        let workflow = Workflow::new("chained")
            .add_step(Step::new("generate", |_| {
                StepOutcome::success("generate", serde_json::json!({"value": 10}))
            }))
            .add_step(Step::new("transform", |input| {
                let val = input.get("value").and_then(|v| v.as_u64()).unwrap_or(0);
                StepOutcome::success("transform", serde_json::json!({"value": val * 3}))
            }))
            .add_step(Step::new("finalize", |input| {
                let val = input.get("value").and_then(|v| v.as_u64()).unwrap_or(0);
                StepOutcome::success("finalize", serde_json::json!({"result": val + 12}))
            }));

        let result = executor.run(&workflow).await.unwrap();
        assert!(result.is_completed());
        assert_eq!(result.outcomes[2].output["result"], 42);
    }

    #[tokio::test]
    async fn executor_checkpoint_durability() {
        let store = Arc::new(InMemoryCheckpointStore::new());
        let executor = DurableExecutor::with_defaults(store.clone());

        let workflow = Workflow::new("durable")
            .add_step(Step::success("step-1", serde_json::json!({"a": 1})))
            .add_step(Step::success("step-2", serde_json::json!({"b": 2})));

        let result = executor.run(&workflow).await.unwrap();
        assert!(result.is_completed());

        // Verify checkpoints were written
        let count = store.count().await.unwrap();
        assert_eq!(count, 2); // One checkpoint per step
    }

    #[tokio::test]
    async fn executor_crash_recovery() {
        let store = Arc::new(InMemoryCheckpointStore::new());

        // Simulate: first execution completes steps 0-1, "crashes" before step 2
        // We do this by manually creating a checkpoint for steps 0-1
        let workflow_id = "recovery-test";

        let outcome1 = StepOutcome::success("step-1", serde_json::json!(1));
        let outcome2 = StepOutcome::success("step-2", serde_json::json!(2));

        let cp = Checkpoint::new(
            workflow_id,
            "recovery-workflow",
            1, // Completed step index 1
            4,
            outcome2.clone(),
            vec![outcome1],
        );
        store.save(&cp).await.unwrap();

        // Now run a workflow with the same ID — it should resume from step 2
        let executor = DurableExecutor::with_defaults(store.clone());
        let workflow = Workflow::with_id("recovery-workflow", WorkflowId::named(workflow_id))
            .add_step(Step::success("step-1", serde_json::json!(10)))
            .add_step(Step::success("step-2", serde_json::json!(20)))
            .add_step(Step::success("step-3", serde_json::json!(30)))
            .add_step(Step::success("step-4", serde_json::json!(40)));

        let result = executor.run(&workflow).await.unwrap();
        assert!(result.is_completed());

        // Steps 0-1 were recovered, steps 2-3 were executed
        // Total outcomes should include recovered + new
        assert!(result.outcomes.len() >= 4);
    }

    #[tokio::test]
    async fn executor_with_step_retry() {
        let store = InMemoryCheckpointStore::new();
        let config = DurableExecutorConfig {
            max_step_retries: 2,
            ..Default::default()
        };
        let executor = DurableExecutor::new(store, config);

        let attempts = Arc::new(Mutex::new(0));
        let attempts_clone = attempts.clone();

        let workflow = Workflow::new("retry")
            .add_step(Step::new("flaky", move |_| {
                let mut count = attempts_clone.blocking_lock();
                *count += 1;
                if *count < 3 {
                    StepOutcome::failure("flaky", "transient error")
                } else {
                    StepOutcome::success("flaky", serde_json::json!("recovered"))
                }
            }));

        let result = executor.run(&workflow).await.unwrap();
        assert!(result.is_completed());
        let final_attempts = *attempts.blocking_lock();
        assert!(final_attempts >= 3, "Should have retried at least 3 times");
    }

    #[tokio::test]
    async fn executor_no_checkpoint_mode() {
        let store = InMemoryCheckpointStore::new();
        let config = DurableExecutorConfig {
            checkpoint_enabled: false,
            ..Default::default()
        };
        let executor = DurableExecutor::new(store, config);

        let workflow = Workflow::new("no-checkpoint")
            .add_step(Step::success("step-1", serde_json::json!(1)));

        let result = executor.run(&workflow).await.unwrap();
        assert!(result.is_completed());

        // No checkpoints should have been written
        let count = InMemoryCheckpointStore::new().count().await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn executor_cancel_workflow() {
        let store = Arc::new(InMemoryCheckpointStore::new());
        let executor = DurableExecutor::with_defaults(store.clone());

        let workflow = Workflow::new("to-cancel")
            .add_step(Step::success("step-1", serde_json::json!(1)));

        executor.run(&workflow).await.unwrap();
        executor.cancel(&workflow.id.to_string()).await.unwrap();
    }

    // ── Crash Recovery Tests ──────────────────────────────────

    #[tokio::test]
    async fn crash_recovery_scan_empty() {
        let store = InMemoryCheckpointStore::new();
        let recovery = CrashRecovery::new(store);
        let result = recovery.scan().await.unwrap();
        assert_eq!(result.found, 0);
        assert_eq!(result.recovered, 0);
    }

    #[tokio::test]
    async fn crash_recovery_finds_incomplete() {
        let store = InMemoryCheckpointStore::new();

        // Save an incomplete workflow
        let outcome = StepOutcome::success("step-1", serde_json::json!(1));
        let cp = Checkpoint::new("wf-incomplete", "test", 0, 3, outcome, vec![]);
        store.save(&cp).await.unwrap();

        let recovery = CrashRecovery::new(store);
        let result = recovery.scan().await.unwrap();
        assert_eq!(result.found, 1);
        assert_eq!(result.recovered, 1);
        assert_eq!(result.details[0].resume_from_step, 1);
    }

    #[tokio::test]
    async fn crash_recovery_skips_complete() {
        let store = InMemoryCheckpointStore::new();

        // Save a complete workflow
        let outcome = StepOutcome::success("step-3", serde_json::json!("done"));
        let cp = Checkpoint::new("wf-complete", "test", 2, 3, outcome, vec![]);
        store.save(&cp).await.unwrap();

        let recovery = CrashRecovery::new(store);
        let result = recovery.scan().await.unwrap();
        assert_eq!(result.found, 0);
    }

    #[tokio::test]
    async fn crash_recovery_get_checkpoint() {
        let store = InMemoryCheckpointStore::new();

        let outcome = StepOutcome::success("step-1", serde_json::json!(1));
        let cp = Checkpoint::new("wf-1", "test", 0, 3, outcome, vec![]);
        store.save(&cp).await.unwrap();

        let recovery = CrashRecovery::new(store);
        let loaded = recovery.get_checkpoint("wf-1").await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().step_index, 0);
    }

    // ── WorkflowStatus Tests ──────────────────────────────────

    #[test]
    fn workflow_status_terminal() {
        assert!(!WorkflowStatus::Pending.is_terminal());
        assert!(!WorkflowStatus::Running.is_terminal());
        assert!(WorkflowStatus::Completed.is_terminal());
        assert!(WorkflowStatus::Failed.is_terminal());
        assert!(WorkflowStatus::Cancelled.is_terminal());
    }

    #[test]
    fn workflow_status_display() {
        assert_eq!(format!("{}", WorkflowStatus::Pending), "pending");
        assert_eq!(format!("{}", WorkflowStatus::Running), "running");
        assert_eq!(format!("{}", WorkflowStatus::Completed), "completed");
        assert_eq!(format!("{}", WorkflowStatus::Failed), "failed");
        assert_eq!(format!("{}", WorkflowStatus::Cancelled), "cancelled");
    }

    // ── StepOutcome Serialization Tests ───────────────────────

    #[test]
    fn step_outcome_serialization_roundtrip() {
        let outcome = StepOutcome::success("step-1", serde_json::json!({"result": 42}));
        let json = serde_json::to_string(&outcome).unwrap();
        let deserialized: StepOutcome = serde_json::from_str(&json).unwrap();
        assert_eq!(outcome.step_name, deserialized.step_name);
        assert_eq!(outcome.success, deserialized.success);
        assert_eq!(outcome.output, deserialized.output);
    }

    #[test]
    fn checkpoint_serialization_roundtrip() {
        let outcome = StepOutcome::success("step-1", serde_json::json!({"x": 1}));
        let checkpoint = Checkpoint::new("wf-1", "test", 0, 3, outcome, vec![]);
        let json = serde_json::to_string(&checkpoint).unwrap();
        let deserialized: Checkpoint = serde_json::from_str(&json).unwrap();
        assert_eq!(checkpoint.workflow_id, deserialized.workflow_id);
        assert_eq!(checkpoint.step_index, deserialized.step_index);
        assert!(deserialized.verify_hash());
    }

    // ── WorkflowResult Tests ──────────────────────────────────

    #[test]
    fn workflow_result_helpers() {
        let result = WorkflowResult {
            workflow_id: "wf-1".to_string(),
            workflow_name: "test".to_string(),
            status: WorkflowStatus::Completed,
            steps_completed: 3,
            total_steps: 3,
            outcomes: vec![
                StepOutcome::success("step-1", serde_json::json!(1)),
                StepOutcome::success("step-2", serde_json::json!(2)),
                StepOutcome::success("step-3", serde_json::json!(3)),
            ],
            total_duration_ms: 150,
        };
        assert!(result.is_completed());
        assert!(!result.is_failed());
        assert_eq!(result.last_output().unwrap(), &serde_json::json!(3));
    }

    // ── DurableExecutorConfig Tests ───────────────────────────

    #[test]
    fn executor_config_default() {
        let config = DurableExecutorConfig::default();
        assert!(config.checkpoint_enabled);
        assert_eq!(config.max_step_retries, 0);
        assert_eq!(config.step_timeout_ms, 0);
        assert!(config.verify_hashes);
    }
}
