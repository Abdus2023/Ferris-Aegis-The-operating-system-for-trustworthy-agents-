//! # Audit Ledger — Immutable Accountability for Agent Actions
//!
//! The Audit Ledger provides a cryptographically chained, append-only log
//! of every significant action taken by agents in the system. Each entry
//! is linked to the previous one via a SHA-256 hash, creating a tamper-evident
//! chain similar to a blockchain.
//!
//! ## Properties
//!
//! - **Append-only** — entries can never be modified or deleted
//! - **Cryptographically chained** — each entry includes the hash of the previous
//! - **Verifiable** — the entire chain can be verified for integrity
//! - **Queryable** — entries can be filtered by agent, action type, or time range

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::agent::AgentId;

/// Unique identifier for an audit entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct AuditEntryId(String);

impl AuditEntryId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

impl Default for AuditEntryId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for AuditEntryId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "aud-{}", self.0)
    }
}

/// Severity level for audit entries
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum AuditSeverity {
    /// Informational — routine actions
    Info = 0,
    /// Warning — potentially concerning behavior
    Warning = 1,
    /// Critical — policy violations or security events
    Critical = 2,
    /// Emergency — immediate intervention required
    Emergency = 3,
}

impl std::fmt::Display for AuditSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditSeverity::Info => write!(f, "INFO"),
            AuditSeverity::Warning => write!(f, "WARN"),
            AuditSeverity::Critical => write!(f, "CRITICAL"),
            AuditSeverity::Emergency => write!(f, "EMERGENCY"),
        }
    }
}

/// A single entry in the audit ledger
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Unique entry identifier
    pub id: AuditEntryId,
    /// The agent that performed the action
    pub agent_id: AgentId,
    /// The action that was performed
    pub action: String,
    /// Target of the action
    pub target: String,
    /// Whether the action was allowed or denied
    pub allowed: bool,
    /// Severity of this entry
    pub severity: AuditSeverity,
    /// Timestamp of the action
    pub timestamp: DateTime<Utc>,
    /// Hash of this entry's contents
    pub hash: String,
    /// Hash of the previous entry (chain link)
    pub prev_hash: String,
    /// Additional metadata
    pub metadata: serde_json::Value,
}

