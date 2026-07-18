//! # Guard — Real-Time Monitoring and Intervention
//!
//! The Guard is the watchful eye of Ferris Aegis. It continuously monitors
//! agent behavior, detects anomalies, and can intervene when agents deviate
//! from expected patterns. The Guard can:
//!
//! - **Alert** — Issue warnings when behavior is suspicious
//! - **Throttle** — Slow down agents that are acting too aggressively
//! - **Quarantine** — Isolate agents that pose a threat
//! - **Terminate** — Kill agents that are causing harm
//!
//! ## Anomaly Detection
//!
//! The Guard uses simple statistical thresholds to detect anomalies:
//! - Action rate exceeding expected bounds
//! - Unusual access patterns
//! - Trust score deterioration
//! - Policy violation frequency

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::agent::AgentId;
use crate::audit::AuditSeverity;
use crate::kernel::TrustKernel;

/// The type of action the Guard can take
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum GuardAction {
    /// Just log a warning, no intervention
    Alert,
    /// Slow down the agent's execution
    Throttle,
    /// Isolate the agent (strip capabilities, suspend)
    Quarantine,
    /// Terminate the agent immediately
    Terminate,
}

impl GuardAction {
    /// Get the severity of this action
    pub fn severity(&self) -> AuditSeverity {
        match self {
            GuardAction::Alert => AuditSeverity::Warning,
            GuardAction::Throttle => AuditSeverity::Warning,
            GuardAction::Quarantine => AuditSeverity::Critical,
            GuardAction::Terminate => AuditSeverity::Emergency,
        }
    }

    /// Whether this action is terminal (agent cannot continue)
    pub fn is_terminal(&self) -> bool {
        matches!(self, GuardAction::Terminate)
    }
}

impl std::fmt::Display for GuardAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GuardAction::Alert => write!(f, "alert"),
            GuardAction::Throttle => write!(f, "throttle"),
            GuardAction::Quarantine => write!(f, "quarantine"),
            GuardAction::Terminate => write!(f, "terminate"),
        }
    }
}

/// An alert raised by the Guard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardAlert {
    /// Unique alert identifier
    pub id: String,
    /// The agent that triggered the alert
    pub agent_id: AgentId,
    /// The rule that triggered
    pub rule: GuardRule,
    /// The action taken
    pub action: GuardAction,
    /// When the alert was raised
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Human-readable description
    pub description: String,
    /// The observed value that triggered the alert
    pub observed_value: f64,
    /// The threshold that was exceeded
    pub threshold: f64,
}

/// A rule monitored by the Guard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GuardRule {
    /// Agent is performing actions too quickly
    ActionRateExceeded {
        /// Maximum actions per minute
        max_per_minute: u32,
    },
    /// Agent's trust score has dropped below a threshold
    TrustScoreDegraded {
        /// Minimum acceptable trust score
        min_score: f64,
    },
    /// Too many policy violations in a time window
    PolicyViolationSpike {
        /// Maximum violations per minute
        max_per_minute: u32,
    },
    /// Agent is consuming too many resources
    ResourceUsageExceeded {
        /// Resource type
        resource: String,
        /// Maximum allowed usage
        max_usage: f64,
    },
    /// Agent has been idle for too long (possible deadlock)
    IdleTooLong {
        /// Maximum idle seconds
        max_idle_seconds: u64,
    },
    /// Custom rule with a name
    Custom {
        name: String,
        description: String,
    },
}

impl GuardRule {
    /// Get a human-readable name for this rule
    pub fn name(&self) -> &str {
        match self {
            GuardRule::ActionRateExceeded { .. } => "action_rate_exceeded",
            GuardRule::TrustScoreDegraded { .. } => "trust_score_degraded",
            GuardRule::PolicyViolationSpike { .. } => "policy_violation_spike",
            GuardRule::ResourceUsageExceeded { .. } => "resource_usage_exceeded",
            GuardRule::IdleTooLong { .. } => "idle_too_long",
            GuardRule::Custom { name, .. } => name,
        }
    }
}

/// Configuration for the Guard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardConfig {
    /// Maximum actions per minute before alerting
    pub max_actions_per_minute: u32,
    /// Maximum actions per minute before throttling
    pub throttle_threshold: u32,
    /// Maximum actions per minute before quarantine
    pub quarantine_threshold: u32,
    /// Minimum trust score before alerting
    pub min_trust_score: f64,
    /// Maximum policy violations per minute
    pub max_violations_per_minute: u32,
    /// Maximum idle time in seconds
    pub max_idle_seconds: u64,
}

