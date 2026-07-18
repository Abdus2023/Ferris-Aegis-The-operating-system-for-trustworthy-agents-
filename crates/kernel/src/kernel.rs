//! # Trust Kernel — The Heart of Ferris Aegis
//!
//! The Trust Kernel is the foundational layer that establishes and maintains
//! trust relationships between all entities in the system. Every agent must
//! be attested and scored before it can participate.
//!
//! ## Core Concepts
//!
//! - **Trust Level** — A categorical classification of how much an agent is trusted
//! - **Trust Score** — A fine-grained numeric score (0.0–1.0) reflecting behavior history
//! - **Attestation** — Cryptographic proof that an agent's identity and capabilities are genuine
//! - **Trust Decay** — Trust scores decay over time without positive reinforcement

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Unique identifier for a trust attestation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct AttestationId(String);

impl AttestationId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

impl Default for AttestationId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for AttestationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "att-{}", self.0)
    }
}

/// Categorical trust levels for agents
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum TrustLevel {
    /// Unverified — no trust established, minimal capabilities
    Unverified = 0,
    /// Probationary — limited trust, under observation
    Probationary = 1,
    /// Standard — baseline trust for production agents
    Standard = 2,
    /// Elevated — agents with proven track records
    Elevated = 3,
    /// Sovereign — highest trust, system-critical agents
    Sovereign = 4,
}

impl TrustLevel {
    /// Get all trust levels in ascending order
    pub fn all() -> &'static [TrustLevel] {
        &[
            TrustLevel::Unverified,
            TrustLevel::Probationary,
            TrustLevel::Standard,
            TrustLevel::Elevated,
            TrustLevel::Sovereign,
        ]
    }

    /// Get the numeric value of this trust level
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }

    /// Get a human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            TrustLevel::Unverified => "unverified",
            TrustLevel::Probationary => "probationary",
            TrustLevel::Standard => "standard",
            TrustLevel::Elevated => "elevated",
            TrustLevel::Sovereign => "sovereign",
        }
    }

    /// Get the minimum trust score required for this level
    pub fn min_score(&self) -> f64 {
        match self {
            TrustLevel::Unverified => 0.0,
            TrustLevel::Probationary => 0.2,
            TrustLevel::Standard => 0.5,
            TrustLevel::Elevated => 0.75,
            TrustLevel::Sovereign => 0.95,
        }
    }
}

impl std::fmt::Display for TrustLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// A fine-grained trust score between 0.0 (no trust) and 1.0 (complete trust)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, PartialOrd)]
pub struct TrustScore(f64);

impl TrustScore {
    /// Create a new trust score, clamped to [0.0, 1.0]
    pub fn new(value: f64) -> Self {
        Self(value.clamp(0.0, 1.0))
    }

    /// Create a trust score of zero (no trust)
    pub fn zero() -> Self {
        Self(0.0)
    }

    /// Create a trust score of one (complete trust)
    pub fn one() -> Self {
        Self(1.0)
    }

    /// Get the raw score value
    pub fn value(&self) -> f64 {
        self.0
    }

    /// Apply positive reinforcement, increasing the score
    pub fn reinforce(&self, delta: f64) -> Self {
        Self::new(self.0 + delta)
    }

    /// Apply negative reinforcement, decreasing the score
    pub fn penalize(&self, delta: f64) -> Self {
        Self::new(self.0 - delta)
    }

    /// Apply time-based trust decay
    pub fn decay(&self, factor: f64) -> Self {
        Self::new(self.0 * factor)
    }

    /// Determine the trust level corresponding to this score
    pub fn level(&self) -> TrustLevel {
        if self.0 >= 0.95 {
            TrustLevel::Sovereign
        } else if self.0 >= 0.75 {
            TrustLevel::Elevated
        } else if self.0 >= 0.5 {
            TrustLevel::Standard
        } else if self.0 >= 0.2 {
            TrustLevel::Probationary
        } else {
            TrustLevel::Unverified
        }
    }
}

impl Default for TrustScore {
    fn default() -> Self {
        Self::zero()
    }
}

impl std::fmt::Display for TrustScore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.3}", self.0)
    }
}

/// A cryptographic attestation of an agent's identity and capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attestation {
    /// Unique attestation identifier
    pub id: AttestationId,
    /// The agent this attestation belongs to
    pub agent_id: crate::agent::AgentId,
    /// When the attestation was issued
    pub issued_at: chrono::DateTime<chrono::Utc>,
    /// When the attestation expires
    pub expires_at: chrono::DateTime<chrono::Utc>,
    /// Hash of the attested agent code/configuration
    pub code_hash: String,
    /// Capabilities attested for this agent
    pub capabilities: Vec<crate::sandbox::Capability>,
    /// Trust level at time of attestation
    pub trust_level: TrustLevel,
}

