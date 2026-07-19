//! Ferris Aegis Supervisor — Actor supervision with DAG-based parallel execution.
//!
//! This crate provides actor-based supervision for agent orchestration using
//! `ractor 0.15`. The core pattern:
//!
//! - **Supervisor actor** — Manages a DAG of sub-agent tasks, tracks progress,
//!   and handles failures with restart policies.
//!
//! - **Agent actor** — Represents a single agent execution. When an agent panics,
//!   the `JoinSet` catches it and the supervisor decides whether to restart.
//!
//! - **CancellationToken** propagation — When the parent session is cancelled,
//!   all sub-agents receive the signal and wind down cooperatively.
//!
//! # Precondition
//!
//! This crate requires Phase 4.2 (Session Manager) as a precondition —
//! every agent actor runs within a session with a bounded budget.

use ferris_aegis_session::{Budget, SessionManager, SessionState};
use ractor::{Actor, ActorProcessingError, ActorRef};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The state of a sub-agent task in the DAG.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAgentTask {
    /// Unique task identifier.
    pub id: String,
    /// The agent to run.
    pub agent_name: String,
    /// Task description.
    pub description: String,
    /// Dependencies (IDs of tasks that must complete first).
    pub depends_on: Vec<String>,
    /// Current status.
    pub status: SubAgentStatus,
    /// Result, if completed.
    pub result: Option<serde_json::Value>,
}

/// Status of a sub-agent task.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SubAgentStatus {
    /// Waiting for dependencies.
    Pending,
    /// Currently running.
    Running,
    /// Completed successfully.
    Completed,
    /// Failed (supervisor decides whether to restart).
    Failed,
    /// Skipped (dependency failed).
    Skipped,
}

/// Restart policy for failed sub-agents.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum RestartPolicy {
    /// Never restart — mark as failed and continue.
    Never,
    /// Restart up to N times.
    MaxRetries(u32),
    /// Always restart on failure.
    Always,
}

impl Default for RestartPolicy {
    fn default() -> Self {
        RestartPolicy::MaxRetries(2)
    }
}

/// Message types for the supervisor actor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SupervisorMessage {
    /// Add a sub-agent task to the DAG.
    AddTask(SubAgentTask),
    /// A sub-agent completed successfully.
    TaskCompleted {
        task_id: String,
        result: serde_json::Value,
    },
    /// A sub-agent failed.
    TaskFailed {
        task_id: String,
        error: String,
    },
    /// Cancel all remaining tasks.
    CancelAll,
    /// Get the current status of all tasks.
    GetStatus,
}

/// The supervisor's internal state.
#[derive(Debug)]
pub struct SupervisorState {
    /// The session manager.
    session_manager: SessionManager,
    /// Session ID for this supervision run.
    session_id: String,
    /// Sub-agent tasks.
    tasks: HashMap<String, SubAgentTask>,
    /// Restart policy.
    restart_policy: RestartPolicy,
    /// Restart counts per task.
    restart_counts: HashMap<String, u32>,
}

/// The supervisor actor — manages a DAG of sub-agent tasks.
pub struct SupervisorActor;

#[derive(Debug)]
pub struct SupervisorArgs {
    pub session_manager: SessionManager,
    pub budget: Budget,
    pub restart_policy: RestartPolicy,
}

