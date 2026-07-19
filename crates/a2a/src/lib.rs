//! Ferris Aegis A2A — Agent-to-Agent protocol.
//!
//! Implements the Agent-to-Agent (A2A) protocol for inter-agent communication.
//! Every agent exposes a discoverable `AgentCard` that declares its capabilities,
//! skills, trust level, and communication endpoints.
//!
//! # Architecture
//!
//! This crate supports two deployment modes:
//!
//! ## Branch A — Standalone (`branch_a`)
//!
//! Full A2A protocol stack: AgentCard discovery server at
//! `/.well-known/agent-card.json`, task handler, and A2A client.
//! Use when Ferris Aegis needs to be discoverable by agents
//! you don't control.
//!
//! ## Branch B — Integrated (`branch_b`)
//!
//! Skip the AgentCard server. Expose Phase 4 capabilities (session
//! management, supervision, semantic memory) as MCP tools that the
//! TypeScript orchestrator calls through the existing MCP interface
//! from Phase 2. No new protocol surface.
//!
//! # Core Concepts
//!
//! - **AgentCard** — Discoverable manifest describing identity, skills,
//!   capabilities, trust level, and endpoint. Served at the
//!   `/.well-known/agent-card.json` well-known URI per RFC 8615.
//!   JSON Schema generated via `schemars`.
//!
//! - **A2A Task** — A unit of work sent between agents with a defined
//!   lifecycle: Submitted → Working → Completed / Cancelled / Failed.
//!
//! - **A2A Router** — Trust-gated message routing. Maintains a registry
//!   of known AgentCards and enforces trust-level requirements on
//!   inter-agent communication.

pub mod agent_card;
pub mod branch_a;
pub mod branch_b;
pub mod task;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// Re-export everything from submodules
pub use agent_card::{
    AgentCapabilities, AgentCard, AgentProvider, AgentAuthentication,
    AgentSkill, TrustLevel, default_aegis_card, AGENT_CARD_PATH,
};
pub use branch_a::{A2aServerConfig, serve_a2a, send_task_to_remote};
pub use branch_b::{
    BudgetStatus, SessionCreateParams, SessionStatusParams,
    SupervisorInspectParams, MemorySearchParams, ConceptStoreParams,
    AgentCardQueryParams,
};
pub use task::{A2aTask, TaskResult, TaskState, send_task};

/// The current A2A protocol version.
pub const A2A_PROTOCOL_VERSION: &str = "0.1.0";

/// The type of an A2A message.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub enum MessageType {
    /// A request for information or action.
    #[serde(rename = "request")]
    Request,
    /// A response to a previous request.
    #[serde(rename = "response")]
    Response,
    /// A one-way notification (no response expected).
    #[serde(rename = "notification")]
    Notification,
    /// An AgentCard discovery query.
    #[serde(rename = "discovery")]
    Discovery,
    /// An AgentCard disclosure.
    #[serde(rename = "agent_card")]
    AgentCardDisclosure,
}

impl std::fmt::Display for MessageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MessageType::Request => write!(f, "request"),
            MessageType::Response => write!(f, "response"),
            MessageType::Notification => write!(f, "notification"),
            MessageType::Discovery => write!(f, "discovery"),
            MessageType::AgentCardDisclosure => write!(f, "agent_card"),
        }
    }
}

/// An A2A message between agents.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct A2aMessage {
    /// Unique message identifier.
    pub id: String,
    /// The sender agent ID.
    pub sender: String,
    /// The recipient agent ID.
    pub recipient: String,
    /// The message type.
    pub message_type: MessageType,
    /// The message content (serialized as JSON).
    pub content: serde_json::Value,
    /// The session ID this message belongs to, if any.
    pub session_id: Option<String>,
    /// Required trust level for the recipient.
    pub required_trust: Option<TrustLevel>,
    /// When the message was created.
    pub timestamp: DateTime<Utc>,
    /// Optional cryptographic attestation.
    pub attestation: Option<Attestation>,
}

impl A2aMessage {
    /// Create a new A2A message.
    pub fn new(
        sender: &str,
        recipient: &str,
        message_type: MessageType,
        content: serde_json::Value,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            sender: sender.to_string(),
            recipient: recipient.to_string(),
            message_type,
            content,
            session_id: None,
            required_trust: None,
            timestamp: Utc::now(),
            attestation: None,
        }
    }

    /// Set the session ID.
    pub fn with_session(mut self, session_id: &str) -> Self {
        self.session_id = Some(session_id.to_string());
        self
    }

    /// Require a minimum trust level from the recipient.
    pub fn with_required_trust(mut self, level: TrustLevel) -> Self {
        self.required_trust = Some(level);
        self
    }

    /// Attach a cryptographic attestation.
    pub fn with_attestation(mut self, attestation: Attestation) -> Self {
        self.attestation = Some(attestation);
        self
    }

    /// Check if the sender meets the required trust level.
    pub fn verify_trust(&self, sender_card: &AgentCard) -> bool {
        if let Some(required) = &self.required_trust {
            sender_card.trust_level >= *required
        } else {
            true
        }
    }
}