impl Attestation {
    /// Create a new attestation for an agent
    pub fn new(
        agent_id: crate::agent::AgentId,
        code_hash: String,
        capabilities: Vec<crate::sandbox::Capability>,
        trust_level: TrustLevel,
        ttl: chrono::Duration,
    ) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: AttestationId::new(),
            agent_id,
            issued_at: now,
            expires_at: now + ttl,
            code_hash,
            capabilities,
            trust_level,
        }
    }

    /// Check if the attestation has expired
    pub fn is_expired(&self) -> bool {
        chrono::Utc::now() > self.expires_at
    }
}

/// Record of an agent's trust state in the kernel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustRecord {
    /// The agent this record belongs to
    pub agent_id: crate::agent::AgentId,
    /// Current trust score
    pub score: TrustScore,
    /// Current trust level
    pub level: TrustLevel,
    /// Active attestation, if any
    pub attestation: Option<Attestation>,
    /// Number of positive interactions
    pub positive_interactions: u64,
    /// Number of negative interactions
    pub negative_interactions: u64,
    /// When the agent was first registered
    pub registered_at: chrono::DateTime<chrono::Utc>,
    /// Last time the trust score was updated
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

/// The Trust Kernel — central authority for trust management
#[derive(Debug)]
pub struct TrustKernel {
    /// Trust records indexed by agent ID
    records: HashMap<crate::agent::AgentId, TrustRecord>,
    /// Default trust score for new agents
    initial_score: TrustScore,
    /// Trust decay factor per interval
    decay_factor: f64,
    /// Minimum score before an agent is suspended
    suspension_threshold: TrustScore,
}

impl TrustKernel {
    /// Create a new trust kernel with default settings
    pub fn new() -> Self {
        Self {
            records: HashMap::new(),
            initial_score: TrustScore::new(0.1),
            decay_factor: 0.999,
            suspension_threshold: TrustScore::new(0.05),
        }
    }

    /// Create a trust kernel with custom initial score
    pub fn with_initial_score(mut self, score: f64) -> Self {
        self.initial_score = TrustScore::new(score);
        self
    }

    /// Create a trust kernel with custom decay factor
    pub fn with_decay_factor(mut self, factor: f64) -> Self {
        self.decay_factor = factor.clamp(0.0, 1.0);
        self
    }

    /// Register a new agent with the trust kernel
    pub fn register(&mut self, agent_id: &crate::agent::AgentId) -> &TrustRecord {
        let now = chrono::Utc::now();
        let record = TrustRecord {
            agent_id: agent_id.clone(),
            score: self.initial_score,
            level: self.initial_score.level(),
            attestation: None,
            positive_interactions: 0,
            negative_interactions: 0,
            registered_at: now,
            last_updated: now,
        };
        self.records.insert(agent_id.clone(), record);
        self.records.get(agent_id).unwrap()
    }

    /// Get the trust record for an agent
    pub fn get_record(&self, agent_id: &crate::agent::AgentId) -> Option<&TrustRecord> {
        self.records.get(agent_id)
    }

    /// Get a mutable reference to a trust record
    fn get_record_mut(&mut self, agent_id: &crate::agent::AgentId) -> Option<&mut TrustRecord> {
        self.records.get_mut(agent_id)
    }

    /// Apply positive reinforcement to an agent's trust score
    pub fn reinforce(&mut self, agent_id: &crate::agent::AgentId, delta: f64) -> Option<TrustScore> {
        let record = self.get_record_mut(agent_id)?;
        record.score = record.score.reinforce(delta);
        record.level = record.score.level();
        record.positive_interactions += 1;
        record.last_updated = chrono::Utc::now();
        Some(record.score)
    }

    /// Apply a penalty to an agent's trust score
    pub fn penalize(&mut self, agent_id: &crate::agent::AgentId, delta: f64) -> Option<TrustScore> {
        let record = self.get_record_mut(agent_id)?;
        record.score = record.score.penalize(delta);
        record.level = record.score.level();
        record.negative_interactions += 1;
        record.last_updated = chrono::Utc::now();
        Some(record.score)
    }

    /// Check if an agent should be suspended due to low trust
    pub fn should_suspend(&self, agent_id: &crate::agent::AgentId) -> bool {
        self.records
            .get(agent_id)
            .map(|r| r.score <= self.suspension_threshold)
            .unwrap_or(true)
    }

