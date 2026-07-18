//! # Configuration — System-Wide Settings
//!
//! Configuration for the Ferris Aegis system, loaded from TOML files
//! or set programmatically.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Top-level configuration for Ferris Aegis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AegisConfig {
    /// General system settings
    pub system: SystemConfig,
    /// Trust kernel configuration
    pub trust: TrustConfig,
    /// Guard configuration
    pub guard: crate::guard::GuardConfig,
    /// Sandbox defaults
    pub sandbox: SandboxConfig,
    /// Audit ledger settings
    pub audit: AuditConfig,
}

impl AegisConfig {
    /// Load configuration from a TOML file
    pub fn from_file(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Self::from_toml(&content)
    }

    /// Parse configuration from a TOML string
    pub fn from_toml(content: &str) -> anyhow::Result<Self> {
        let config: AegisConfig = toml::from_str(content)?;
        Ok(config)
    }

    /// Get the default configuration
    pub fn default_config() -> Self {
        Self {
            system: SystemConfig::default(),
            trust: TrustConfig::default(),
            guard: crate::guard::GuardConfig::default(),
            sandbox: SandboxConfig::default(),
            audit: AuditConfig::default(),
        }
    }
}

/// General system configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    /// Name of this Aegis instance
    pub name: String,
    /// Data directory for persistent storage
    pub data_dir: String,
    /// Log level (trace, debug, info, warn, error)
    pub log_level: String,
    /// Log format (text, json)
    pub log_format: String,
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            name: "ferris-aegis".to_string(),
            data_dir: "/var/lib/aegis".to_string(),
            log_level: "info".to_string(),
            log_format: "text".to_string(),
        }
    }
}

/// Trust kernel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustConfig {
    /// Initial trust score for new agents
    pub initial_score: f64,
    /// Trust decay factor per interval
    pub decay_factor: f64,
    /// Suspension threshold
    pub suspension_threshold: f64,
    /// Attestation TTL in hours
    pub attestation_ttl_hours: i64,
}

impl Default for TrustConfig {
    fn default() -> Self {
        Self {
            initial_score: 0.1,
            decay_factor: 0.999,
            suspension_threshold: 0.05,
            attestation_ttl_hours: 24,
        }
    }
}

/// Sandbox configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    /// Default workspace path
    pub workspace_path: String,
    /// Maximum total memory across all sandboxes (bytes)
    pub total_memory_limit: u64,
    /// Default max memory per sandbox (bytes)
    pub default_memory_limit: u64,
    /// Enable network isolation
    pub network_isolation: bool,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            workspace_path: "/workspace".to_string(),
            total_memory_limit: 8 * 1024 * 1024 * 1024, // 8 GB
            default_memory_limit: 512 * 1024 * 1024,     // 512 MB
            network_isolation: true,
        }
    }
}

/// Audit ledger configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditConfig {
    /// Path to store the audit ledger
    pub ledger_path: String,
    /// Whether to persist the ledger to disk
    pub persist: bool,
    /// Maximum ledger size in entries before rotation
    pub max_entries: u64,
    /// Genesis hash seed
    pub genesis_seed: String,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            ledger_path: "/var/lib/aegis/ledger".to_string(),
            persist: true,
            max_entries: 1_000_000,
            genesis_seed: "ferris-aegis-genesis".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AegisConfig::default_config();
        assert_eq!(config.system.name, "ferris-aegis");
        assert!((config.trust.initial_score - 0.1).abs() < f64::EPSILON);
    }

    #[test]
    fn test_config_serialization_roundtrip() {
        let config = AegisConfig::default_config();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let deserialized: AegisConfig = toml::from_str(&toml_str).unwrap();

        assert_eq!(config.system.name, deserialized.system.name);
        assert!((config.trust.decay_factor - deserialized.trust.decay_factor).abs() < f64::EPSILON);
    }
}
