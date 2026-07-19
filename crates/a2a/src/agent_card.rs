//! AgentCard — The A2A discovery document.
//!
//! The AgentCard is a JSON document served at `/.well-known/agent-card.json`
//! that describes an agent's capabilities, skills, trust level, and connection
//! details to other agents in an A2A mesh. Per the A2A specification and
//! RFC 8615.
//!
//! # Branch A (Standalone)
//!
//! Build the full AgentCard discovery server. Use when Ferris Aegis needs
//! to be discoverable by agents you don't control.
//!
//! # Branch B (Integrated)
//!
//! Skip the AgentCard server. Expose Phase 4 capabilities as MCP tools
//! through the existing MCP interface. Use when a TypeScript layer already
//! owns the A2A mesh.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The well-known path for the AgentCard.
/// Per the A2A spec and RFC 8615.
/// WARNING: Do NOT use `/.well-known/agent.json` (pre-1.0 path).
pub const AGENT_CARD_PATH: &str = "/.well-known/agent-card.json";

/// An AgentCard per the A2A specification.
///
/// Served at `/.well-known/agent-card.json` for agent discovery.
/// JSON Schema generated via `schemars` for OpenAPI compatibility.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentCard {
    /// The name of the agent.
    pub name: String,
    /// A human-readable description.
    pub description: String,
    /// The URL where the agent can be reached.
    pub url: String,
    /// The agent's version.
    pub version: String,
    /// The A2A protocol version this agent speaks.
    pub protocol_version: String,
    /// The agent's trust level.
    pub trust_level: TrustLevel,
    /// The agent's trust score (0.0–1.0).
    pub trust_score: f64,
    /// The skills this agent offers.
    pub skills: Vec<AgentSkill>,
    /// The agent's capabilities.
    pub capabilities: AgentCapabilities,
    /// The agent's provider/organization.
    pub provider: Option<AgentProvider>,
    /// Authentication requirements.
    pub authentication: Option<AgentAuthentication>,
    /// Additional metadata.
    pub metadata: serde_json::Value,
    /// When this card was last updated.
    pub updated_at: DateTime<Utc>,
    /// The card schema version (for forward compatibility).
    pub schema_version: String,
}

/// A skill offered by an agent.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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

/// An agent's protocol capabilities.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentCapabilities {
    /// Whether the agent supports streaming responses.
    pub streaming: bool,
    /// Whether the agent can push notifications.
    pub push_notifications: bool,
    /// Whether the agent records state transition history.
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
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentProvider {
    /// Provider organization name.
    pub organization: String,
    /// Provider URL.
    pub url: Option<String>,
}

/// Authentication requirements for connecting to the agent.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentAuthentication {
    /// Authentication scheme (e.g., "bearer", "oauth2").
    pub scheme: String,
    /// Required scopes, if any.
    pub scopes: Vec<String>,
}

/// Trust level in the A2A protocol.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord)]
pub enum TrustLevel {
    /// No trust established.
    #[serde(rename = "unverified")]
    Unverified,
    /// Under observation, limited capabilities.
    #[serde(rename = "probationary")]
    Probationary,
    /// Standard production trust.
    #[serde(rename = "standard")]
    Standard,
    /// Elevated trust, proven track record.
    #[serde(rename = "elevated")]
    Elevated,
    /// Maximum trust, system-critical.
    #[serde(rename = "sovereign")]
    Sovereign,
}

impl TrustLevel {
    /// Convert to a numeric score floor.
    pub fn minimum_score(&self) -> f64 {
        match self {
            TrustLevel::Unverified => 0.0,
            TrustLevel::Probationary => 0.2,
            TrustLevel::Standard => 0.5,
            TrustLevel::Elevated => 0.75,
            TrustLevel::Sovereign => 0.95,
        }
    }

    /// Whether this trust level can initiate A2A communication.
    pub fn can_initiate(&self) -> bool {
        matches!(
            self,
            TrustLevel::Standard | TrustLevel::Elevated | TrustLevel::Sovereign
        )
    }

    /// Whether this trust level can be discovered by other agents.
    pub fn is_discoverable(&self) -> bool {
        matches!(
            self,
            TrustLevel::Standard | TrustLevel::Elevated | TrustLevel::Sovereign
        )
    }
}