impl Default for GuardConfig {
    fn default() -> Self {
        Self {
            max_actions_per_minute: 500,
            throttle_threshold: 750,
            quarantine_threshold: 1000,
            min_trust_score: 0.1,
            max_violations_per_minute: 10,
            max_idle_seconds: 3600,
        }
    }
}

/// Per-agent monitoring state
#[derive(Debug, Clone)]
struct AgentMonitor {
    /// Actions in the current minute
    actions_this_minute: u32,
    /// Policy violations in the current minute
    violations_this_minute: u32,
    /// Last action timestamp
    last_action: chrono::DateTime<chrono::Utc>,
    /// Current throttle factor (1.0 = normal)
    throttle_factor: f64,
    /// Number of alerts raised for this agent
    alert_count: u32,
}

impl AgentMonitor {
    fn new() -> Self {
        Self {
            actions_this_minute: 0,
            violations_this_minute: 0,
            last_action: chrono::Utc::now(),
            throttle_factor: 1.0,
            alert_count: 0,
        }
    }
}

/// The Guard — real-time monitoring and intervention system
#[derive(Debug)]
pub struct Guard {
    /// Guard configuration
    config: GuardConfig,
    /// Per-agent monitoring state
    monitors: HashMap<AgentId, AgentMonitor>,
    /// Active alerts
    alerts: Vec<GuardAlert>,
    /// Whether the Guard is enabled
    enabled: bool,
}

impl Guard {
    /// Create a new Guard with default configuration
    pub fn new() -> Self {
        Self {
            config: GuardConfig::default(),
            monitors: HashMap::new(),
            alerts: Vec::new(),
            enabled: true,
        }
    }

    /// Create a Guard with custom configuration
    pub fn with_config(config: GuardConfig) -> Self {
        Self {
            config,
            monitors: HashMap::new(),
            alerts: Vec::new(),
            enabled: true,
        }
    }

    /// Enable the Guard
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Disable the Guard (for testing only!)
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Register an agent for monitoring
    pub fn register_agent(&mut self, agent_id: &AgentId) {
        self.monitors.insert(agent_id.clone(), AgentMonitor::new());
    }

    /// Unregister an agent from monitoring
    pub fn unregister_agent(&mut self, agent_id: &AgentId) {
        self.monitors.remove(agent_id);
    }

    /// Record that an agent performed an action
    pub fn record_action(&mut self, agent_id: &AgentId) -> Option<GuardAction> {
        if !self.enabled {
            return None;
        }

        let monitor = self.monitors.get_mut(agent_id)?;
        monitor.actions_this_minute += 1;
        monitor.last_action = chrono::Utc::now();

        // Check action rate thresholds
        if monitor.actions_this_minute >= self.config.quarantine_threshold {
            return Some(GuardAction::Quarantine);
        }
        if monitor.actions_this_minute >= self.config.throttle_threshold {
            return Some(GuardAction::Throttle);
        }
        if monitor.actions_this_minute >= self.config.max_actions_per_minute {
            return Some(GuardAction::Alert);
        }

        None
    }

    /// Record that an agent violated a policy
    pub fn record_violation(&mut self, agent_id: &AgentId) -> Option<GuardAction> {
        if !self.enabled {
            return None;
        }

        let monitor = self.monitors.get_mut(agent_id)?;
        monitor.violations_this_minute += 1;

        if monitor.violations_this_minute >= self.config.max_violations_per_minute {
            return Some(GuardAction::Quarantine);
        }
        None
    }

    /// Check an agent's trust score against thresholds
    pub fn check_trust(&mut self, agent_id: &AgentId, trust_kernel: &TrustKernel) -> Option<GuardAction> {
        if !self.enabled {
            return None;
        }

        let record = trust_kernel.get_record(agent_id)?;
        if record.score.value() < self.config.min_trust_score {
            return Some(GuardAction::Quarantine);
        }
        None
    }

    /// Get the current throttle factor for an agent
    pub fn throttle_factor(&self, agent_id: &AgentId) -> f64 {
        self.monitors
            .get(agent_id)
            .map(|m| m.throttle_factor)
            .unwrap_or(1.0)
    }

    /// Set the throttle factor for an agent
    pub fn set_throttle_factor(&mut self, agent_id: &AgentId, factor: f64) {
        if let Some(monitor) = self.monitors.get_mut(agent_id) {
            monitor.throttle_factor = factor.clamp(0.01, 1.0);
        }
    }

    /// Reset per-minute counters (call once per minute)
    pub fn reset_counters(&mut self) {
        for monitor in self.monitors.values_mut() {
            monitor.actions_this_minute = 0;
            monitor.violations_this_minute = 0;
        }
    }

