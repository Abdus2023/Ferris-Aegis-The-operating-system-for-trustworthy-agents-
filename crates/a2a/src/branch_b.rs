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

use serde::{Deserialize, Serialize};

/// MCP tool definitions for Branch B — Phase 4 capabilities exposed
/// through the existing MCP interface.

/// Parameters for the `session_create` MCP tool.
#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct SessionCreateParams {
    /// Agent ID to create a session for.
    pub agent_id: String,
    /// Maximum tokens.
    pub max_tokens: Option<u64>,
    /// Maximum cost in USD.
    pub max_cost_usd: Option<f64>,
    /// Maximum rounds.
    pub max_rounds: Option<u32>,
    /// Maximum wall-clock seconds.
    pub max_wall_clock_secs: Option<u64>,
}

/// Parameters for the `session_status` MCP tool.
#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct SessionStatusParams {
    /// Session ID to check.
    pub session_id: String,
}

/// Parameters for the `supervisor_run` MCP tool.
#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct SupervisorRunParams {
    /// Agent ID for the supervision session.
    pub agent_id: String,
    /// Maximum parallel sub-agents.
    pub max_parallel: Option<u32>,
    /// Maximum total tokens across all sub-agents.
    pub max_total_tokens: Option<u64>,
}

/// Parameters for the `memory_search` MCP tool.
#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct MemorySearchParams {
    /// Agent ID to search knowledge for.
    pub agent_id: String,
    /// Search query text.
    pub query: String,
    /// Maximum results.
    pub limit: Option<usize>,
}

/// Response from a Branch B MCP tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchBResponse {
    /// Whether the operation succeeded.
    pub success: bool,
    /// Result data.
    pub data: serde_json::Value,
    /// Error message, if any.
    pub error: Option<String>,
}

impl BranchBResponse {
    /// Create a success response.
    pub fn ok(data: serde_json::Value) -> Self {
        Self {
            success: true,
            data,
            error: None,
        }
    }

    /// Create an error response.
    pub fn err(msg: &str) -> Self {
        Self {
            success: false,
            data: serde_json::Value::Null,
            error: Some(msg.to_string()),
        }
    }
}

/// List of MCP tool names exposed by Branch B.
pub const BRANCH_B_TOOLS: &[&str] = &[
    "session_create",
    "session_status",
    "supervisor_run",
    "memory_search",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_b_tools_are_listed() {
        assert!(BRANCH_B_TOOLS.contains(&"session_create"));
        assert!(BRANCH_B_TOOLS.contains(&"session_status"));
        assert!(BRANCH_B_TOOLS.contains(&"supervisor_run"));
        assert!(BRANCH_B_TOOLS.contains(&"memory_search"));
    }

    #[test]
    fn branch_b_response_success() {
        let resp = BranchBResponse::ok(serde_json::json!({"session_id": "abc"}));
        assert!(resp.success);
        assert!(resp.error.is_none());
    }

    #[test]
    fn branch_b_response_error() {
        let resp = BranchBResponse::err("session not found");
        assert!(!resp.success);
        assert_eq!(resp.error.unwrap(), "session not found");
    }

    #[test]
    fn session_create_params_schema() {
        // Verify the params have JsonSchema derived
        let schema = schemars::schema_for!(SessionCreateParams);
        let json = serde_json::to_string(&schema).unwrap();
        assert!(json.contains("agent_id"));
        assert!(json.contains("max_tokens"));
    }
}
