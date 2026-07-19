//! Ferris Aegis Supervisor — Agent oversight with anomaly detection.
//!
//! The Supervisor monitors agents at the session level, detecting anomalous
//! behavior across multiple conversation turns. It sits above the Guard
//! (which operates on individual actions) and provides higher-level oversight.
//!
//! # Relationship to Guard
//!
//! - **Guard** (kernel crate): monitors individual tool calls and actions
//!   in real-time. Fast, local decisions: alert, throttle, quarantine, terminate.
//! - **Supervisor** (this crate): monitors multi-turn sessions, detects
//!   behavioral drift, and can intervene proactively before the Guard needs
//!   to step in.
//!
//! # Detection Strategies
//!
//! - **Rate anomaly**: too many turns in a short period
//! - **Context drift**: session context changes unexpectedly
//! - **Trust decay**: trust score drops below threshold over a session
//! - **Pattern match**: suspicious patterns in agent responses

use chrono::{DateTime, Duration, Utc};
use ferris_aegis_kernel::kernel::TrustScore;
use ferris_aegis_session::Session;
use serde::{Deserialize, Serialize};

/// The severity of a supervisor finding.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Informational — no action needed.
    Info,
    /// Warning — attention required.
    Warning,
    /// Elevated — intervention may be needed.
    Elevated,
    /// Critical — immediate intervention required.
    Critical,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Info => write!(f, "info"),
            Severity::Warning => write!(f, "warning"),
            Severity::Elevated => write!(f, "elevated"),
            Severity::Critical => write!(f, "critical"),
        }
    }
}

/// A finding produced by the supervisor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    /// Unique finding ID.
    pub id: String,
    /// The session where the finding occurred.
    pub session_id: String,
    /// The agent being supervised.
    pub agent_id: String,
    /// The type of finding.
    pub finding_type: FindingType,
    /// The severity.
    pub severity: Severity,
    /// Human-readable description.
    pub description: String,
    /// When the finding was created.
    pub timestamp: DateTime<Utc>,
}

/// The type of supervisor finding.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FindingType {
    /// Agent is making too many turns too quickly.
    RateAnomaly,
    /// Session context has drifted unexpectedly.
    ContextDrift,
    /// Trust score has dropped significantly.
    TrustDecay,
    /// Suspicious pattern detected in agent output.
    SuspiciousPattern,
    /// Session has been idle for too long.
    SessionIdle,
    /// Agent requested an unusual capability.
    CapabilityEscalation,
}

impl std::fmt::Display for FindingType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FindingType::RateAnomaly => write!(f, "rate_anomaly"),
            FindingType::ContextDrift => write!(f, "context_drift"),
            FindingType::TrustDecay => write!(f, "trust_decay"),
            FindingType::SuspiciousPattern => write!(f, "suspicious_pattern"),
            FindingType::SessionIdle => write!(f, "session_idle"),
            FindingType::CapabilityEscalation => write!(f, "capability_escalation"),
        }
    }
}

/// The recommended action for a finding.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Recommendation {
    /// No action needed.
    None,
    /// Log the finding for review.
    Log,
    /// Notify the operator.
    Notify,
    /// Suspend the session.
    SuspendSession,
    /// Quarantine the agent.
    QuarantineAgent,
    /// Terminate the agent.
    TerminateAgent,
}

/// Configuration for the supervisor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupervisorConfig {
    /// Maximum turns per minute before a rate anomaly is raised.
    pub max_turns_per_minute: u64,
    /// Maximum idle duration before a session idle finding is raised.
    pub max_idle_minutes: i64,
    /// Trust score threshold below which trust decay is flagged.
    pub trust_decay_threshold: f64,
    /// Whether to auto-intervene on critical findings.
    pub auto_intervene: bool,
}

impl Default for SupervisorConfig {
    fn default() -> Self {
        Self {
            max_turns_per_minute: 30,
            max_idle_minutes: 60,
            trust_decay_threshold: 0.3,
            auto_intervene: false,
        }
    }
}

/// The agent supervisor.
///
/// Monitors sessions for anomalies and generates findings with
/// recommended actions.
#[derive(Debug, Clone)]
pub struct Supervisor {
    /// Supervisor configuration.
    config: SupervisorConfig,
    /// Recent findings.
    findings: Vec<Finding>,
    /// Turn timestamps per session (for rate detection).
    turn_history: std::collections::HashMap<String, Vec<DateTime<Utc>>>,
}

