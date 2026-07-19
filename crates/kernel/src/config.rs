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

/// Validation errors for configuration.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ConfigError {
    /// A required field is missing or invalid.
    #[error("invalid configuration: {0}")]
    Invalid(String),

    /// A value is out of acceptable range.
    #[error("value out of range: {field} = {value} (expected {expected})")]
    OutOfRange {
        /// The field name.
        field: String,
        /// The invalid value.
        value: String,
        /// What was expected.
        expected: String,
    },
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
        config.validate()?;
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

    /// Validate the entire configuration.
    ///
    /// Returns `Ok(())` if all values are within acceptable ranges,
    /// or a `ConfigError` describing the first problem found.
    pub fn validate(&self) -> Result<(), ConfigError> {
        self.system.validate()?;
        self.trust.validate()?;
        self.sandbox.validate()?;
        self.audit.validate()?;
        Ok(())
    }

    /// Return a list of all validation warnings (non-fatal issues).
    pub fn warnings(&self) -> Vec<String> {
        let mut warnings = Vec::new();

        if self.trust.initial_score < 0.05 {
            warnings.push(
                "trust.initial_score is very low (< 0.05) — agents will start nearly untrusted"
                    .to_string(),
            );
        }

        if self.trust.decay_factor < 0.99 {
            warnings.push(
                "trust.decay_factor is aggressive (< 0.99) — trust will decay quickly".to_string(),
            );
        }

        if self.sandbox.default_memory_limit < 64 * 1024 * 1024 {
            warnings.push(
                "sandbox.default_memory_limit is below 64 MiB — agents may run out of memory"
                    .to_string(),
            );
        }

        if self.guard.max_violations_per_minute == 0 {
            warnings.push(
                "guard.max_violations is 0 — any violation will immediately quarantine agents"
                    .to_string(),
            );
        }

        warnings
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

impl SystemConfig {
    /// Validate system configuration.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.name.is_empty() {
            return Err(ConfigError::Invalid("system.name must not be empty".to_string()));
        }

        let valid_levels = ["trace", "debug", "info", "warn", "error"];
        if !valid_levels.contains(&self.log_level.as_str()) {
            return Err(ConfigError::Invalid(format!(
                "system.log_level must be one of: {}",
                valid_levels.join(", ")
            )));
        }

        let valid_formats = ["text", "json"];
        if !valid_formats.contains(&self.log_format.as_str()) {
            return Err(ConfigError::Invalid(format!(
                "system.log_format must be one of: {}",
                valid_formats.join(", ")
            )));
        }

        Ok(())
    }
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

impl TrustConfig {
    /// Validate trust configuration.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.initial_score < 0.0 || self.initial_score > 1.0 {
            return Err(ConfigError::OutOfRange {
                field: "trust.initial_score".to_string(),
                value: self.initial_score.to_string(),
                expected: "0.0 to 1.0".to_string(),
            });
        }

        if self.decay_factor < 0.0 || self.decay_factor > 1.0 {
            return Err(ConfigError::OutOfRange {
                field: "trust.decay_factor".to_string(),
                value: self.decay_factor.to_string(),
                expected: "0.0 to 1.0".to_string(),
            });
        }

        if self.suspension_threshold < 0.0 || self.suspension_threshold > 1.0 {
            return Err(ConfigError::OutOfRange {
                field: "trust.suspension_threshold".to_string(),
                value: self.suspension_threshold.to_string(),
                expected: "0.0 to 1.0".to_string(),
            });
        }

        if self.attestation_ttl_hours < 1 || self.attestation_ttl_hours > 720 {
            return Err(ConfigError::OutOfRange {
                field: "trust.attestation_ttl_hours".to_string(),
                value: self.attestation_ttl_hours.to_string(),
                expected: "1 to 720 (30 days)".to_string(),
            });
        }

        Ok(())
    }
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

impl SandboxConfig {
    /// Validate sandbox configuration.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.workspace_path.is_empty() {
            return Err(ConfigError::Invalid(
                "sandbox.workspace_path must not be empty".to_string(),
            ));
        }

        if self.default_memory_limit < 1024 * 1024 {
            return Err(ConfigError::OutOfRange {
                field: "sandbox.default_memory_limit".to_string(),
                value: self.default_memory_limit.to_string(),
                expected: ">= 1 MiB (1048576 bytes)".to_string(),
            });
        }

        if self.total_memory_limit < self.default_memory_limit {
            return Err(ConfigError::Invalid(
                "sandbox.total_memory_limit must be >= sandbox.default_memory_limit".to_string(),
            ));
        }

        Ok(())
    }
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

impl AuditConfig {
    /// Validate audit configuration.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.genesis_seed.is_empty() {
            return Err(ConfigError::Invalid(
                "audit.genesis_seed must not be empty".to_string(),
            ));
        }

        if self.max_entries < 100 {
            return Err(ConfigError::OutOfRange {
                field: "audit.max_entries".to_string(),
                value: self.max_entries.to_string(),
                expected: ">= 100".to_string(),
            });
        }

        Ok(())
    }
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

    #[test]
    fn test_validate_default_config() {
        let config = AegisConfig::default_config();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_invalid_trust_score() {
        let mut config = AegisConfig::default_config();
        config.trust.initial_score = 2.0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_invalid_log_level() {
        let mut config = AegisConfig::default_config();
        config.system.log_level = "verbose".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_memory_too_small() {
        let mut config = AegisConfig::default_config();
        config.sandbox.default_memory_limit = 100; // Way too small
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_warnings_on_low_trust() {
        let mut config = AegisConfig::default_config();
        config.trust.initial_score = 0.01;
        let warnings = config.warnings();
        assert!(warnings.iter().any(|w| w.contains("initial_score")));
    }

    #[test]
    fn test_warnings_on_aggressive_decay() {
        let mut config = AegisConfig::default_config();
        config.trust.decay_factor = 0.5;
        let warnings = config.warnings();
        assert!(warnings.iter().any(|w| w.contains("decay_factor")));
    }
}