impl AuditEntry {
    /// Compute the hash of this entry's contents
    fn compute_hash(
        agent_id: &AgentId,
        action: &str,
        target: &str,
        allowed: bool,
        severity: AuditSeverity,
        timestamp: &DateTime<Utc>,
        prev_hash: &str,
    ) -> String {
        let mut hasher = Sha256::new();
        hasher.update(agent_id.to_string().as_bytes());
        hasher.update(action.as_bytes());
        hasher.update(target.as_bytes());
        hasher.update(if allowed { "1" } else { "0" }.as_bytes());
        hasher.update((severity as u8).to_le_bytes());
        hasher.update(timestamp.to_rfc3339().as_bytes());
        hasher.update(prev_hash.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Create a new audit entry
    pub fn new(
        agent_id: AgentId,
        action: String,
        target: String,
        allowed: bool,
        severity: AuditSeverity,
        prev_hash: String,
    ) -> Self {
        let timestamp = Utc::now();
        let hash = Self::compute_hash(
            &agent_id,
            &action,
            &target,
            allowed,
            severity,
            &timestamp,
            &prev_hash,
        );
        Self {
            id: AuditEntryId::new(),
            agent_id,
            action,
            target,
            allowed,
            severity,
            timestamp,
            hash,
            prev_hash,
            metadata: serde_json::Value::Null,
        }
    }

    /// Verify this entry's hash is consistent with its contents
    pub fn verify_hash(&self) -> bool {
        let computed = Self::compute_hash(
            &self.agent_id,
            &self.action,
            &self.target,
            self.allowed,
            self.severity,
            &self.timestamp,
            &self.prev_hash,
        );
        computed == self.hash
    }
}

/// The Audit Ledger — an append-only, cryptographically chained log
#[derive(Debug)]
pub struct AuditLedger {
    /// All entries in order
    entries: Vec<AuditEntry>,
    /// The genesis hash (hash before the first entry)
    genesis_hash: String,
}

impl AuditLedger {
    /// Create a new empty ledger
    pub fn new() -> Self {
        let mut hasher = Sha256::new();
        hasher.update(b"ferris-aegis-genesis");
        let genesis_hash = hex::encode(hasher.finalize());

        Self {
            entries: Vec::new(),
            genesis_hash,
        }
    }

    /// Append a new entry to the ledger
    pub fn append(
        &mut self,
        agent_id: AgentId,
        action: String,
        target: String,
        allowed: bool,
        severity: AuditSeverity,
    ) -> &AuditEntry {
        let prev_hash = self
            .entries
            .last()
            .map(|e| e.hash.clone())
            .unwrap_or_else(|| self.genesis_hash.clone());

        let entry = AuditEntry::new(agent_id, action, target, allowed, severity, prev_hash);
        self.entries.push(entry);
        self.entries.last().unwrap()
    }

    /// Verify the entire chain is intact
    pub fn verify_chain(&self) -> bool {
        let mut prev_hash = self.genesis_hash.clone();

        for entry in &self.entries {
            // Verify the entry links to the correct previous hash
            if entry.prev_hash != prev_hash {
                return false;
            }
            // Verify the entry's own hash is correct
            if !entry.verify_hash() {
                return false;
            }
            prev_hash = entry.hash.clone();
        }

        true
    }

    /// Get all entries
    pub fn entries(&self) -> &[AuditEntry] {
        &self.entries
    }

    /// Get mutable access to entries (for testing tamper detection)
    pub fn entries_mut(&mut self) -> &mut [AuditEntry] {
        &mut self.entries
    }

    /// Get entries for a specific agent
    pub fn entries_for_agent(&self, agent_id: &AgentId) -> Vec<&AuditEntry> {
        self.entries
            .iter()
            .filter(|e| &e.agent_id == agent_id)
            .collect()
    }

    /// Get entries within a time range
    pub fn entries_in_range(
        &self,
        start: &DateTime<Utc>,
        end: &DateTime<Utc>,
    ) -> Vec<&AuditEntry> {
        self.entries
            .iter()
            .filter(|e| &e.timestamp >= start && &e.timestamp <= end)
            .collect()
    }

    /// Get entries of a specific severity or higher
    pub fn entries_with_severity(&self, min_severity: AuditSeverity) -> Vec<&AuditEntry> {
        self.entries
            .iter()
            .filter(|e| e.severity >= min_severity)
            .collect()
    }

    /// Get the number of entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the ledger is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get the latest hash (tip of the chain)
    pub fn latest_hash(&self) -> &str {
        self.entries
            .last()
            .map(|e| e.hash.as_str())
            .unwrap_or(&self.genesis_hash)
    }
}

impl Default for AuditLedger {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentId;

    fn test_agent_id() -> AgentId {
        AgentId::new("test-agent")
    }

    #[test]
    fn test_ledger_append() {
        let mut ledger = AuditLedger::new();
        let agent_id = test_agent_id();

        let entry = ledger.append(
            agent_id.clone(),
            "file:read".to_string(),
            "/workspace/data.txt".to_string(),
            true,
            AuditSeverity::Info,
        );

        assert!(entry.allowed);
        assert_eq!(entry.action, "file:read");
        assert_eq!(ledger.len(), 1);
    }

    #[test]
    fn test_ledger_chain_integrity() {
        let mut ledger = AuditLedger::new();
        let agent_id = test_agent_id();

        for i in 0..10 {
            ledger.append(
                agent_id.clone(),
                format!("action:{}", i),
                format!("target:{}", i),
                true,
                AuditSeverity::Info,
            );
        }

        assert!(ledger.verify_chain());
        assert_eq!(ledger.len(), 10);
    }

    #[test]
    fn test_ledger_chain_tamper_detection() {
        let mut ledger = AuditLedger::new();
        let agent_id = test_agent_id();

        ledger.append(
            agent_id.clone(),
            "action:1".to_string(),
            "target:1".to_string(),
            true,
            AuditSeverity::Info,
        );
        ledger.append(
            agent_id.clone(),
            "action:2".to_string(),
            "target:2".to_string(),
            true,
            AuditSeverity::Info,
        );

        // Tamper with the first entry
        ledger.entries[0].action = "tampered".to_string();

        // Chain should no longer be valid
        assert!(!ledger.verify_chain());
    }

    #[test]
    fn test_entry_hash_verification() {
        let agent_id = test_agent_id();
        let entry = AuditEntry::new(
            agent_id,
            "file:write".to_string(),
            "/workspace/test.txt".to_string(),
            false,
            AuditSeverity::Warning,
            "genesis".to_string(),
        );

        assert!(entry.verify_hash());
    }

    #[test]
    fn test_entries_by_severity() {
        let mut ledger = AuditLedger::new();
        let agent_id = test_agent_id();

        ledger.append(
            agent_id.clone(),
            "action:1".to_string(),
            "t:1".to_string(),
            true,
            AuditSeverity::Info,
        );
        ledger.append(
            agent_id.clone(),
            "action:2".to_string(),
            "t:2".to_string(),
            false,
            AuditSeverity::Critical,
        );
        ledger.append(
            agent_id.clone(),
            "action:3".to_string(),
            "t:3".to_string(),
            true,
            AuditSeverity::Warning,
        );

        let critical = ledger.entries_with_severity(AuditSeverity::Critical);
        assert_eq!(critical.len(), 1);

        let warnings_and_above = ledger.entries_with_severity(AuditSeverity::Warning);
        assert_eq!(warnings_and_above.len(), 2);
    }

    #[test]
    fn test_entries_for_agent() {
        let mut ledger = AuditLedger::new();
        let agent1 = AgentId::new("agent-1");
        let agent2 = AgentId::new("agent-2");

        ledger.append(
            agent1.clone(),
            "action:1".to_string(),
            "t:1".to_string(),
            true,
            AuditSeverity::Info,
        );
        ledger.append(
            agent2.clone(),
            "action:2".to_string(),
            "t:2".to_string(),
            true,
            AuditSeverity::Info,
        );

        assert_eq!(ledger.entries_for_agent(&agent1).len(), 1);
        assert_eq!(ledger.entries_for_agent(&agent2).len(), 1);
    }
}
