//! AgentCard — The A2A discovery document.
//!
//! The AgentCard is a JSON document served at `/.well-known/agent-card.json`
//! that describes an agent's capabilities, skills, and connection details
//! to other agents in an A2A mesh.

use serde::{Deserialize, Serialize};

/// An AgentCard per the A2A specification.
///
/// Served at `/.well-known/agent-card.json` for agent discovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCard {
    /// The name of the agent.
    pub name: String,
    /// A human-readable description.
    pub description: String,
    /// The URL where the agent can be reached.
    pub url: String,
    /// The agent's version.
    pub version: String,
    /// The skills this agent offers.
    pub skills: Vec<AgentSkill>,
    /// The agent's capabilities.
    pub capabilities: AgentCapabilities,
    /// The agent's provider/organization.
    pub provider: Option<AgentProvider>,
    /// Authentication requirements.
    pub authentication: Option<AgentAuthentication>,
}

/// A skill offered by an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSkill {
    /// Unique skill identifier.
    pub id: String,
    /// Human-readable skill name.
    pub name: String,
    /// What this skill does.
    pub description: String,
    /// Input schema for the skill (JSON Schema).
    pub input_schema: Option<serde_json::Value>,
    /// Output schema for the skill (JSON Schema).
    pub output_schema: Option<serde_json::Value>,
    /// Tags for skill discovery.
    pub tags: Vec<String>,
}

/// An agent's capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCapabilities {
    /// Whether the agent supports streaming responses.
    pub streaming: bool,
    /// Whether the agent can push notifications.
    pub push_notifications: bool,
    /// Maximum time (ms) the agent will work on a task.
    pub state_transition_history: bool,
}

impl Default for AgentCapabilities {
    fn default() -> Self {
        Self {
            streaming: true,
            push_notifications: false,
            state_transition_history: true,
        }
    }
}

/// Information about the agent's provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProvider {
    /// Provider organization name.
    pub organization: String,
    /// Provider URL.
    pub url: Option<String>,
}

/// Authentication requirements for connecting to the agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentAuthentication {
    /// Authentication scheme (e.g., "bearer", "oauth2").
    pub scheme: String,
    /// Required scopes, if any.
    pub scopes: Vec<String>,
}

/// Builder for constructing an AgentCard.
pub struct AgentCardBuilder {
    card: AgentCard,
}

impl AgentCardBuilder {
    /// Start building an AgentCard with required fields.
    pub fn new(name: &str, url: &str, version: &str) -> Self {
        Self {
            card: AgentCard {
                name: name.to_string(),
                description: String::new(),
                url: url.to_string(),
                version: version.to_string(),
                skills: Vec::new(),
                capabilities: AgentCapabilities::default(),
                provider: None,
                authentication: None,
            },
        }
    }

    /// Set the description.
    pub fn description(mut self, desc: &str) -> Self {
        self.card.description = desc.to_string();
        self
    }

    /// Add a skill.
    pub fn skill(mut self, skill: AgentSkill) -> Self {
        self.card.skills.push(skill);
        self
    }

    /// Set capabilities.
    pub fn capabilities(mut self, caps: AgentCapabilities) -> Self {
        self.card.capabilities = caps;
        self
    }

    /// Set the provider.
    pub fn provider(mut self, org: &str) -> Self {
        self.card.provider = Some(AgentProvider {
            organization: org.to_string(),
            url: None,
        });
        self
    }

    /// Set authentication.
    pub fn authentication(mut self, scheme: &str, scopes: Vec<String>) -> Self {
        self.card.authentication = Some(AgentAuthentication {
            scheme: scheme.to_string(),
            scopes,
        });
        self
    }

    /// Build the AgentCard.
    pub fn build(self) -> AgentCard {
        self.card
    }
}

/// Build the default Ferris Aegis AgentCard.
pub fn default_aegis_card(base_url: &str) -> AgentCard {
    AgentCardBuilder::new("ferris-aegis", base_url, env!("CARGO_PKG_VERSION"))
        .description("Ferris Aegis — The Rust Guardian for Autonomous Intelligence. A trustworthy agent operating system with policy enforcement, audit logging, and credential protection.")
        .skill(AgentSkill {
            id: "file_read".to_string(),
            name: "File Read".to_string(),
            description: "Read a file from the local filesystem. Only absolute paths accepted.".to_string(),
            input_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Absolute path to the file"},
                    "max_bytes": {"type": "integer", "description": "Maximum bytes to read (default: 65536)"}
                },
                "required": ["path"]
            })),
            output_schema: None,
            tags: vec!["file".to_string(), "read".to_string(), "filesystem".to_string()],
        })
        .skill(AgentSkill {
            id: "session_create".to_string(),
            name: "Create Session".to_string(),
            description: "Create a new agent session with budget tracking.".to_string(),
            input_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "agent_id": {"type": "string"},
                    "max_tokens": {"type": "integer"},
                    "max_cost_usd": {"type": "number"},
                    "max_rounds": {"type": "integer"},
                    "max_wall_clock_secs": {"type": "integer"}
                },
                "required": ["agent_id"]
            })),
            output_schema: None,
            tags: vec!["session".to_string(), "budget".to_string()],
        })
        .capabilities(AgentCapabilities {
            streaming: true,
            push_notifications: false,
            state_transition_history: true,
        })
        .provider("Ferris Aegis")
        .build()
}

/// The well-known path for the AgentCard.
/// Per the A2A spec and RFC 8615.
/// WARNING: Do NOT use `/.well-known/agent.json` (pre-1.0 path).
pub const AGENT_CARD_PATH: &str = "/.well-known/agent-card.json";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_card_serialization() {
        let card = default_aegis_card("http://localhost:8080");
        let json = serde_json::to_string_pretty(&card).unwrap();
        assert!(json.contains("ferris-aegis"));
        assert!(json.contains("file_read"));
        assert!(json.contains("session_create"));
    }

    #[test]
    fn agent_card_builder() {
        let card = AgentCardBuilder::new("test-agent", "http://localhost:3000", "1.0.0")
            .description("A test agent")
            .skill(AgentSkill {
                id: "test_skill".to_string(),
                name: "Test Skill".to_string(),
                description: "A test".to_string(),
                input_schema: None,
                output_schema: None,
                tags: vec!["test".to_string()],
            })
            .provider("Test Org")
            .build();

        assert_eq!(card.name, "test-agent");
        assert_eq!(card.skills.len(), 1);
        assert!(card.provider.is_some());
    }

    #[test]
    fn agent_card_path_is_correct() {
        // Must be the hyphenated, plural path per A2A spec
        assert_eq!(AGENT_CARD_PATH, "/.well-known/agent-card.json");
        // NOT the old pre-1.0 path
        assert_ne!(AGENT_CARD_PATH, "/.well-known/agent.json");
    }

    #[test]
    fn default_card_has_required_skills() {
        let card = default_aegis_card("http://localhost:8080");
        let skill_ids: Vec<&str> = card.skills.iter().map(|s| s.id.as_str()).collect();
        assert!(skill_ids.contains(&"file_read"));
        assert!(skill_ids.contains(&"session_create"));
    }
}
