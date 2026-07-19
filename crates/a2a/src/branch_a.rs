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
use crate::task::A2aTask;

/// The standalone A2A server configuration.
#[derive(Debug, Clone)]
pub struct A2aServerConfig {
    /// The base URL this server is reachable at.
    pub base_url: String,
    /// The port to listen on.
    pub port: u16,
    /// The AgentCard to serve.
    pub card: AgentCard,
}

impl A2aServerConfig {
    /// Create a new A2A server config with the default AgentCard.
    pub fn new(base_url: &str, port: u16) -> Self {
        Self {
            base_url: base_url.to_string(),
            port,
            card: default_aegis_card(base_url),
        }
    }

    /// Create a server config with a custom AgentCard.
    pub fn with_card(base_url: &str, port: u16, card: AgentCard) -> Self {
        Self {
            base_url: base_url.to_string(),
            port,
            card,
        }
    }

    /// The full URL to the AgentCard.
    pub fn card_url(&self) -> String {
        format!("{}{}", self.base_url, AGENT_CARD_PATH)
    }
}

/// Start the standalone A2A server.
///
/// Serves the AgentCard at `/.well-known/agent-card.json` and
/// handles incoming A2A tasks. Full HTTP server implementation
/// deferred to Phase 5.1 (durable execution).
pub async fn serve_a2a(_config: A2aServerConfig) -> anyhow::Result<()> {
    // Phase 5.1: Stand up an HTTP server with:
    // - GET /.well-known/agent-card.json → serve AgentCard
    // - POST /a2a/tasks → accept new tasks, return task ID
    // - GET /a2a/tasks/{id} → stream task state transitions
    // - POST /a2a/tasks/{id}/cancel → cancel a running task
    unimplemented!("A2A HTTP server deferred to Phase 5.1 — durable execution")
}

/// Send a task to a remote agent discovered via its AgentCard.
pub async fn send_task_to_remote(
    _card: &AgentCard,
    _task: &A2aTask,
) -> anyhow::Result<crate::task::TaskResult> {
    // Phase 5.1: Resolve card endpoint, verify trust, POST task
    unimplemented!("A2A remote task dispatch deferred to Phase 5.1")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_config_card_url() {
        let config = A2aServerConfig::new("http://localhost:8080", 8080);
        assert_eq!(config.card_url(), "http://localhost:8080/.well-known/agent-card.json");
    }

    #[test]
    fn server_config_default_card_has_skills() {
        let config = A2aServerConfig::new("http://localhost:9999", 9999);
        assert!(!config.card.skills.is_empty());
        assert_eq!(config.card.name, "ferris-aegis");
    }
}
