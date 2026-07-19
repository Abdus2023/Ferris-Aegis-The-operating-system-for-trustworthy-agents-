//! # System Health — Component Health Reporting
//!
//! Provides health reporting for all kernel subsystems. Each component
//! can report its status, and the system can query aggregate health.
//! This is used by the resilience crate's `HealthCheck` trait and by
//! the CLI `aegis health` command.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The health status of a component.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum HealthStatus {
    /// Component is healthy.
    Healthy,
    /// Component is degraded — operating but with reduced capacity.
    Degraded,
    /// Component is unhealthy — not functioning.
    Unhealthy,
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthStatus::Healthy => write!(f, "healthy"),
            HealthStatus::Degraded => write!(f, "degraded"),
            HealthStatus::Unhealthy => write!(f, "unhealthy"),
        }
    }
}

/// Health report for a single component.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    /// The component name (e.g. "trust_kernel", "policy_engine").
    pub component: String,
    /// The health status.
    pub status: HealthStatus,
    /// Optional diagnostic message.
    pub message: Option<String>,
    /// When the check was performed.
    pub checked_at: DateTime<Utc>,
    /// Duration of the check in milliseconds.
    pub duration_ms: u64,
    /// Additional component-specific metrics.
    pub metrics: serde_json::Value,
}

impl ComponentHealth {
    /// Create a healthy component report.
    pub fn healthy(component: &str) -> Self {
        Self {
            component: component.to_string(),
            status: HealthStatus::Healthy,
            message: None,
            checked_at: Utc::now(),
            duration_ms: 0,
            metrics: serde_json::Value::Object(serde_json::Map::new()),
        }
    }

    /// Create a degraded component report.
    pub fn degraded(component: &str, message: &str) -> Self {
        Self {
            component: component.to_string(),
            status: HealthStatus::Degraded,
            message: Some(message.to_string()),
            checked_at: Utc::now(),
            duration_ms: 0,
            metrics: serde_json::Value::Object(serde_json::Map::new()),
        }
    }

    /// Create an unhealthy component report.
    pub fn unhealthy(component: &str, message: &str) -> Self {
        Self {
            component: component.to_string(),
            status: HealthStatus::Unhealthy,
            message: Some(message.to_string()),
            checked_at: Utc::now(),
            duration_ms: 0,
            metrics: serde_json::Value::Object(serde_json::Map::new()),
        }
    }

    /// Add metrics to the health report.
    pub fn with_metrics(mut self, metrics: serde_json::Value) -> Self {
        self.metrics = metrics;
        self
    }
}

/// Aggregate health report for the entire system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    /// Aggregate system health status.
    pub system_status: HealthStatus,
    /// Per-component health reports.
    pub components: Vec<ComponentHealth>,
    /// Total number of components.
    pub total_components: usize,
    /// Number of healthy components.
    pub healthy_count: usize,
    /// Number of degraded components.
    pub degraded_count: usize,
    /// Number of unhealthy components.
    pub unhealthy_count: usize,
    /// When the report was generated.
    pub generated_at: DateTime<Utc>,
}

impl HealthReport {
    /// Create a health report from component reports.
    pub fn from_components(components: Vec<ComponentHealth>) -> Self {
        let healthy_count = components
            .iter()
            .filter(|c| c.status == HealthStatus::Healthy)
            .count();
        let degraded_count = components
            .iter()
            .filter(|c| c.status == HealthStatus::Degraded)
            .count();
        let unhealthy_count = components
            .iter()
            .filter(|c| c.status == HealthStatus::Unhealthy)
            .count();

        let system_status = if unhealthy_count > 0 {
            HealthStatus::Unhealthy
        } else if degraded_count > 0 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };

        Self {
            system_status,
            components,
            total_components: healthy_count + degraded_count + unhealthy_count,
            healthy_count,
            degraded_count,
            unhealthy_count,
            generated_at: Utc::now(),
        }
    }

    /// Whether the system is fully healthy.
    pub fn is_healthy(&self) -> bool {
        self.system_status == HealthStatus::Healthy
    }

    /// Whether the system is degraded (operating with reduced capacity).
    pub fn is_degraded(&self) -> bool {
        self.system_status == HealthStatus::Degraded
    }

    /// Whether the system is unhealthy.
    pub fn is_unhealthy(&self) -> bool {
        self.system_status == HealthStatus::Unhealthy
    }
}