/// Cryptographic attestation for an A2A message.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Attestation {
    /// The signature algorithm (e.g. "Ed25519").
    pub algorithm: String,
    /// The hex-encoded signature over the message content.
    pub signature: String,
    /// The signer's public key (hex-encoded).
    pub public_key: String,
}

/// An A2A message envelope for transport.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct A2aEnvelope {
    /// The A2A protocol version.
    pub protocol_version: String,
    /// The message inside the envelope.
    pub message: A2aMessage,
    /// The sender's AgentCard (optional, for discovery).
    pub sender_card: Option<AgentCard>,
}

impl A2aEnvelope {
    /// Wrap a message in an envelope.
    pub fn new(message: A2aMessage) -> Self {
        Self {
            protocol_version: A2A_PROTOCOL_VERSION.to_string(),
            message,
            sender_card: None,
        }
    }

    /// Include the sender's AgentCard.
    pub fn with_sender_card(mut self, card: AgentCard) -> Self {
        self.sender_card = Some(card);
        self
    }

    /// Check protocol compatibility.
    pub fn is_compatible(&self) -> bool {
        self.protocol_version == A2A_PROTOCOL_VERSION
    }
}

/// An A2A router for managing agent-to-agent communication.
///
/// The router maintains a registry of known AgentCards and routes
/// messages between agents with trust-level enforcement.
#[derive(Debug, Clone)]
pub struct A2aRouter {
    /// Registry of known agents by name.
    registry: std::collections::HashMap<String, AgentCard>,
}

impl A2aRouter {
    /// Create a new A2A router.
    pub fn new() -> Self {
        Self {
            registry: std::collections::HashMap::new(),
        }
    }

    /// Register an agent's card.
    pub fn register(&mut self, card: AgentCard) {
        tracing::info!(
            agent = %card.name,
            trust_level = %card.trust_level,
            "Agent registered in A2A router"
        );
        self.registry.insert(card.name.clone(), card);
    }

    /// Unregister an agent.
    pub fn unregister(&mut self, name: &str) -> Option<AgentCard> {
        let card = self.registry.remove(name);
        if card.is_some() {
            tracing::info!(agent = name, "Agent unregistered from A2A router");
        }
        card
    }

    /// Look up an agent's card.
    pub fn lookup(&self, name: &str) -> Option<&AgentCard> {
        self.registry.get(name)
    }

    /// List all registered agents.
    pub fn list_agents(&self) -> Vec<&AgentCard> {
        self.registry.values().collect()
    }

    /// List agents at or above a given trust level.
    pub fn agents_at_trust_level(&self, level: TrustLevel) -> Vec<&AgentCard> {
        self.registry
            .values()
            .filter(|card| card.trust_level >= level)
            .collect()
    }

    /// Discover agents by skill ID.
    pub fn agents_with_skill(&self, skill_id: &str) -> Vec<&AgentCard> {
        self.registry
            .values()
            .filter(|card| card.skills.iter().any(|s| s.id == skill_id))
            .collect()
    }

    /// Route a message to the appropriate recipient.
    ///
    /// Returns an error if the recipient is not registered or if
    /// the sender's trust level is insufficient.
    pub fn route_message(
        &self,
        envelope: &A2aEnvelope,
    ) -> Result<AgentCard, RouteError> {
        let recipient = self.lookup(&envelope.message.recipient).ok_or_else(|| {
            RouteError::RecipientNotFound(envelope.message.recipient.clone())
        })?;

        if !envelope.is_compatible() {
            return Err(RouteError::IncompatibleProtocol {
                expected: A2A_PROTOCOL_VERSION.to_string(),
                got: envelope.protocol_version.clone(),
            });
        }

        if let Some(sender_card) = &envelope.sender_card {
            if !sender_card.is_compatible_with(&recipient.protocol_version) {
                return Err(RouteError::IncompatibleProtocol {
                    expected: recipient.protocol_version.clone(),
                    got: sender_card.protocol_version.clone(),
                });
            }

            if !envelope.message.verify_trust(sender_card) {
                return Err(RouteError::InsufficientTrust {
                    sender: sender_card.trust_level,
                    required: envelope
                        .message
                        .required_trust
                        .unwrap_or(TrustLevel::Standard),
                });
            }

            if !sender_card.trust_level.can_initiate() {
                return Err(RouteError::CannotInitiate {
                    sender: sender_card.name.clone(),
                    level: sender_card.trust_level,
                });
            }
        }

        Ok(recipient.clone())
    }

    /// Number of registered agents.
    pub fn agent_count(&self) -> usize {
        self.registry.len()
    }
}