    /// Apply trust decay to all registered agents
    pub fn apply_decay(&mut self) {
        let now = chrono::Utc::now();
        for record in self.records.values_mut() {
            record.score = record.score.decay(self.decay_factor);
            record.level = record.score.level();
            record.last_updated = now;
        }
    }

    /// Attest an agent, granting it a formal attestation
    pub fn attest(
        &mut self,
        agent_id: &crate::agent::AgentId,
        code_hash: String,
        capabilities: Vec<crate::sandbox::Capability>,
        ttl: chrono::Duration,
    ) -> Option<&Attestation> {
        let record = self.get_record_mut(agent_id)?;
        let attestation = Attestation::new(
            agent_id.clone(),
            code_hash,
            capabilities,
            record.level,
            ttl,
        );
        record.attestation = Some(attestation);
        record.last_updated = chrono::Utc::now();
        Some(record.attestation.as_ref().unwrap())
    }

    /// List all registered agents and their trust levels
    pub fn list_agents(&self) -> Vec<(&crate::agent::AgentId, TrustLevel, TrustScore)> {
        self.records
            .iter()
            .map(|(id, r)| (id, r.level, r.score))
            .collect()
    }

    /// Get the number of registered agents
    pub fn agent_count(&self) -> usize {
        self.records.len()
    }
}

impl Default for TrustKernel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_agent_id() -> crate::agent::AgentId {
        crate::agent::AgentId::new("test-agent")
    }

    #[test]
    fn test_trust_score_clamping() {
        let score = TrustScore::new(1.5);
        assert!((score.value() - 1.0).abs() < f64::EPSILON);

        let score = TrustScore::new(-0.5);
        assert!((score.value() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_trust_score_level_mapping() {
        assert_eq!(TrustScore::new(0.0).level(), TrustLevel::Unverified);
        assert_eq!(TrustScore::new(0.3).level(), TrustLevel::Probationary);
        assert_eq!(TrustScore::new(0.6).level(), TrustLevel::Standard);
        assert_eq!(TrustScore::new(0.8).level(), TrustLevel::Elevated);
        assert_eq!(TrustScore::new(0.97).level(), TrustLevel::Sovereign);
    }

    #[test]
    fn test_trust_kernel_register() {
        let mut kernel = TrustKernel::new();
        let agent_id = test_agent_id();
        let record = kernel.register(&agent_id);
        assert_eq!(record.agent_id, agent_id);
        assert_eq!(record.level, TrustLevel::Unverified);
    }

    #[test]
    fn test_trust_kernel_reinforce() {
        let mut kernel = TrustKernel::new();
        let agent_id = test_agent_id();
        kernel.register(&agent_id);

        let new_score = kernel.reinforce(&agent_id, 0.3).unwrap();
        assert!(new_score.value() > 0.1);
    }

    #[test]
    fn test_trust_kernel_penalize() {
        let mut kernel = TrustKernel::new();
        let agent_id = test_agent_id();
        kernel.register(&agent_id);

        let initial = kernel.get_record(&agent_id).unwrap().score;
        kernel.reinforce(&agent_id, 0.5);
        let new_score = kernel.penalize(&agent_id, 0.3).unwrap();
        assert!(new_score.value() < initial.value() + 0.5);
    }

    #[test]
    fn test_trust_decay() {
        let mut kernel = TrustKernel::new().with_decay_factor(0.9);
        let agent_id = test_agent_id();
        kernel.register(&agent_id);
        kernel.reinforce(&agent_id, 0.5);

        let before = kernel.get_record(&agent_id).unwrap().score;
        kernel.apply_decay();
        let after = kernel.get_record(&agent_id).unwrap().score;
        assert!(after.value() < before.value());
    }

    #[test]
    fn test_attestation_creation() {
        let attestation = Attestation::new(
            test_agent_id(),
            "abc123".to_string(),
            vec![crate::sandbox::Capability::NetworkAccess],
            TrustLevel::Standard,
            chrono::Duration::hours(24),
        );
        assert!(!attestation.is_expired());
    }

    #[test]
    fn test_suspension_threshold() {
        let mut kernel = TrustKernel::new();
        let agent_id = test_agent_id();
        kernel.register(&agent_id);
        // Initial score is 0.1, above the 0.05 threshold
        assert!(!kernel.should_suspend(&agent_id));

        // Penalize below threshold
        kernel.penalize(&agent_id, 0.1);
        assert!(kernel.should_suspend(&agent_id));
    }
}
