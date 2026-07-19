//! Ferris Aegis A2A — Agent-to-Agent protocol.
//!
//! Implements the Agent-to-Agent (A2A) protocol for inter-agent communication.
//! Every agent exposes a discoverable AgentCard that declares its capabilities,
//! trust level, and communication endpoints. Agents use structured messages
//! with cryptographic attestation to collaborate safely.
//!
//! # Core Concepts
//!
//! - **AgentCard** — A discoverable manifest that describes an agent's identity,
//!   capabilities, trust level, and supported protocols. JSON Schema generated
//!   via `schemars` for validation and OpenAPI compatibility.
//! - **A2A Message** — A structured message between agents with sender/receiver
//!   identity, attestation, and capability requirements.
//! - **Protocol** — The supported A2A protocol version and transports.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The current A2A protocol version.
pub const A2A_PROTOCOL_VERSION: &str = "0.1.0";

/// An AgentCard — the discoverable manifest for an agent.
///
/// The AgentCard is the A2A equivalent of an OpenAPI spec or a
/// Docker image manifest. It tells other agents (and operators)
/// what this agent can do, how to reach it, and how trustworthy
/// it is.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentCard {
    /// The agent's unique name.
    pub name: String,
    /// The agent's version.
    pub version: String,
    /// A human-readable description.
    pub description: String,
    /// The A2A protocol version this agent speaks.
    pub protocol_version: String,
    /// The agent's trust level.
    pub trust_level: TrustLevel,
    /// The agent's trust score.
    pub trust_score: f64,
    /// The agent's capabilities.
    pub capabilities: Vec<String>,
    /// The transports this agent supports (e.g. "http", "stdio").
    pub transports: Vec<String>,
    /// The agent's endpoint URL, if applicable.
    pub endpoint: Option<String>,
    /// The agent's public key (hex-encoded Ed25519).
    pub public_key: Option<String>,
    /// Additional metadata.
    pub metadata: serde_json::Value,
    /// When this card was last updated.
    pub updated_at: DateTime<Utc>,
    /// The card schema version (for forward compatibility).
    pub schema_version: String,
}