impl Default for A2aRouter {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that can occur during message routing.
#[derive(Debug, Clone, thiserror::Error)]
pub enum RouteError {
    /// The recipient was not found in the registry.
    #[error("recipient not found: {0}")]
    RecipientNotFound(String),

    /// Protocol version mismatch.
    #[error("incompatible protocol: expected {expected}, got {got}")]
    IncompatibleProtocol {
        /// The expected protocol version.
        expected: String,
        /// The actual protocol version.
        got: String,
    },

    /// The sender has insufficient trust.
    #[error("insufficient trust: sender is {sender}, required {required}")]
    InsufficientTrust {
        /// The sender's trust level.
        sender: TrustLevel,
        /// The required trust level.
        required: TrustLevel,
    },

    /// The sender cannot initiate A2A communication.
    #[error("{sender} (level={level}) cannot initiate A2A communication")]
    CannotInitiate {
        /// The sender's name.
        sender: String,
        /// The sender's trust level.
        level: TrustLevel,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_card(name: &str) -> AgentCard {
        AgentCard::new(name, &format!("http://{name}.local"), "1.0.0")
            .with_trust(TrustLevel::Standard, 0.7)
    }

    fn elevated_card(name: &str) -> AgentCard {
        AgentCard::new(name, &format!("http://{name}.local"), "1.0.0")
            .with_trust(TrustLevel::Elevated, 0.85)
    }

    #[test]
    fn router_register_and_lookup() {
        let mut router = A2aRouter::new();
        let card = test_card("agent-a");
        router.register(card.clone());

        let found = router.lookup("agent-a").unwrap();
        assert_eq!(found.name, "agent-a");
        assert!(router.lookup("agent-b").is_none());
    }

    #[test]
    fn router_trust_level_filter() {
        let mut router = A2aRouter::new();
        router.register(test_card("standard-agent"));
        router.register(elevated_card("elevated-agent"));
        router.register(
            AgentCard::new("low-agent", "http://low.local", "1.0.0")
                .with_trust(TrustLevel::Probationary, 0.3),
        );

        let elevated_plus = router.agents_at_trust_level(TrustLevel::Elevated);
        assert_eq!(elevated_plus.len(), 1);

        let standard_plus = router.agents_at_trust_level(TrustLevel::Standard);
        assert_eq!(standard_plus.len(), 2);
    }

    #[test]
    fn router_skill_filter() {
        let mut router = A2aRouter::new();
        let card = test_card("reader")
            .with_skill(AgentSkill {
                id: "file_read".to_string(),
                name: "File Read".to_string(),
                description: "Read files".to_string(),
                input_schema: None,
                output_schema: None,
                tags: vec![],
            });
        router.register(card);

        let readers = router.agents_with_skill("file_read");
        assert_eq!(readers.len(), 1);
        assert_eq!(router.agents_with_skill("nonexistent").len(), 0);
    }

    #[test]
    fn route_message_success() {
        let mut router = A2aRouter::new();
        let sender_card = elevated_card("agent-a");
        let recipient_card = test_card("agent-b");
        router.register(sender_card.clone());
        router.register(recipient_card.clone());

        let msg = A2aMessage::new(
            "agent-a", "agent-b",
            MessageType::Request,
            serde_json::json!({"task": "read_file"}),
        );
        let envelope = A2aEnvelope::new(msg).with_sender_card(sender_card);
        assert!(router.route_message(&envelope).is_ok());
    }

    #[test]
    fn route_message_recipient_not_found() {
        let router = A2aRouter::new();
        let msg = A2aMessage::new(
            "agent-a", "nonexistent",
            MessageType::Request,
            serde_json::json!({}),
        );
        let envelope = A2aEnvelope::new(msg);

        let result = router.route_message(&envelope);
        assert!(matches!(result.unwrap_err(), RouteError::RecipientNotFound(_)));
    }

    #[test]
    fn route_message_insufficient_trust() {
        let mut router = A2aRouter::new();
        let sender_card = test_card("agent-a");
        let recipient_card = elevated_card("agent-b");
        router.register(sender_card.clone());
        router.register(recipient_card);

        let msg = A2aMessage::new(
            "agent-a", "agent-b",
            MessageType::Request,
            serde_json::json!({}),
        )
        .with_required_trust(TrustLevel::Elevated);
        let envelope = A2aEnvelope::new(msg).with_sender_card(sender_card);

        let result = router.route_message(&envelope);
        assert!(matches!(result.unwrap_err(), RouteError::InsufficientTrust { .. }));
    }

    #[test]
    fn envelope_serialization_roundtrip() {
        let msg = A2aMessage::new(
            "agent-a", "agent-b",
            MessageType::Request,
            serde_json::json!({"action": "test"}),
        );
        let envelope = A2aEnvelope::new(msg);

        let json = serde_json::to_string(&envelope).unwrap();
        let deserialized: A2aEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.message.sender, "agent-a");
    }
}