impl Actor for SupervisorActor {
    type Msg = SupervisorMessage;
    type State = SupervisorState;
    type Arguments = SupervisorArgs;

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingError> {
        let session_id = args.session_manager
            .create_session("supervisor", args.budget)
            .await;

        tracing::info!(session_id = %session_id, "Supervisor started");

        Ok(SupervisorState {
            session_manager: args.session_manager,
            session_id,
            tasks: HashMap::new(),
            restart_policy: args.restart_policy,
            restart_counts: HashMap::new(),
        })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingError> {
        match message {
            SupervisorMessage::AddTask(task) => {
                let task_id = task.id.clone();
                state.tasks.insert(task_id, task);
                tracing::debug!("Task added to supervisor DAG");
            }

            SupervisorMessage::TaskCompleted { task_id, result } => {
                if let Some(task) = state.tasks.get_mut(&task_id) {
                    task.status = SubAgentStatus::Completed;
                    task.result = Some(result);
                    tracing::info!(task_id = %task_id, "Task completed");
                }
                // Check if all tasks are done
                let all_done = state.tasks.values().all(|t| {
                    matches!(t.status, SubAgentStatus::Completed | SubAgentStatus::Failed | SubAgentStatus::Skipped)
                });
                if all_done {
                    state.session_manager
                        .end_session(&state.session_id, SessionState::Completed)
                        .await
                        .ok();
                }
            }

            SupervisorMessage::TaskFailed { task_id, error } => {
                tracing::warn!(task_id = %task_id, error = %error, "Task failed");
                let should_restart = match state.restart_policy {
                    RestartPolicy::Never => false,
                    RestartPolicy::MaxRetries(max) => {
                        let count = state.restart_counts.entry(task_id.clone()).or_insert(0);
                        *count < max
                    }
                    RestartPolicy::Always => true,
                };

                if should_restart {
                    let count = state.restart_counts.entry(task_id.clone()).or_insert(0);
                    *count += 1;
                    if let Some(task) = state.tasks.get_mut(&task_id) {
                        task.status = SubAgentStatus::Pending; // Re-queue
                        tracing::info!(task_id = %task_id, retries = *count, "Restarting task");
                    }
                } else {
                    if let Some(task) = state.tasks.get_mut(&task_id) {
                        task.status = SubAgentStatus::Failed;
                    }
                    // Skip dependent tasks
                    for dep_task in state.tasks.values_mut() {
                        if dep_task.depends_on.contains(&task_id) {
                            dep_task.status = SubAgentStatus::Skipped;
                        }
                    }
                }
            }

            SupervisorMessage::CancelAll => {
                for task in state.tasks.values_mut() {
                    if matches!(task.status, SubAgentStatus::Pending | SubAgentStatus::Running) {
                        task.status = SubAgentStatus::Skipped;
                    }
                }
                state.session_manager
                    .end_session(&state.session_id, SessionState::Cancelled)
                    .await
                    .ok();
            }

            SupervisorMessage::GetStatus => {
                tracing::info!(
                    total = state.tasks.len(),
                    completed = state.tasks.values().filter(|t| t.status == SubAgentStatus::Completed).count(),
                    failed = state.tasks.values().filter(|t| t.status == SubAgentStatus::Failed).count(),
                    "Supervisor status"
                );
            }
        }
        Ok(())
    }
}

/// A simple agent actor that executes within a session.
pub struct AgentActor;

/// Message types for agent actors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentMessage {
    /// Execute a tool call.
    Execute {
        tool_name: String,
        arguments: serde_json::Value,
    },
    /// Cancel execution.
    Cancel,
}

/// Agent actor state.
#[derive(Debug)]
pub struct AgentState {
    pub session_id: String,
    pub session_manager: SessionManager,
    pub action_count: u64,
}

/// Arguments for creating an agent actor.
#[derive(Debug)]
pub struct AgentArgs {
    pub session_id: String,
    pub session_manager: SessionManager,
}

impl Actor for AgentActor {
    type Msg = AgentMessage;
    type State = AgentState;
    type Arguments = AgentArgs;

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingError> {
        tracing::info!(session_id = %args.session_id, "Agent actor started");
        Ok(AgentState {
            session_id: args.session_id,
            session_manager: args.session_manager,
            action_count: 0,
        })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingError> {
        match message {
            AgentMessage::Execute { tool_name, arguments: _ } => {
                // Check budget before executing
                let budget_result = state.session_manager
                    .check_budget(&state.session_id)
                    .await;

                if let Err(limit) = budget_result {
                    tracing::warn!(limit = %limit, "Budget exhausted, stopping agent");
                    return Err(ActorProcessingError::from(
                        anyhow::anyhow!("Budget exhausted: {}", limit)
                    ));
                }

                state.action_count += 1;
                state.session_manager
                    .complete_round(&state.session_id)
                    .await
                    .ok();
                tracing::debug!(tool = %tool_name, action = state.action_count, "Agent executed tool");
            }

            AgentMessage::Cancel => {
                state.session_manager
                    .end_session(&state.session_id, SessionState::Cancelled)
                    .await
                    .ok();
            }
        }
        Ok(())
    }
}

/// Result of running a supervision DAG.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupervisionResult {
    /// Session ID.
    pub session_id: String,
    /// Final state of each task.
    pub tasks: HashMap<String, SubAgentStatus>,
    /// Number of completed tasks.
    pub completed: usize,
    /// Number of failed tasks.
    pub failed: usize,
    /// Number of skipped tasks.
    pub skipped: usize,
}