/// System health checker that aggregates all kernel components.
pub struct SystemHealth {
    /// Trust kernel operational status.
    pub trust_kernel_ok: bool,
    /// Policy engine operational status.
    pub policy_engine_ok: bool,
    /// Agent runtime operational status.
    pub agent_runtime_ok: bool,
    /// Sandbox operational status.
    pub sandbox_ok: bool,
    /// Guard operational status.
    pub guard_ok: bool,
    /// Audit ledger integrity.
    pub audit_ledger_ok: bool,
    /// Agent count (for metrics).
    pub agent_count: usize,
    /// Active agent count.
    pub active_agent_count: usize,
    /// Policy count.
    pub policy_count: usize,
}

impl SystemHealth {
    /// Create a new system health snapshot.
    pub fn new() -> Self {
        Self {
            trust_kernel_ok: true,
            policy_engine_ok: true,
            agent_runtime_ok: true,
            sandbox_ok: true,
            guard_ok: true,
            audit_ledger_ok: true,
            agent_count: 0,
            active_agent_count: 0,
            policy_count: 0,
        }
    }

    /// Generate a full health report.
    pub fn report(&self) -> HealthReport {
        let components = vec![
            self.component_health(
                "trust_kernel",
                self.trust_kernel_ok,
                "Trust scoring and attestation",
            ),
            self.component_health(
                "policy_engine",
                self.policy_engine_ok,
                "Policy evaluation and enforcement",
            ),
            self.component_health(
                "agent_runtime",
                self.agent_runtime_ok,
                &format!(
                    "{} agents ({} active)",
                    self.agent_count, self.active_agent_count
                ),
            ),
            self.component_health(
                "sandbox",
                self.sandbox_ok,
                "Capability-based isolation",
            ),
            self.component_health(
                "guard",
                self.guard_ok,
                "Real-time monitoring and intervention",
            ),
            self.component_health(
                "audit_ledger",
                self.audit_ledger_ok,
                "Cryptographic audit chain",
            ),
        ];

        HealthReport::from_components(components)
    }

    /// Create a component health report.
    fn component_health(&self, name: &str, ok: bool, detail: &str) -> ComponentHealth {
        if ok {
            ComponentHealth::healthy(name).with_metrics(
                serde_json::json!({"detail": detail}),
            )
        } else {
            ComponentHealth::unhealthy(name, &format!("{name} is not operational - {detail}"))
        }
    }
}

impl Default for SystemHealth {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_healthy_system() {
        let health = SystemHealth::new();
        let report = health.report();
        assert!(report.is_healthy());
        assert_eq!(report.healthy_count, 6);
        assert_eq!(report.unhealthy_count, 0);
    }

    #[test]
    fn degraded_system() {
        let mut health = SystemHealth::new();
        health.guard_ok = false;
        let report = health.report();
        assert!(report.is_unhealthy());
        assert_eq!(report.unhealthy_count, 1);
    }

    #[test]
    fn health_report_counts() {
        let reports = vec![
            ComponentHealth::healthy("a"),
            ComponentHealth::healthy("b"),
            ComponentHealth::degraded("c", "slow"),
            ComponentHealth::unhealthy("d", "down"),
        ];
        let report = HealthReport::from_components(reports);
        assert_eq!(report.total_components, 4);
        assert_eq!(report.healthy_count, 2);
        assert_eq!(report.degraded_count, 1);
        assert_eq!(report.unhealthy_count, 1);
        assert_eq!(report.system_status, HealthStatus::Unhealthy);
    }

    #[test]
    fn degraded_only_system() {
        let reports = vec![
            ComponentHealth::healthy("a"),
            ComponentHealth::degraded("b", "slow"),
        ];
        let report = HealthReport::from_components(reports);
        assert_eq!(report.system_status, HealthStatus::Degraded);
        assert!(report.is_degraded());
    }
}