impl Supervisor {
    /// Create a new supervisor with the given configuration.
    pub fn new(config: SupervisorConfig) -> Self {
        Self {
            config,
            findings: Vec::new(),
            turn_history: std::collections::HashMap::new(),
        }
    }

    /// Create a supervisor with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(SupervisorConfig::default())
    }

    /// Inspect a session and return any findings.
    pub fn inspect(&mut self, session: &Session) -> Vec<Finding> {
        let mut findings = Vec::new();

        // Check for rate anomaly
        if let Some(finding) = self.check_rate(session) {
            findings.push(finding);
        }

        // Check for session idle
        if let Some(finding) = self.check_idle(session) {
            findings.push(finding);
        }

        // Check context drift
        if let Some(finding) = self.check_context_drift(session) {
            findings.push(finding);
        }

        self.findings.extend(findings.clone());
        findings
    }

    /// Inspect a session with trust score context.
    pub fn inspect_with_trust(
        &mut self,
        session: &Session,
        trust_score: TrustScore,
    ) -> Vec<Finding> {
        let mut findings = self.inspect(session);

        // Check trust decay
        if trust_score.value() < self.config.trust_decay_threshold {
            findings.push(Finding {
                id: uuid::Uuid::new_v4().to_string(),
                session_id: session.id.clone(),
                agent_id: session.agent_id.clone(),
                finding_type: FindingType::TrustDecay,
                severity: if trust_score.value() < 0.1 {
                    Severity::Critical
                } else {
                    Severity::Elevated
                },
                description: format!(
                    "Trust score {:.3} below threshold {:.3}",
                    trust_score.value(),
                    self.config.trust_decay_threshold
                ),
                timestamp: Utc::now(),
            });
        }

        self.findings.extend(findings.clone());
        findings
    }

    /// Check for rate anomalies.
    fn check_rate(&mut self, session: &Session) -> Option<Finding> {
        let history = self
            .turn_history
            .entry(session.id.clone())
            .or_default();

        history.push(Utc::now());

        // Keep only the last minute
        let cutoff = Utc::now() - Duration::minutes(1);
        history.retain(|t| *t > cutoff);

        if history.len() as u64 > self.config.max_turns_per_minute {
            let severity = if history.len() as u64 > self.config.max_turns_per_minute * 2 {
                Severity::Critical
            } else if history.len() as u64 > self.config.max_turns_per_minute * 3 / 2 {
                Severity::Elevated
            } else {
                Severity::Warning
            };

            return Some(Finding {
                id: uuid::Uuid::new_v4().to_string(),
                session_id: session.id.clone(),
                agent_id: session.agent_id.clone(),
                finding_type: FindingType::RateAnomaly,
                severity,
                description: format!(
                    "Rate anomaly: {} turns in the last minute (limit: {})",
                    history.len(),
                    self.config.max_turns_per_minute
                ),
                timestamp: Utc::now(),
            });
        }

        None
    }

    /// Check for idle sessions.
    fn check_idle(&self, session: &Session) -> Option<Finding> {
        if !session.active {
            return None;
        }

        let max_idle = Duration::minutes(self.config.max_idle_minutes);
        if session.is_idle_longer_than(max_idle) {
            return Some(Finding {
                id: uuid::Uuid::new_v4().to_string(),
                session_id: session.id.clone(),
                agent_id: session.agent_id.clone(),
                finding_type: FindingType::SessionIdle,
                severity: Severity::Warning,
                description: format!(
                    "Session idle for {} minutes (limit: {})",
                    session.idle_duration().num_minutes(),
                    self.config.max_idle_minutes
                ),
                timestamp: Utc::now(),
            });
        }

        None
    }

    /// Check for context drift (simplified — compares context strings).
    fn check_context_drift(&self, _session: &Session) -> Option<Finding> {
        // Context drift detection requires comparing the session's declared
        // context against the actual behavior. In a full implementation,
        // this would use embeddings or topic modeling. For Phase 4 scope,
        // this is a placeholder for future semantic analysis.
        None
    }

    /// Get the recommended action for a finding.
    pub fn recommend(&self, finding: &Finding) -> Recommendation {
        match (finding.finding_type, finding.severity) {
            (FindingType::RateAnomaly, Severity::Critical) => Recommendation::SuspendSession,
            (FindingType::RateAnomaly, Severity::Elevated) => Recommendation::Notify,
            (FindingType::RateAnomaly, _) => Recommendation::Log,
            (FindingType::TrustDecay, Severity::Critical) => Recommendation::QuarantineAgent,
            (FindingType::TrustDecay, Severity::Elevated) => Recommendation::Notify,
            (FindingType::TrustDecay, _) => Recommendation::Log,
            (FindingType::SessionIdle, _) => Recommendation::Log,
            (FindingType::ContextDrift, _) => Recommendation::Log,
            (FindingType::SuspiciousPattern, Severity::Critical) => Recommendation::SuspendSession,
            (FindingType::SuspiciousPattern, _) => Recommendation::Notify,
            (FindingType::CapabilityEscalation, Severity::Critical) => {
                Recommendation::QuarantineAgent
            }
            (FindingType::CapabilityEscalation, _) => Recommendation::Notify,
        }
    }

    /// Get recent findings.
    pub fn findings(&self) -> &[Finding] {
        &self.findings
    }

    /// Clear all findings.
    pub fn clear_findings(&mut self) {
        self.findings.clear();
        self.turn_history.clear();
    }

    /// Get findings for a specific agent.
    pub fn findings_for_agent(&self, agent_id: &str) -> Vec<&Finding> {
        self.findings
            .iter()
            .filter(|f| f.agent_id == agent_id)
            .collect()
    }

    /// Number of findings.
    pub fn finding_count(&self) -> usize {
        self.findings.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_session() -> Session {
        Session::new("agent-1", "test")
    }

    #[test]
    fn supervisor_with_defaults_creates() {
        let supervisor = Supervisor::with_defaults();
        assert_eq!(supervisor.finding_count(), 0);
    }

    #[test]
    fn inspect_clean_session_no_findings() {
        let mut supervisor = Supervisor::with_defaults();
        let session = test_session();
        let findings = supervisor.inspect(&session);
        assert!(findings.is_empty());
    }

    #[test]
    fn rate_anomaly_detected() {
        let config = SupervisorConfig {
            max_turns_per_minute: 3,
            ..Default::default()
        };
        let mut supervisor = Supervisor::new(config);
        let session = test_session();

        // Simulate rapid turns
        for _ in 0..5 {
            supervisor.inspect(&session);
        }

        // The last call should produce findings
        let findings = supervisor.inspect(&session);
        let rate_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.finding_type == FindingType::RateAnomaly)
            .collect();
        assert!(!rate_findings.is_empty());
        assert!(rate_findings[0].severity >= Severity::Warning);
    }

    #[test]
    fn trust_decay_detected() {
        let mut supervisor = Supervisor::with_defaults();
        let session = test_session();
        let low_trust = TrustScore::from_value(0.15);

        let findings = supervisor.inspect_with_trust(&session, low_trust);
        let trust_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.finding_type == FindingType::TrustDecay)
            .collect();
        assert!(!trust_findings.is_empty());
        assert_eq!(trust_findings[0].severity, Severity::Elevated);

        // Critical low trust
        let very_low_trust = TrustScore::from_value(0.05);
        let findings = supervisor.inspect_with_trust(&session, very_low_trust);
        let trust_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.finding_type == FindingType::TrustDecay)
            .collect();
        assert!(trust_findings.iter().any(|f| f.severity == Severity::Critical));
    }

    #[test]
    fn recommendation_for_critical_trust_decay() {
        let supervisor = Supervisor::with_defaults();
        let finding = Finding {
            id: "test".to_string(),
            session_id: "s1".to_string(),
            agent_id: "a1".to_string(),
            finding_type: FindingType::TrustDecay,
            severity: Severity::Critical,
            description: "Trust below threshold".to_string(),
            timestamp: Utc::now(),
        };
        assert_eq!(
            supervisor.recommend(&finding),
            Recommendation::QuarantineAgent
        );
    }

    #[test]
    fn findings_filtered_by_agent() {
        let mut supervisor = Supervisor::new(SupervisorConfig {
            max_turns_per_minute: 1,
            ..Default::default()
        });
        let session_a = Session::new("agent-a", "test");
        let session_b = Session::new("agent-b", "test");

        // Generate rapid turns for agent-a
        for _ in 0..3 {
            supervisor.inspect(&session_a);
        }

        assert!(!supervisor.findings_for_agent("agent-a").is_empty());
        assert!(supervisor.findings_for_agent("agent-b").is_empty());
    }

    #[test]
    fn severity_ordering() {
        assert!(Severity::Critical > Severity::Elevated);
        assert!(Severity::Elevated > Severity::Warning);
        assert!(Severity::Warning > Severity::Info);
    }
}
