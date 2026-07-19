use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Unique identifier for a skill: `skill:<category>:<name>`
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SkillId(pub String);

impl SkillId {
    /// Parse and validate a skill ID.
    /// Format: `skill:<category>:<name>`
    pub fn new(category: &str, name: &str) -> Self {
        SkillId(format!("skill:{}:{}", category, name))
    }

    pub fn category(&self) -> Option<&str> {
        self.0.split(':').nth(1)
    }

    pub fn name(&self) -> Option<&str> {
        self.0.split(':').nth(2)
    }
}

impl std::fmt::Display for SkillId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Trust level required to execute a skill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrustLevelRequired {
    Unverified,    // 0.00–0.19
    Probationary,  // 0.20–0.49
    Standard,      // 0.50–0.74
    Elevated,      // 0.75–0.94
    Sovereign,     // 0.95–1.00
}

impl TrustLevelRequired {
    pub fn as_f64(&self) -> f64 {
        match self {
            TrustLevelRequired::Unverified => 0.0,
            TrustLevelRequired::Probationary => 0.2,
            TrustLevelRequired::Standard => 0.5,
            TrustLevelRequired::Elevated => 0.75,
            TrustLevelRequired::Sovereign => 0.95,
        }
    }
}

impl std::fmt::Display for TrustLevelRequired {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                TrustLevelRequired::Unverified => "unverified",
                TrustLevelRequired::Probationary => "probationary",
                TrustLevelRequired::Standard => "standard",
                TrustLevelRequired::Elevated => "elevated",
                TrustLevelRequired::Sovereign => "sovereign",
            }
        )
    }
}

/// Capability in the format: `<domain>:<operation>[:<scope>]`
/// Examples: `filesystem:read`, `network:connect`, `validation:injection-scan`
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Capability(pub String);

impl Capability {
    /// Check if this capability matches a pattern (supports wildcards).
    /// Examples:
    /// - `filesystem:read` matches `filesystem:*`
    /// - `filesystem:read` matches `filesystem:read`
    /// - `filesystem:read` does NOT match `network:*`
    pub fn matches(&self, pattern: &str) -> bool {
        if pattern.contains('*') {
            let parts: Vec<&str> = pattern.split(':').collect();
            let self_parts: Vec<&str> = self.0.split(':').collect();

            for (i, part) in parts.iter().enumerate() {
                if i >= self_parts.len() {
                    return false;
                }
                if *part != "*" && *part != self_parts[i] {
                    return false;
                }
            }
            true
        } else {
            self.0 == pattern
        }
    }
}

impl std::fmt::Display for Capability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Resource limits for skill execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub max_file_size: String,          // e.g., "100MB"
    pub max_execution_time: String,     // e.g., "30s"
    pub max_memory: String,             // e.g., "256MB"
    pub max_concurrent_calls: usize,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_file_size: "100MB".to_string(),
            max_execution_time: "30s".to_string(),
            max_memory: "256MB".to_string(),
            max_concurrent_calls: 5,
        }
    }
}

/// Declarative policy rule for skill execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRule {
    pub id: String,
    pub rule: String,
    pub effect: PolicyEffect,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PolicyEffect {
    Allow,
    Deny,
    Alert,
}

/// Skill dependency: another skill, system tool, or crate.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Dependency {
    Skill {
        skill: String,
        version: String,
        #[serde(default)]
        optional: bool,
        #[serde(default)]
        fallback: bool,
    },
    SystemTool {
        #[serde(rename = "system")]
        tools: HashMap<String, String>,
    },
    Crate {
        #[serde(rename = "crate")]
        name: String,
        version: String,
    },
}

/// Trigger condition for automatic skill activation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trigger {
    pub event: String,
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default)]
    pub pattern: Option<String>,
    #[serde(default)]
    pub weight: u32,
}

/// Signature information for skill attestation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signature {
    pub algorithm: String,
    pub public_key: String,
    pub signed_at: DateTime<Utc>,
    #[serde(default)]
    pub signature: String,
}

/// Agent compatibility metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCompatibility {
    pub name: String,
    pub min_version: String,
    #[serde(default)]
    pub features: Vec<String>,
}

/// Complete SKILL.md representation after parsing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    // Metadata
    pub skill_version: String,
    pub skill_id: SkillId,
    pub name: String,
    pub category: String,
    pub description: String,
    pub version: String,
    pub author: String,
    pub license: String,

    // Discovery
    pub tags: Vec<String>,
    pub keywords: Vec<String>,
    pub maintainer: Option<String>,

    // Capabilities
    pub capabilities_required: Vec<Capability>,
    pub trust_level_minimum: TrustLevelRequired,
    pub sandbox_boundary: String,

    // Dependencies
    #[serde(default)]
    pub dependencies: Vec<Dependency>,

    // Triggers
    #[serde(default)]
    pub triggers: Vec<Trigger>,

    // Resource Limits
    pub resource_limits: ResourceLimits,

    // Policies
    #[serde(default)]
    pub policies: Vec<PolicyRule>,

    // Execution
    pub execution_protocol: String,
    pub protocol_version: String,
    pub export_format: String,

    // Compatibility
    #[serde(default)]
    pub compatible_agents: Vec<AgentCompatibility>,

    // Attestation
    #[serde(default)]
    pub signature: Option<Signature>,

    // The actual markdown content (body after frontmatter)
    #[serde(skip)]
    pub content: String,
}

/// Execution context for a skill.
#[derive(Debug, Clone)]
pub struct SkillExecutionContext {
    pub execution_id: Uuid,
    pub agent_id: String,
    pub agent_trust_score: f64,
    pub session_id: Uuid,

    pub capabilities: HashSet<Capability>,
    pub sandbox_boundary: String,

    pub workspace_root: std::path::PathBuf,
    pub temp_dir: std::path::PathBuf,

    pub start_time: DateTime<Utc>,
    pub deadline: Option<DateTime<Utc>>,
}

/// Result of skill execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillExecutionResult {
    pub execution_id: Uuid,
    pub skill_id: SkillId,
    pub status: ExecutionStatus,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub trace_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionStatus {
    Success,
    Failed,
    Denied,
    TimedOut,
}

/// Skill metadata for discovery and caching.
#[derive(Debug, Clone)]
pub struct SkillMetadata {
    pub skill_id: SkillId,
    pub version: String,
    pub category: String,
    pub trust_level_minimum: TrustLevelRequired,
    pub capabilities_required: Vec<Capability>,
    pub last_loaded: DateTime<Utc>,
}
