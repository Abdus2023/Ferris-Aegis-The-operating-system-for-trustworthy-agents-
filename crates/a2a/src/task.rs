//! A2A Task Protocol — Send → Stream → Complete.
//!
//! The core A2A task lifecycle for inter-agent communication.
//! Tasks carry a skill invocation from one agent to another and
//! track state through a well-defined lifecycle.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The state of an A2A task.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskState {
    /// Task has been submitted to the receiver.
    Submitted,
    /// Task is actively being worked on.
    Working,
    /// Task completed successfully.
    Completed,
    /// Task was cancelled by the sender.
    Cancelled,
    /// Task failed during execution.
    Failed,
}

impl TaskState {
    /// Whether the task is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TaskState::Completed | TaskState::Cancelled | TaskState::Failed
        )
    }

    /// Whether the task is actively being processed.
    pub fn is_active(&self) -> bool {
        matches!(self, TaskState::Submitted | TaskState::Working)
    }
}

impl std::fmt::Display for TaskState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskState::Submitted => write!(f, "submitted"),
            TaskState::Working => write!(f, "working"),
            TaskState::Completed => write!(f, "completed"),
            TaskState::Cancelled => write!(f, "cancelled"),
            TaskState::Failed => write!(f, "failed"),
        }
    }
}

/// An A2A task — a unit of work sent between agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2aTask {
    /// Unique task identifier.
    pub id: String,
    /// The agent that sent the task.
    pub sender_agent_id: String,
    /// The agent that should handle the task.
    pub receiver_agent_id: String,
    /// The skill to invoke on the receiver.
    pub skill_id: String,
    /// Task input payload.
    pub input: serde_json::Value,
    /// Current task state.
    pub state: TaskState,
    /// When the task was created.
    pub created_at: DateTime<Utc>,
    /// When the task reached a terminal state.
    pub completed_at: Option<DateTime<Utc>>,
    /// Optional session context.
    pub session_id: Option<String>,
    /// Required trust level for the receiver.
    pub required_trust: Option<crate::agent_card::TrustLevel>,
}

impl A2aTask {
    /// Create a new A2A task.
    pub fn new(
        sender_agent_id: &str,
        receiver_agent_id: &str,
        skill_id: &str,
        input: serde_json::Value,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            sender_agent_id: sender_agent_id.to_string(),
            receiver_agent_id: receiver_agent_id.to_string(),
            skill_id: skill_id.to_string(),
            input,
            state: TaskState::Submitted,
            created_at: Utc::now(),
            completed_at: None,
            session_id: None,
            required_trust: None,
        }
    }

    /// Transition to working state.
    pub fn start(&mut self) {
        self.state = TaskState::Working;
    }

    /// Complete the task with output.
    pub fn complete(&mut self, output: serde_json::Value) -> TaskResult {
        self.state = TaskState::Completed;
        self.completed_at = Some(Utc::now());
        TaskResult {
            task_id: self.id.clone(),
            state: TaskState::Completed,
            output,
        }
    }

    /// Cancel the task.
    pub fn cancel(&mut self) {
        self.state = TaskState::Cancelled;
        self.completed_at = Some(Utc::now());
    }

    /// Mark the task as failed with an error message.
    pub fn fail(&mut self, error: &str) -> TaskResult {
        self.state = TaskState::Failed;
        self.completed_at = Some(Utc::now());
        TaskResult {
            task_id: self.id.clone(),
            state: TaskState::Failed,
            output: serde_json::json!({"error": error}),
        }
    }

    /// Set the session context.
    pub fn with_session(mut self, session_id: &str) -> Self {
        self.session_id = Some(session_id.to_string());
        self
    }

    /// Set a required trust level.
    pub fn with_required_trust(mut self, level: crate::agent_card::TrustLevel) -> Self {
        self.required_trust = Some(level);
        self
    }
}

/// The result of an A2A task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    /// The task ID this result belongs to.
    pub task_id: String,
    /// Final task state.
    pub state: TaskState,
    /// Task output payload.
    pub output: serde_json::Value,
}

/// Send a task to an agent and wait for the result.
///
/// This is the primary A2A interaction pattern: send a task,
/// stream progress updates, and receive the final result.
pub async fn send_task(
    _task: &A2aTask,
    _router: &crate::A2aRouter,
) -> Result<TaskResult, crate::RouteError> {
    // Full implementation requires:
    // 1. Resolve receiver's AgentCard via router
    // 2. Verify trust level
    // 3. Open connection to receiver's endpoint
    // 4. Send task, stream state transitions, await result
    // Deferred to Phase 5.1 (durable execution) per the plan.
    unimplemented!("full A2A task transport deferred to Phase 5.1")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_lifecycle_submitted_to_completed() {
        let mut task = A2aTask::new(
            "sender",
            "receiver",
            "file_read",
            serde_json::json!({"path": "/workspace/test.txt"}),
        );

        assert_eq!(task.state, TaskState::Submitted);
        assert!(!task.state.is_terminal());

        task.start();
        assert_eq!(task.state, TaskState::Working);

        let result = task.complete(serde_json::json!({"content": "hello"}));
        assert_eq!(result.state, TaskState::Completed);
        assert!(!result.output.is_null());
        assert!(task.completed_at.is_some());
        assert!(task.state.is_terminal());
    }

    #[test]
    fn task_cancel() {
        let mut task = A2aTask::new("s", "r", "skill", serde_json::Value::Null);
        task.start();
        task.cancel();
        assert_eq!(task.state, TaskState::Cancelled);
        assert!(task.state.is_terminal());
    }

    #[test]
    fn task_fail_with_error() {
        let mut task = A2aTask::new("s", "r", "skill", serde_json::Value::Null);
        task.start();
        let result = task.fail("skill not found");
        assert_eq!(result.state, TaskState::Failed);
        assert!(result.output.to_string().contains("error"));
    }

    #[test]
    fn task_with_session_and_trust() {
        let task = A2aTask::new("s", "r", "read", serde_json::json!({}))
            .with_session("session-1")
            .with_required_trust(crate::agent_card::TrustLevel::Standard);

        assert_eq!(task.session_id.as_deref(), Some("session-1"));
        assert_eq!(
            task.required_trust,
            Some(crate::agent_card::TrustLevel::Standard)
        );
    }
}