    /// Get all active alerts
    pub fn alerts(&self) -> &[GuardAlert] {
        &self.alerts
    }

    /// Clear all alerts
    pub fn clear_alerts(&mut self) {
        self.alerts.clear();
    }

    /// Get the number of monitored agents
    pub fn monitored_count(&self) -> usize {
        self.monitors.len()
    }

    /// Check if the guard is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl Default for Guard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_agent_id() -> AgentId {
        AgentId::new("test-agent")
    }

    #[test]
    fn test_guard_action_rate_alert() {
        let mut guard = Guard::with_config(GuardConfig {
            max_actions_per_minute: 5,
            throttle_threshold: 8,
            quarantine_threshold: 10,
            ..Default::default()
        });
        let agent_id = test_agent_id();
        guard.register_agent(&agent_id);

        // First 4 actions should be fine
        for _ in 0..4 {
            let action = guard.record_action(&agent_id);
            assert!(action.is_none());
        }

        // 5th action should trigger alert
        let action = guard.record_action(&agent_id);
        assert_eq!(action, Some(GuardAction::Alert));
    }

    #[test]
    fn test_guard_action_rate_throttle() {
        let mut guard = Guard::with_config(GuardConfig {
            max_actions_per_minute: 5,
            throttle_threshold: 8,
            quarantine_threshold: 10,
            ..Default::default()
        });
        let agent_id = test_agent_id();
        guard.register_agent(&agent_id);

        for _ in 0..8 {
            guard.record_action(&agent_id);
        }

        let action = guard.record_action(&agent_id);
        assert_eq!(action, Some(GuardAction::Throttle));
    }

    #[test]
    fn test_guard_action_rate_quarantine() {
        let mut guard = Guard::with_config(GuardConfig {
            max_actions_per_minute: 5,
            throttle_threshold: 8,
            quarantine_threshold: 10,
            ..Default::default()
        });
        let agent_id = test_agent_id();
        guard.register_agent(&agent_id);

        for _ in 0..10 {
            guard.record_action(&agent_id);
        }

        let action = guard.record_action(&agent_id);
        assert_eq!(action, Some(GuardAction::Quarantine));
    }

    #[test]
    fn test_guard_violation_tracking() {
        let mut guard = Guard::with_config(GuardConfig {
            max_violations_per_minute: 3,
            ..Default::default()
        });
        let agent_id = test_agent_id();
        guard.register_agent(&agent_id);

        for _ in 0..3 {
            guard.record_violation(&agent_id);
        }

        let action = guard.record_violation(&agent_id);
        assert_eq!(action, Some(GuardAction::Quarantine));
    }

    #[test]
    fn test_guard_disable() {
        let mut guard = Guard::new();
        guard.disable();

        let agent_id = test_agent_id();
        guard.register_agent(&agent_id);

        // Even with many actions, disabled guard should not intervene
        for _ in 0..1000 {
            assert!(guard.record_action(&agent_id).is_none());
        }
    }

    #[test]
    fn test_guard_reset_counters() {
        let mut guard = Guard::with_config(GuardConfig {
            max_actions_per_minute: 3,
            throttle_threshold: 5,
            quarantine_threshold: 7,
            ..Default::default()
        });
        let agent_id = test_agent_id();
        guard.register_agent(&agent_id);

        for _ in 0..3 {
            guard.record_action(&agent_id);
        }

        guard.reset_counters();

        // After reset, should not alert
        let action = guard.record_action(&agent_id);
        assert!(action.is_none());
    }

    #[test]
    fn test_guard_trust_check() {
        let mut guard = Guard::with_config(GuardConfig {
            min_trust_score: 0.1,
            ..Default::default()
        });
        let mut trust_kernel = TrustKernel::new();
        let agent_id = test_agent_id();
        trust_kernel.register(&agent_id);
        guard.register_agent(&agent_id);

        // Initial score is 0.1, which is right at the threshold
        let action = guard.check_trust(&agent_id, &trust_kernel);
        assert!(action.is_none());

        // Penalize below threshold
        trust_kernel.penalize(&agent_id, 0.1);
        let action = guard.check_trust(&agent_id, &trust_kernel);
        assert_eq!(action, Some(GuardAction::Quarantine));
    }

    #[test]
    fn test_throttle_factor() {
        let mut guard = Guard::new();
        let agent_id = test_agent_id();
        guard.register_agent(&agent_id);

        assert!((guard.throttle_factor(&agent_id) - 1.0).abs() < f64::EPSILON);

        guard.set_throttle_factor(&agent_id, 0.5);
        assert!((guard.throttle_factor(&agent_id) - 0.5).abs() < f64::EPSILON);
    }
}
