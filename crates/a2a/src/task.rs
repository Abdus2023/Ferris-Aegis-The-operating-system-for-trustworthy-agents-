//! A2A Task Protocol — Send → Stream → Complete.
//!
//! The core A2A task lifecycle for inter-agent communication.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The state of an A2A task.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskState {
    /// Task has been submitted.
    Submitted,
    /// Task is being worked on.
    Working,
    /// Task completed successfully.
    Completed,
    /// Task was cancelled.
    Cancelled,
    /// Task failed.
    Failed,
}

impl TaskState {
    /// Whether the task is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(self, TaskState::Completed | TaskState::Cancelled | TaskState::Failed)
    }
}

/// An A2A task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2aTask {
    /// Unique task identifier.
    pub id: String,
    /// The agent that sent the task.
    pub sender_agent_id: String,
    /// The agent that should handle the task.
    pub receiver_agent_id: String,
    /// The skill to invoke.
    pub skill_id: String,
    /// Task input.
    pub input: serde_json::Value,
    /// Current state.
    pub state: TaskState,
    /// When the task was created.
    pub created_at: DateTime<Utc>,
    /// When the task was completed.
    pub completed_at: Option<DateTime<Utc>>,
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
        }
    }

    /// Transition to working state.
    pub fn start(&mut self) {
        self.state = TaskState::Working;
    }

    /// Complete the task with a result.
    pub fn complete(&mut self) -> TaskResult {
        self.state = TaskState::Completed;
        self.completed_at = Some(Utc::now());
        TaskResult {
            task_id: self.id.clone(),
            state: TaskState::Completed,
            output: serde_json::Value::Null,
        }
    }

    /// Cancel the task.
    pub fn cancel(&mut self) {
        self.state = TaskState::Cancelled;
        self.completed_at = Some(Utc::now());
    }

    /// Mark the task as failed.
    pub fn fail(&mut self) {
        self.state = TaskState::Failed;
        self.completed_at = Some(Utc::now());
    }
}

/// The result of an A2A task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    /// The task ID.
    pub task_id: String,
    /// Final task state.
    pub state: TaskState,
    /// Task output.
    pub output: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_lifecycle() {
        let mut task = A2aTask::new(
            "sender",
            "receiver",
            "file_read",
            serde_json::json!({"path": "/workspace/test.txt"}),
        );

        assert_eq!(task.state, TaskState::Submitted);

        task.start();
        assert_eq!(task.state, TaskState::Working);

        let result = task.complete();
        assert_eq!(result.state, TaskState::Completed);
        assert!(task.completed_at.is_some());
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
    fn task_fail() {
        let mut task = A2aTask::new("s", "r", "skill", serde_json::Value::Null);
        task.start();
        task.fail();
        assert_eq!(task.state, TaskState::Failed);
        assert!(task.state.is_terminal());
    }
}
