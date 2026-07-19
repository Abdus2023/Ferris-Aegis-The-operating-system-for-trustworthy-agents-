//! Branch A — Standalone A2A Server.
//!
//! Full A2A protocol stack: AgentCard server at
//! `/.well-known/agent-card.json`, task handler, and A2A client.
//! Use when Ferris Aegis needs to be discoverable by agents
//! you don't control.
//!
//! This is the **full-fat** implementation. If the TypeScript layer
//! already owns the A2A mesh, use `branch_b` instead.

use crate::agent_card::{AgentCard, AGENT_CARD_PATH, default_aegis_card};
use crate::task::{A2aTask, TaskResult, TaskState};

/// The standalone A2A server configuration.
#[derive(Debug, Clone)]
pub struct A2aServerConfig {
    /// The base URL this server is reachable at.
    pub base_url: String,
    /// The port to listen on.
    pub port: u16,
}

impl Default for A2aServerConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:8080".to_string(),
            port: 8080,
        }
    }
}

/// The standalone A2A server.
pub struct A2aServer {
    config: A2aServerConfig,
    card: AgentCard,
    tasks: Vec<A2aTask>,
}

impl A2aServer {
    /// Create a new A2A server with the given configuration.
    pub fn new(config: A2aServerConfig) -> Self {
        let card = default_aegis_card(&config.base_url);
        Self {
            config,
            card,
            tasks: Vec::new(),
        }
    }

    /// Create a server with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(A2aServerConfig::default())
    }

    /// Get the AgentCard.
    pub fn agent_card(&self) -> &AgentCard {
        &self.card
    }

    /// Get the AgentCard path.
    pub fn agent_card_path(&self) -> &'static str {
        AGENT_CARD_PATH
    }

    /// Submit a task to this server.
    pub fn submit_task(&mut self, mut task: A2aTask) -> &A2aTask {
        task.start();
        self.tasks.push(task);
        self.tasks.last().unwrap()
    }

    /// Complete a task.
    pub fn complete_task(&mut self, task_id: &str) -> Option<TaskResult> {
        let task = self.tasks.iter_mut().find(|t| t.id == task_id)?;
        Some(task.complete())
    }

    /// List all tasks.
    pub fn list_tasks(&self) -> &[A2aTask] {
        &self.tasks
    }

    /// Get pending tasks.
    pub fn pending_tasks(&self) -> Vec<&A2aTask> {
        self.tasks.iter().filter(|t| !t.state.is_terminal()).collect()
    }

    /// Get the server configuration.
    pub fn config(&self) -> &A2aServerConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_serves_agent_card() {
        let server = A2aServer::with_defaults();
        let card = server.agent_card();
        assert_eq!(card.name, "ferris-aegis");
        assert_eq!(server.agent_card_path(), "/.well-known/agent-card.json");
    }

    #[test]
    fn server_task_lifecycle() {
        let mut server = A2aServer::with_defaults();

        let task = A2aTask::new(
            "remote-agent",
            "ferris-aegis",
            "file_read",
            serde_json::json!({"path": "/workspace/data.txt"}),
        );
        let task_id = task.id.clone();

        server.submit_task(task);
        assert_eq!(server.pending_tasks().len(), 1);

        let result = server.complete_task(&task_id).unwrap();
        assert_eq!(result.state, TaskState::Completed);
        assert!(server.pending_tasks().is_empty());
    }

    #[test]
    fn server_default_config() {
        let config = A2aServerConfig::default();
        assert_eq!(config.port, 8080);
    }
}