impl AgentCard {
    /// Create a minimal agent card with required fields.
    pub fn new(name: &str, version: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            version: version.to_string(),
            description: description.to_string(),
            protocol_version: A2A_PROTOCOL_VERSION.to_string(),
            trust_level: TrustLevel::Unverified,
            trust_score: 0.0,
            capabilities: Vec::new(),
            transports: Vec::new(),
            endpoint: None,
            public_key: None,
            metadata: serde_json::Value::Object(serde_json::Map::new()),
            updated_at: Utc::now(),
            schema_version: "1.0.0".to_string(),
        }
    }

    /// Set the trust information on the card.
    pub fn with_trust(mut self, level: TrustLevel, score: f64) -> Self {
        self.trust_level = level;
        self.trust_score = score;
        self
    }

    /// Add a capability to the card.
    pub fn with_capability(mut self, capability: &str) -> Self {
        self.capabilities.push(capability.to_string());
        self
    }

    /// Add multiple capabilities.
    pub fn with_capabilities(mut self, capabilities: &[&str]) -> Self {
        for cap in capabilities {
            self.capabilities.push(cap.to_string());
        }
        self
    }

    /// Add a transport.
    pub fn with_transport(mut self, transport: &str) -> Self {
        self.transports.push(transport.to_string());
        self
    }

    /// Set the endpoint URL.
    pub fn with_endpoint(mut self, endpoint: &str) -> Self {
        self.endpoint = Some(endpoint.to_string());
        self
    }

    /// Set the public key.
    pub fn with_public_key(mut self, key: &str) -> Self {
        self.public_key = Some(key.to_string());
        self
    }

    /// Check if this card is compatible with the given protocol version.
    pub fn is_compatible_with(&self, protocol_version: &str) -> bool {
        // For now, exact match only. Future versions should support
        // semver-style compatibility checks.
        self.protocol_version == protocol_version
    }

    /// Generate the JSON Schema for this AgentCard (via schemars).
    pub fn json_schema() -> serde_json::Value {
        let schema = schemars::schema_for!(AgentCard);
        serde_json::to_value(&schema).unwrap_or_default()
    }
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
    /// Required trust level for the recipient to process this message.
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
/// messages between agents.
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

    /// Discover agents by capability.
    pub fn agents_with_capability(&self, capability: &str) -> Vec<&AgentCard> {
        self.registry
            .values()
            .filter(|card| card.capabilities.iter().any(|c| c == capability))
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

        // Check protocol compatibility
        if !envelope.is_compatible() {
            return Err(RouteError::IncompatibleProtocol {
                expected: A2A_PROTOCOL_VERSION.to_string(),
                got: envelope.protocol_version.clone(),
            });
        }

        // If the sender's card is included, verify trust
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

    /// The sender has insufficient trust to send this message.
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
        AgentCard::new(name, "1.0.0", &format!("{name} agent"))
            .with_trust(TrustLevel::Standard, 0.7)
            .with_capability("file_read")
            .with_transport("http")
    }

    fn elevated_card(name: &str) -> AgentCard {
        AgentCard::new(name, "1.0.0", &format!("{name} agent"))
            .with_trust(TrustLevel::Elevated, 0.85)
            .with_capabilities(&["file_read", "network_access"])
            .with_transport("http")
    }

    #[test]
    fn agent_card_creation() {
        let card = AgentCard::new("test-agent", "1.0.0", "A test agent");
        assert_eq!(card.name, "test-agent");
        assert_eq!(card.trust_level, TrustLevel::Unverified);
        assert_eq!(card.trust_score, 0.0);
    }

    #[test]
    fn agent_card_with_trust() {
        let card = test_card("trusted-agent");
        assert_eq!(card.trust_level, TrustLevel::Standard);
        assert!((card.trust_score - 0.7).abs() < 0.001);
    }

    #[test]
    fn agent_card_protocol_compatibility() {
        let card = test_card("agent");
        assert!(card.is_compatible_with(A2A_PROTOCOL_VERSION));
        assert!(!card.is_compatible_with("0.2.0"));
    }

    #[test]
    fn agent_card_json_schema() {
        let schema = AgentCard::json_schema();
        assert!(schema.is_object());
        assert!(schema.get("$schema").is_some() || schema.get("title").is_some());
    }

    #[test]
    fn trust_level_ordering() {
        assert!(TrustLevel::Sovereign > TrustLevel::Elevated);
        assert!(TrustLevel::Elevated > TrustLevel::Standard);
        assert!(TrustLevel::Standard > TrustLevel::Probationary);
        assert!(TrustLevel::Probationary > TrustLevel::Unverified);
    }

    #[test]
    fn trust_level_minimum_score() {
        assert!((TrustLevel::Standard.minimum_score() - 0.5).abs() < 0.001);
        assert!((TrustLevel::Sovereign.minimum_score() - 0.95).abs() < 0.001);
    }

    #[test]
    fn trust_level_can_initiate() {
        assert!(!TrustLevel::Unverified.can_initiate());
        assert!(!TrustLevel::Probationary.can_initiate());
        assert!(TrustLevel::Standard.can_initiate());
        assert!(TrustLevel::Elevated.can_initiate());
        assert!(TrustLevel::Sovereign.can_initiate());
    }

    #[test]
    fn a2a_message_creation() {
        let msg = A2aMessage::new(
            "agent-a",
            "agent-b",
            MessageType::Request,
            serde_json::json!({"action": "read_file", "path": "/workspace/data.txt"}),
        );

        assert_eq!(msg.sender, "agent-a");
        assert_eq!(msg.recipient, "agent-b");
        assert_eq!(msg.message_type, MessageType::Request);
    }

    #[test]
    fn a2a_message_trust_verification() {
        let msg = A2aMessage::new(
            "agent-a",
            "agent-b",
            MessageType::Request,
            serde_json::json!({}),
        )
        .with_required_trust(TrustLevel::Standard);

        let elevated = elevated_card("agent-a");
        assert!(msg.verify_trust(&elevated));

        let unverified = AgentCard::new("agent-a", "1.0.0", "low trust")
            .with_trust(TrustLevel::Unverified, 0.1);
        assert!(!msg.verify_trust(&unverified));
    }

    #[test]
    fn a2a_router_register_and_lookup() {
        let mut router = A2aRouter::new();
        let card = test_card("agent-a");
        router.register(card.clone());

        let found = router.lookup("agent-a").unwrap();
        assert_eq!(found.name, "agent-a");
        assert!(router.lookup("agent-b").is_none());
    }

    #[test]
    fn a2a_router_trust_level_filter() {
        let mut router = A2aRouter::new();
        router.register(test_card("standard-agent"));
        router.register(elevated_card("elevated-agent"));
        router.register(
            AgentCard::new("low-agent", "1.0.0", "low trust")
                .with_trust(TrustLevel::Probationary, 0.3),
        );

        let elevated_plus = router.agents_at_trust_level(TrustLevel::Elevated);
        assert_eq!(elevated_plus.len(), 1);
        assert_eq!(elevated_plus[0].name, "elevated-agent");

        let standard_plus = router.agents_at_trust_level(TrustLevel::Standard);
        assert_eq!(standard_plus.len(), 2);
    }

    #[test]
    fn a2a_router_capability_filter() {
        let mut router = A2aRouter::new();
        router.register(test_card("reader"));
        router.register(elevated_card("networker"));

        let readers = router.agents_with_capability("file_read");
        assert_eq!(readers.len(), 2);

        let networkers = router.agents_with_capability("network_access");
        assert_eq!(networkers.len(), 1);
        assert_eq!(networkers[0].name, "networker");
    }

    #[test]
    fn a2a_router_route_message_success() {
        let mut router = A2aRouter::new();
        let sender_card = elevated_card("agent-a");
        let recipient_card = test_card("agent-b");
        router.register(sender_card.clone());
        router.register(recipient_card.clone());

        let msg = A2aMessage::new(
            "agent-a",
            "agent-b",
            MessageType::Request,
            serde_json::json!({"task": "read_file"}),
        );
        let envelope = A2aEnvelope::new(msg).with_sender_card(sender_card);

        let result = router.route_message(&envelope);
        assert!(result.is_ok());
    }

    #[test]
    fn a2a_router_route_message_recipient_not_found() {
        let router = A2aRouter::new();
        let msg = A2aMessage::new(
            "agent-a",
            "nonexistent",
            MessageType::Request,
            serde_json::json!({}),
        );
        let envelope = A2aEnvelope::new(msg);

        let result = router.route_message(&envelope);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RouteError::RecipientNotFound(_)));
    }

    #[test]
    fn a2a_router_route_message_insufficient_trust() {
        let mut router = A2aRouter::new();
        let sender_card = test_card("agent-a");
        let recipient_card = elevated_card("agent-b");
        router.register(sender_card.clone());
        router.register(recipient_card);

        let msg = A2aMessage::new(
            "agent-a",
            "agent-b",
            MessageType::Request,
            serde_json::json!({}),
        )
        .with_required_trust(TrustLevel::Elevated);
        let envelope = A2aEnvelope::new(msg).with_sender_card(sender_card);

        let result = router.route_message(&envelope);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RouteError::InsufficientTrust { .. }));
    }

    #[test]
    fn a2a_envelope_serialization() {
        let msg = A2aMessage::new(
            "agent-a",
            "agent-b",
            MessageType::Request,
            serde_json::json!({"action": "test"}),
        );
        let envelope = A2aEnvelope::new(msg);

        let json = serde_json::to_string(&envelope).unwrap();
        let deserialized: A2aEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(
            deserialized.message.sender,
            "agent-a"
        );
    }
}
