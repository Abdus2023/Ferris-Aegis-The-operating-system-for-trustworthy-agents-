//! Branch B — Integrated (MCP Tool Exposure).
//!
//! Skip the AgentCard server. Expose Phase 4 capabilities (session
//! management, supervision, semantic memory) as MCP tools that the
//! TypeScript orchestrator calls through the existing MCP interface
//! from Phase 2.
//!
//! No new protocol surface, no AgentCard, no separate client library.
//! This is the correct choice when a TypeScript layer already owns
//! the A2A mesh.

use schemars::JsonSchema;
use serde::Deserialize;

/// Parameters for the `session_create` MCP tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct SessionCreateParams {
    /// Agent ID to create a session for.
    pub agent_id: String,
    /// Maximum tokens for this session.
    pub max_tokens: Option<u64>,
    /// Maximum cost in USD for this session.
    pub max_cost_usd: Option<f64>,
    /// Maximum conversation rounds for this session.
    pub max_rounds: Option<u32>,
    /// Maximum wall-clock seconds for this session.
    pub max_wall_clock_secs: Option<u64>,
}

/// Parameters for the `session_status` MCP tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct SessionStatusParams {
    /// Session ID to check.
    pub session_id: String,
}

/// Parameters for the `supervisor_inspect` MCP tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct SupervisorInspectParams {
    /// Agent ID to run supervision on.
    pub agent_id: String,
    /// Session ID to inspect.
    pub session_id: String,
}

/// Parameters for the `memory_search` MCP tool (semantic memory).
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct MemorySearchParams {
    /// Agent ID to search knowledge for.
    pub agent_id: String,
    /// Search query text.
    pub query: String,
    /// Maximum results to return.
    pub limit: Option<usize>,
}

/// Parameters for the `concept_store` MCP tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ConceptStoreParams {
    /// Agent ID to associate the concept with.
    pub agent_id: String,
    /// Concept name.
    pub name: String,
    /// Concept description.
    pub description: String,
    /// Labels for classification.
    pub labels: Option<Vec<String>>,
}

/// Parameters for the `agent_card_query` MCP tool (limited AgentCard exposure).
///
/// In Branch B, we don't serve a full `/.well-known/agent-card.json`.
/// Instead, the orchestrator can query capabilities through this MCP tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct AgentCardQueryParams {
    /// Which agent to query capabilities for.
    pub agent_id: String,
}

/// Budget tracking result returned by session_status.
#[derive(Debug, Clone, serde::Serialize, JsonSchema)]
pub struct BudgetStatus {
    /// Total tokens used so far.
    pub tokens_used: u64,
    /// Maximum tokens allowed (if set).
    pub max_tokens: Option<u64>,
    /// Total cost in USD so far.
    pub cost_usd: f64,
    /// Maximum cost allowed (if set).
    pub max_cost_usd: Option<f64>,
    /// Rounds completed so far.
    pub rounds_completed: u32,
    /// Maximum rounds allowed (if set).
    pub max_rounds: Option<u32>,
    /// Whether any budget limit has been exceeded.
    pub budget_exceeded: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_create_params_serialize() {
        let params = SessionCreateParams {
            agent_id: "agent-1".to_string(),
            max_tokens: Some(1000),
            max_cost_usd: Some(5.0),
            max_rounds: Some(10),
            max_wall_clock_secs: Some(300),
        };
        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("agent-1"));
        assert!(json.contains("max_tokens"));
    }

    #[test]
    fn budget_status_defaults() {
        let status = BudgetStatus {
            tokens_used: 500,
            max_tokens: Some(1000),
            cost_usd: 2.5,
            max_cost_usd: Some(5.0),
            rounds_completed: 5,
            max_rounds: Some(10),
            budget_exceeded: false,
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("500"));
        assert!(json.contains("budget_exceeded"));
    }
}