impl std::fmt::Display for TrustLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrustLevel::Unverified => write!(f, "unverified"),
            TrustLevel::Probationary => write!(f, "probationary"),
            TrustLevel::Standard => write!(f, "standard"),
            TrustLevel::Elevated => write!(f, "elevated"),
            TrustLevel::Sovereign => write!(f, "sovereign"),
        }
    }
}

impl AgentCard {
    /// Create a minimal agent card with required fields.
    pub fn new(name: &str, url: &str, version: &str) -> Self {
        Self {
            name: name.to_string(),
            description: String::new(),
            url: url.to_string(),
            version: version.to_string(),
            protocol_version: crate::A2A_PROTOCOL_VERSION.to_string(),
            trust_level: TrustLevel::Unverified,
            trust_score: 0.0,
            skills: Vec::new(),
            capabilities: AgentCapabilities::default(),
            provider: None,
            authentication: None,
            metadata: serde_json::Value::Object(serde_json::Map::new()),
            updated_at: Utc::now(),
            schema_version: "1.0.0".to_string(),
        }
    }

    /// Set the description.
    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = desc.to_string();
        self
    }

    /// Set the trust information on the card.
    pub fn with_trust(mut self, level: TrustLevel, score: f64) -> Self {
        self.trust_level = level;
        self.trust_score = score;
        self
    }

    /// Add a skill to the card.
    pub fn with_skill(mut self, skill: AgentSkill) -> Self {
        self.skills.push(skill);
        self
    }

    /// Set the capabilities.
    pub fn with_capabilities(mut self, caps: AgentCapabilities) -> Self {
        self.capabilities = caps;
        self
    }

    /// Set the provider.
    pub fn with_provider(mut self, org: &str) -> Self {
        self.provider = Some(AgentProvider {
            organization: org.to_string(),
            url: None,
        });
        self
    }

    /// Set authentication requirements.
    pub fn with_authentication(mut self, scheme: &str, scopes: Vec<String>) -> Self {
        self.authentication = Some(AgentAuthentication {
            scheme: scheme.to_string(),
            scopes,
        });
        self
    }

    /// Check if this card is compatible with the given protocol version.
    pub fn is_compatible_with(&self, protocol_version: &str) -> bool {
        self.protocol_version == protocol_version
    }

    /// Generate the JSON Schema for this AgentCard (via schemars).
    pub fn json_schema() -> serde_json::Value {
        let schema = schemars::schema_for!(AgentCard);
        serde_json::to_value(&schema).unwrap_or_default()
    }
}

/// Build the default Ferris Aegis AgentCard.
pub fn default_aegis_card(base_url: &str) -> AgentCard {
    AgentCard::new("ferris-aegis", base_url, env!("CARGO_PKG_VERSION"))
        .with_description(
            "Ferris Aegis — The Rust Guardian for Autonomous Intelligence. \
             A trustworthy agent operating system with policy enforcement, \
             audit logging, and credential protection.",
        )
        .with_skill(AgentSkill {
            id: "file_read".to_string(),
            name: "File Read".to_string(),
            description: "Read a file from the local filesystem.".to_string(),
            input_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Absolute path to the file"},
                    "max_bytes": {"type": "integer", "description": "Maximum bytes to read"}
                },
                "required": ["path"]
            })),
            output_schema: None,
            tags: vec!["file".to_string(), "read".to_string(), "filesystem".to_string()],
        })
        .with_skill(AgentSkill {
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
        .with_capabilities(AgentCapabilities::default())
        .with_provider("Ferris Aegis")
}

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
    fn agent_card_path_is_correct() {
        assert_eq!(AGENT_CARD_PATH, "/.well-known/agent-card.json");
        assert_ne!(AGENT_CARD_PATH, "/.well-known/agent.json");
    }

    #[test]
    fn default_card_has_required_skills() {
        let card = default_aegis_card("http://localhost:8080");
        let skill_ids: Vec<&str> = card.skills.iter().map(|s| s.id.as_str()).collect();
        assert!(skill_ids.contains(&"file_read"));
        assert!(skill_ids.contains(&"session_create"));
    }

    #[test]
    fn card_trust_level_ordering() {
        assert!(TrustLevel::Sovereign > TrustLevel::Unverified);
        assert!(!TrustLevel::Unverified.can_initiate());
        assert!(TrustLevel::Standard.can_initiate());
    }

    #[test]
    fn card_json_schema_generation() {
        let schema = AgentCard::json_schema();
        assert!(schema.is_object());
    }
}