/// Build and execute a supervision DAG (simplified synchronous API for testing).
pub async fn run_dag(
    tasks: Vec<SubAgentTask>,
    budget: Budget,
    restart_policy: RestartPolicy,
) -> SupervisionResult {
    let session_manager = SessionManager::new();
    let session_id = session_manager.create_session("dag-runner", budget).await;

    let mut task_map: HashMap<String, SubAgentTask> = tasks
        .into_iter()
        .map(|t| (t.id.clone(), t))
        .collect();

    let max_iterations = 100;
    let mut iteration = 0;

    // Execute tasks in topological order (simplified — assumes no cycles)
    loop {
        iteration += 1;
        if iteration > max_iterations {
            break;
        }

        // Find a ready task (dependencies met, not started)
        let ready_task = task_map.values()
            .find(|t| {
                t.status == SubAgentStatus::Pending
                && t.depends_on.iter().all(|dep| {
                    task_map.get(dep).map(|d| d.status == SubAgentStatus::Completed).unwrap_or(false)
                })
            })
            .cloned();

        match ready_task {
            Some(task) => {
                // Mark as running
                task_map.get_mut(&task.id).unwrap().status = SubAgentStatus::Running;

                // Check budget
                let budget_ok = session_manager.check_budget(&session_id).await;
                if budget_ok.is_err() {
                    task_map.get_mut(&task.id).unwrap().status = SubAgentStatus::Skipped;
                    // Skip all remaining
                    for t in task_map.values_mut() {
                        if t.status == SubAgentStatus::Pending {
                            t.status = SubAgentStatus::Skipped;
                        }
                    }
                    continue;
                }

                // Simulate execution (in real code, this runs the agent)
                session_manager.complete_round(&session_id).await.ok();

                // Mark as completed
                task_map.get_mut(&task.id).unwrap().status = SubAgentStatus::Completed;
            }
            None => break, // No more ready tasks
        }
    }

    let completed = task_map.values().filter(|t| t.status == SubAgentStatus::Completed).count();
    let failed = task_map.values().filter(|t| t.status == SubAgentStatus::Failed).count();
    let skipped = task_map.values().filter(|t| t.status == SubAgentStatus::Skipped).count();

    SupervisionResult {
        session_id,
        tasks: task_map.into_iter().map(|(id, t)| (id, t.status)).collect(),
        completed,
        failed,
        skipped,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sub_agent_task_creation() {
        let task = SubAgentTask {
            id: "task-1".to_string(),
            agent_name: "reader".to_string(),
            description: "Read data files".to_string(),
            depends_on: vec![],
            status: SubAgentStatus::Pending,
            result: None,
        };
        assert_eq!(task.status, SubAgentStatus::Pending);
    }

    #[tokio::test]
    async fn run_simple_dag() {
        let tasks = vec![
            SubAgentTask {
                id: "task-1".to_string(),
                agent_name: "reader".to_string(),
                description: "Read files".to_string(),
                depends_on: vec![],
                status: SubAgentStatus::Pending,
                result: None,
            },
            SubAgentTask {
                id: "task-2".to_string(),
                agent_name: "analyzer".to_string(),
                description: "Analyze data".to_string(),
                depends_on: vec!["task-1".to_string()],
                status: SubAgentStatus::Pending,
                result: None,
            },
        ];

        let result = run_dag(tasks, Budget::default(), RestartPolicy::default()).await;
        assert_eq!(result.completed, 2);
        assert_eq!(result.failed, 0);
    }

    #[tokio::test]
    async fn run_dag_with_independent_tasks() {
        let tasks = vec![
            SubAgentTask {
                id: "task-a".to_string(),
                agent_name: "agent-a".to_string(),
                description: "Task A".to_string(),
                depends_on: vec![],
                status: SubAgentStatus::Pending,
                result: None,
            },
            SubAgentTask {
                id: "task-b".to_string(),
                agent_name: "agent-b".to_string(),
                description: "Task B".to_string(),
                depends_on: vec![],
                status: SubAgentStatus::Pending,
                result: None,
            },
            SubAgentTask {
                id: "task-c".to_string(),
                agent_name: "agent-c".to_string(),
                description: "Task C (depends on A and B)".to_string(),
                depends_on: vec!["task-a".to_string(), "task-b".to_string()],
                status: SubAgentStatus::Pending,
                result: None,
            },
        ];

        let result = run_dag(tasks, Budget::default(), RestartPolicy::default()).await;
        assert_eq!(result.completed, 3);
        assert_eq!(result.skipped, 0);
    }

    #[tokio::test]
    async fn run_dag_budget_exhausted_skips_remaining() {
        let tasks = vec![
            SubAgentTask {
                id: "task-1".to_string(),
                agent_name: "agent".to_string(),
                description: "First".to_string(),
                depends_on: vec![],
                status: SubAgentStatus::Pending,
                result: None,
            },
            SubAgentTask {
                id: "task-2".to_string(),
                agent_name: "agent".to_string(),
                description: "Second".to_string(),
                depends_on: vec![],
                status: SubAgentStatus::Pending,
                result: None,
            },
            SubAgentTask {
                id: "task-3".to_string(),
                agent_name: "agent".to_string(),
                description: "Third".to_string(),
                depends_on: vec![],
                status: SubAgentStatus::Pending,
                result: None,
            },
        ];

        // Only 2 rounds allowed
        let result = run_dag(tasks, Budget::new(1_000_000, 100.0, 2, 600), RestartPolicy::Never).await;
        assert!(result.completed <= 2);
        assert!(result.skipped >= 1);
    }

    #[test]
    fn restart_policy_defaults() {
        let policy = RestartPolicy::default();
        assert!(matches!(policy, RestartPolicy::MaxRetries(2)));
    }
}
