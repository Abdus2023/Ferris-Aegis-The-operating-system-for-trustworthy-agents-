//! Ferris Aegis Skills — Agent Skill discovery, parsing, validation, and loading.
//!
//! This crate provides programmatic access to the SKILL.md ecosystem for
//! Ferris Aegis. It implements both:
//!
//! - **agentskills.io v0.2.0** — Legacy frontmatter format (name + description)
//! - **SKILL.md v1.0.0** — Vendor-neutral spec with 10-layer architecture
//!
//! The v1.0.0 format is a strict superset of v0.2.0. Existing skills continue
//! to work; new skills can opt into the richer format by setting
//! `spec_version: "1.0.0"` in their frontmatter.
//!
//! # Progressive Disclosure
//!
//! The crate follows the 3-tier progressive disclosure model:
//!
//! 1. **Metadata** (~100 tokens): `SkillMetadata` — name + description, loaded at startup
//! 2. **Instructions** (<5,000 tokens): `SkillInstructions` — full SKILL.md body, loaded on activation
//! 3. **Resources** (on demand): scripts/, references/, assets/ — loaded lazily
//!
//! # Example
//!
//! ```rust,ignore
//! use ferris_aegis_skills::{SkillRegistry, SkillRegistryConfig};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let config = SkillRegistryConfig::default();
//!     let mut registry = SkillRegistry::new(config);
//!
//!     // Discover skills in .agents/skills/
//!     let discovered = registry.discover(".agents/skills").await?;
//!
//!     // Validate all discovered skills
//!     let results = registry.validate_all();
//!     for result in &results {
//!         if result.is_valid() {
//!             println!("✓ {}", result.skill_name);
//!         } else {
//!             println!("✗ {} — {:?}", result.skill_name, result.errors);
//!         }
//!     }
//!
//!     // Load a specific skill's instructions
//!     let skill = registry.load_skill("aegis-trust-kernel").await?;
//!     println!("Name: {}", skill.metadata.name);
//!     println!("Description: {}", skill.metadata.description);
//!     Ok(())
//! }
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// ═══════════════════════════════════════════════════════════════════
//  V0.2.0 Frontmatter Types (agentskills.io)
// ═══════════════════════════════════════════════════════════════════

/// YAML frontmatter of a SKILL.md file (agentskills.io v0.2.0).
///
/// Only `name` and `description` are required; all other fields are optional.
/// This struct is kept for backward compatibility with existing skills.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillFrontmatter {
    /// Unique skill identifier. 1-64 chars, lowercase alphanumeric + hyphens.
    /// Must match the parent directory name.
    pub name: String,

    /// When this skill should activate. Max 1024 chars.
    /// Should include "Use when..." trigger phrases.
    pub description: String,

    /// SPDX license identifier (e.g., "MIT OR Apache-2.0").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,

    /// Environment requirements. Max 500 chars.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compatibility: Option<String>,

    /// Arbitrary key-value string map for custom properties.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,

    /// Space-delimited list of pre-approved tools.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "allowed-tools")]
    pub allowed_tools: Option<String>,
}

impl SkillFrontmatter {
    /// Get the `aegis-crate` extension field from metadata.
    pub fn aegis_crate(&self) -> Option<&str> {
        self.metadata.as_ref()?.get("aegis-crate").map(|s| s.as_str())
    }

    /// Get the `aegis-phase` extension field from metadata.
    pub fn aegis_phase(&self) -> Option<&str> {
        self.metadata.as_ref()?.get("aegis-phase").map(|s| s.as_str())
    }

    /// Get the `aegis-depends` extension field from metadata.
    pub fn aegis_depends(&self) -> Option<&str> {
        self.metadata.as_ref()?.get("aegis-depends").map(|s| s.as_str())
    }

    /// Get the `aegis-invariants` extension field from metadata.
    pub fn aegis_invariants(&self) -> Option<&str> {
        self.metadata.as_ref()?.get("aegis-invariants").map(|s| s.as_str())
    }

    /// Get the `version` from metadata.
    pub fn version(&self) -> Option<&str> {
        self.metadata.as_ref()?.get("version").map(|s| s.as_str())
    }

    /// Get the `author` from metadata.
    pub fn author(&self) -> Option<&str> {
        self.metadata.as_ref()?.get("author").map(|s| s.as_str())
    }

    /// Get the `tags` from metadata as a slice.
    pub fn tags(&self) -> Vec<&str> {
        self.metadata
            .as_ref()
            .and_then(|m| m.get("tags"))
            .map(|t| t.split_whitespace().collect())
            .unwrap_or_default()
    }

    /// Get the allowed tools as a vector.
    pub fn allowed_tools_list(&self) -> Vec<&str> {
        self.allowed_tools
            .as_ref()
            .map(|t| t.split_whitespace().collect())
            .unwrap_or_default()
    }
}

// ═══════════════════════════════════════════════════════════════════
//  V1.0.0 Vendor-Neutral Frontmatter Types
// ═══════════════════════════════════════════════════════════════════

/// Skill ID in namespace format: `skill:<category>:<name>`
///
/// This provides a globally unique, federated identifier for skills.
/// The category enables registry-level organization.
///
/// # Example
///
/// ```
/// use ferris_aegis_skills::SkillId;
///
/// let id = SkillId::parse("skill:research:research-planner").unwrap();
/// assert_eq!(id.category, "research");
/// assert_eq!(id.name, "research-planner");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct SkillId {
    /// The skill category (e.g., "research", "search", "analysis", "trust").
    pub category: String,
    /// The skill name within the category.
    pub name: String,
}

impl SkillId {
    /// Parse a skill ID from the `skill:<category>:<name>` format.
    pub fn parse(id: &str) -> Result<Self, SkillError> {
        let parts: Vec<&str> = id.splitn(3, ':').collect();
        if parts.len() != 3 || parts[0] != "skill" {
            return Err(SkillError::InvalidSkillId {
                id: id.to_string(),
                reason: "Must follow 'skill:<category>:<name>' format".to_string(),
            });
        }
        let category = parts[1].to_string();
        let name = parts[2].to_string();

        // Validate category
        let cat_re = regex::Regex::new(r"^[a-z0-9]+(-[a-z0-9]+)*$").unwrap();
        if !cat_re.is_match(&category) {
            return Err(SkillError::InvalidSkillId {
                id: id.to_string(),
                reason: format!("Category '{}' must be lowercase alphanumeric with hyphens", category),
            });
        }

        // Validate name
        if !cat_re.is_match(&name) {
            return Err(SkillError::InvalidSkillId {
                id: id.to_string(),
                reason: format!("Name '{}' must be lowercase alphanumeric with hyphens", name),
            });
        }

        Ok(Self { category, name })
    }

    /// Create a new SkillId from category and name parts.
    pub fn new(category: &str, name: &str) -> Self {
        Self {
            category: category.to_string(),
            name: name.to_string(),
        }
    }

    /// Format as the canonical string representation.
    pub fn to_string(&self) -> String {
        format!("skill:{}:{}", self.category, self.name)
    }
}

impl std::fmt::Display for SkillId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "skill:{}:{}", self.category, self.name)
    }
}

/// A capability declaration in `<domain>.<operation>` format.
///
/// # Example
///
/// ```
/// use ferris_aegis_skills::Capability;
///
/// let cap = Capability::parse("network.http.get").unwrap();
/// assert_eq!(cap.domain, "network");
/// assert_eq!(cap.operation, "http.get");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Capability {
    /// The permission domain (e.g., "network", "filesystem", "crypto").
    pub domain: String,
    /// The specific operation (e.g., "http.get", "read.tmp", "hash").
    pub operation: String,
}

impl Capability {
    /// Parse a capability from `<domain>.<operation>` format.
    pub fn parse(perm: &str) -> Result<Self, SkillError> {
        let dot_pos = perm
            .find('.')
            .ok_or_else(|| SkillError::InvalidCapability {
                capability: perm.to_string(),
                reason: "Must follow '<domain>.<operation>' format".to_string(),
            })?;

        let domain = perm[..dot_pos].to_string();
        let operation = perm[dot_pos + 1..].to_string();

        if domain.is_empty() || operation.is_empty() {
            return Err(SkillError::InvalidCapability {
                capability: perm.to_string(),
                reason: "Both domain and operation must be non-empty".to_string(),
            });
        }

        Ok(Self { domain, operation })
    }

    /// Create a new Capability from domain and operation parts.
    pub fn new(domain: &str, operation: &str) -> Self {
        Self {
            domain: domain.to_string(),
            operation: operation.to_string(),
        }
    }
}

impl std::fmt::Display for Capability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.domain, self.operation)
    }
}

/// Required trust level for skill activation.
///
/// Maps directly to the Ferris Aegis trust level hierarchy.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum TrustLevelRequired {
    /// No trust required (0.00+)
    Unverified,
    /// Minimal trust required (0.20+)
    Probationary,
    /// Standard trust required (0.50+)
    Standard,
    /// Elevated trust required (0.75+)
    Elevated,
    /// Maximum trust required (0.95+)
    Sovereign,
}

impl TrustLevelRequired {
    /// Parse from string representation.
    pub fn from_str_opt(s: &str) -> Result<Self, SkillError> {
        match s {
            "Unverified" => Ok(Self::Unverified),
            "Probationary" => Ok(Self::Probationary),
            "Standard" => Ok(Self::Standard),
            "Elevated" => Ok(Self::Elevated),
            "Sovereign" => Ok(Self::Sovereign),
            _ => Err(SkillError::InvalidTrustLevel {
                level: s.to_string(),
                reason: "Must be one of: Unverified, Probationary, Standard, Elevated, Sovereign"
                    .to_string(),
            }),
        }
    }

    /// Get the minimum trust score required for this level.
    pub fn min_score(&self) -> f64 {
        match self {
            Self::Unverified => 0.00,
            Self::Probationary => 0.20,
            Self::Standard => 0.50,
            Self::Elevated => 0.75,
            Self::Sovereign => 0.95,
        }
    }
}

impl std::fmt::Display for TrustLevelRequired {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unverified => write!(f, "Unverified"),
            Self::Probationary => write!(f, "Probationary"),
            Self::Standard => write!(f, "Standard"),
            Self::Elevated => write!(f, "Elevated"),
            Self::Sovereign => write!(f, "Sovereign"),
        }
    }
}

/// Type of dependency.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DependencyType {
    /// Dependency on another skill.
    Skill,
    /// Dependency on an external tool.
    Tool,
    /// Dependency on an AI model.
    Model,
}

/// A dependency with version constraint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    /// The dependency identifier (skill ID, tool name, or model name).
    pub id: String,
    /// Semantic version requirement (e.g., ">=1.0.0").
    #[serde(default = "Dependency::default_version")]
    pub version: String,
    /// Whether this dependency is optional.
    #[serde(default)]
    pub optional: bool,
    /// The type of dependency.
    #[serde(rename = "type", default = "Dependency::default_type")]
    pub dep_type: DependencyType,
    /// Purpose of this dependency (for models).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,
}

impl Dependency {
    fn default_version() -> String {
        "*".to_string()
    }

    fn default_type() -> DependencyType {
        DependencyType::Skill
    }
}

/// A typed input parameter for a skill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInput {
    /// Parameter name.
    pub name: String,
    /// Parameter type (string, integer, float, boolean, json, markdown, enum:A,B, path, url).
    #[serde(rename = "type")]
    pub input_type: String,
    /// Whether this parameter is required.
    #[serde(default)]
    pub required: bool,
    /// Human-readable description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Default value (used when required=false).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_yaml::Value>,
    /// Validation regex constraint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation: Option<String>,
}

/// A typed output from a skill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillOutput {
    /// Output name.
    pub name: String,
    /// Output type (string, integer, float, boolean, json, json[], markdown, path, url).
    #[serde(rename = "type")]
    pub output_type: String,
    /// Human-readable description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Network sandbox constraints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkSandbox {
    /// Allowed domains for outbound network access.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_domains: Option<Vec<String>>,
    /// Maximum number of network requests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_requests: Option<u32>,
}

/// Filesystem sandbox constraints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesystemSandbox {
    /// Allowed file paths (glob patterns).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_paths: Option<Vec<String>>,
    /// Maximum file size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_file_size: Option<String>,
}

/// Compute sandbox constraints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeSandbox {
    /// Maximum memory allocation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_memory: Option<String>,
    /// Maximum CPU seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_cpu_seconds: Option<u64>,
}

/// Sandbox constraints for skill execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConstraints {
    /// Network access constraints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<NetworkSandbox>,
    /// Filesystem access constraints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filesystem: Option<FilesystemSandbox>,
    /// Compute resource constraints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compute: Option<ComputeSandbox>,
}

/// Cryptographic signature for skill provenance (Layer 5).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSignature {
    /// Signing algorithm (e.g., "ed25519").
    pub algorithm: String,
    /// Public key for verification.
    pub public_key: String,
    /// The signature value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// When the signature was created.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signed_at: Option<DateTime<Utc>>,
    /// Who signed the skill.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signed_by: Option<String>,
}

/// Ferris Aegis runtime extension block (Layer 2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FerrisAegisExtension {
    /// Required trust level for skill activation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trust_level: Option<String>,
    /// Policy rules enforced during execution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policies: Option<Vec<String>>,
    /// Whether audit trail is enabled for this skill.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audit: Option<bool>,
    /// Cryptographic signature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<SkillSignature>,
    /// Aegis-specific sandbox overrides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<FerrisAegisSandbox>,
    /// Primary crate this skill wraps (migration from v0.2.0 metadata).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crate_name: Option<String>,
    /// Development phase (migration from v0.2.0 metadata).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
    /// Security invariants enforced (migration from v0.2.0 metadata).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invariants: Option<String>,
}

/// Ferris Aegis sandbox overrides.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FerrisAegisSandbox {
    /// WASM module to execute in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wasm_module: Option<String>,
    /// WASM fuel limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fuel: Option<u64>,
    /// Memory limit for sandboxed execution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_limit: Option<String>,
}

/// Platform compatibility entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformCompat {
    /// Platform/runtime name.
    pub name: String,
    /// Minimum compatible version.
    pub min_version: String,
    /// Maximum compatible version (optional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_version: Option<String>,
}

/// Validation test case.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationTest {
    /// Test name.
    pub name: String,
    /// Test input values.
    pub input: serde_yaml::Value,
    /// Expected output conditions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_output: Option<serde_yaml::Value>,
    /// Test timeout in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
}

/// Validation block for skill self-testing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationBlock {
    /// Test cases.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tests: Option<Vec<ValidationTest>>,
    /// Example usage prompts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub examples: Option<Vec<String>>,
}

/// Skill lifecycle state (Layer 6).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LifecycleState {
    /// Under development, not for production.
    Draft,
    /// Production-ready, maintained.
    Stable,
    /// Superseded, will be retired.
    Deprecated,
    /// No longer supported.
    Retired,
}

/// Lifecycle metadata (Layer 6).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleBlock {
    /// Current lifecycle state.
    pub state: LifecycleState,
    /// Date since this state applies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    /// Deprecation notice message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecation_notice: Option<String>,
    /// Replacement skill ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replacement: Option<String>,
    /// Migration guide URL or description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migration_guide: Option<String>,
}

/// Dependencies block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependenciesBlock {
    /// Skill dependencies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skills: Option<Vec<Dependency>>,
    /// Tool dependencies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Dependency>>,
    /// Model dependencies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<Dependency>>,
}

/// V1.0.0 vendor-neutral SKILL.md frontmatter.
///
/// This is a strict superset of the v0.2.0 `SkillFrontmatter`.
/// Skills that set `spec_version: "1.0.0"` use this richer format.
/// Skills without `spec_version` are treated as v0.2.0.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VendorNeutralFrontmatter {
    // ── Identity (Layer 1) ───────────────────────────────────
    /// Specification version. Must be "1.0.0" for this format.
    pub spec_version: String,
    /// Namespaced skill ID: `skill:<category>:<name>`.
    pub id: String,
    /// Short skill name (1-64 chars, must match directory).
    pub name: String,
    /// Semantic version.
    pub version: String,
    /// When this skill should activate. Must include "Use when...".
    pub description: String,
    /// Author or organization identifier.
    pub author: String,
    /// SPDX license identifier.
    pub license: String,

    // ── Runtime & Compatibility ──────────────────────────────
    /// Execution model: "mcp", "cli", "http", "wasm", "native".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<String>,
    /// Platform compatibility matrix.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platforms: Option<Vec<PlatformCompat>>,
    /// Runtime-specific invocation target.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<String>,
    /// Max execution time in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,

    // ── Capability Model (Layer 7) ──────────────────────────
    /// Permission declarations in `<domain>.<operation>` format.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Vec<String>>,

    // ── I/O Contract ────────────────────────────────────────
    /// Typed input parameters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inputs: Option<Vec<SkillInput>>,
    /// Typed output declarations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outputs: Option<Vec<SkillOutput>>,

    // ── Sandbox Constraints ─────────────────────────────────
    /// Sandbox constraints for execution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<SandboxConstraints>,

    // ── Context & Dependencies (Layer 8) ────────────────────
    /// Skills/tools that MUST be available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_context: Option<Vec<String>>,
    /// Skills/tools that ENHANCE the skill.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub optional_context: Option<Vec<String>>,
    /// Typed dependencies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dependencies: Option<DependenciesBlock>,

    // ── Validation ──────────────────────────────────────────
    /// Self-testing configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation: Option<ValidationBlock>,

    // ── Lifecycle (Layer 6) ─────────────────────────────────
    /// Lifecycle state and metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifecycle: Option<LifecycleBlock>,

    // ── Runtime Extensions (Layer 2) ────────────────────────
    /// Ferris Aegis runtime extension.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ferris_aegis: Option<FerrisAegisExtension>,

    // ── Backward compat fields ──────────────────────────────
    /// Legacy compatibility string (v0.2.0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compatibility: Option<String>,
    /// Legacy metadata (v0.2.0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
    /// Legacy allowed-tools (v0.2.0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "allowed-tools")]
    pub allowed_tools: Option<String>,
}

impl VendorNeutralFrontmatter {
    /// Parse the `id` field into a structured SkillId.
    pub fn skill_id(&self) -> Result<SkillId, SkillError> {
        SkillId::parse(&self.id)
    }

    /// Parse all permissions into structured Capability objects.
    pub fn capabilities(&self) -> Vec<Result<Capability, SkillError>> {
        self.permissions
            .as_ref()
            .map(|perms| perms.iter().map(|p| Capability::parse(p)).collect())
            .unwrap_or_default()
    }

    /// Get the required trust level from the Ferris Aegis extension.
    pub fn trust_level_required(&self) -> Option<Result<TrustLevelRequired, SkillError>> {
        self.ferris_aegis
            .as_ref()
            .and_then(|ext| ext.trust_level.as_ref())
            .map(|tl| TrustLevelRequired::from_str_opt(tl))
    }

    /// Check if this frontmatter uses the v1.0.0 vendor-neutral format.
    pub fn is_vendor_neutral(&self) -> bool {
        self.spec_version == "1.0.0"
    }

    /// Get the effective trust level, considering both the explicit setting
    /// and the computed level from permissions.
    pub fn effective_trust_level(&self) -> TrustLevelRequired {
        // If explicitly set in extension block, use it
        if let Some(Ok(tl)) = self.trust_level_required() {
            return tl;
        }

        // Otherwise compute from permissions
        let caps = self.capabilities();
        let mut max_level = TrustLevelRequired::Unverified;

        for cap_result in caps {
            if let Ok(cap) = cap_result {
                let required = match cap.domain.as_str() {
                    "network" => TrustLevelRequired::Standard,
                    "filesystem" => {
                        if cap.operation.starts_with("write") {
                            TrustLevelRequired::Elevated
                        } else {
                            TrustLevelRequired::Probationary
                        }
                    }
                    "process" => TrustLevelRequired::Elevated,
                    "crypto" => {
                        if cap.operation == "sign" {
                            TrustLevelRequired::Sovereign
                        } else {
                            TrustLevelRequired::Elevated
                        }
                    }
                    "agent" => {
                        if cap.operation == "spawn" {
                            TrustLevelRequired::Sovereign
                        } else {
                            TrustLevelRequired::Unverified
                        }
                    }
                    _ => TrustLevelRequired::Standard,
                };

                if required > max_level {
                    max_level = required;
                }
            }
        }

        max_level
    }

    /// Get all skill dependencies.
    pub fn skill_dependencies(&self) -> Vec<&Dependency> {
        self.dependencies
            .as_ref()
            .and_then(|d| d.skills.as_ref())
            .map(|s| s.iter().collect())
            .unwrap_or_default()
    }

    /// Get the lifecycle state, defaulting to Stable.
    pub fn lifecycle_state(&self) -> LifecycleState {
        self.lifecycle
            .as_ref()
            .map(|l| l.state.clone())
            .unwrap_or(LifecycleState::Stable)
    }
}

// ═══════════════════════════════════════════════════════════════════
//  Unified Frontmatter — supports both v0.2.0 and v1.0.0
// ═══════════════════════════════════════════════════════════════════

/// Unified frontmatter that can represent both v0.2.0 and v1.0.0 formats.
///
/// The parser first attempts to deserialize as v1.0.0 (checking for
/// `spec_version`), and falls back to v0.2.0 if the field is absent.
#[derive(Debug, Clone)]
pub enum UnifiedFrontmatter {
    /// Legacy agentskills.io v0.2.0 format.
    V02(SkillFrontmatter),
    /// Vendor-neutral v1.0.0 format.
    V10(VendorNeutralFrontmatter),
}

impl UnifiedFrontmatter {
    /// Get the skill name (common to both formats).
    pub fn name(&self) -> &str {
        match self {
            Self::V02(fm) => &fm.name,
            Self::V10(fm) => &fm.name,
        }
    }

    /// Get the description (common to both formats).
    pub fn description(&self) -> &str {
        match self {
            Self::V02(fm) => &fm.description,
            Self::V10(fm) => &fm.description,
        }
    }

    /// Get the license (common to both formats, optional in v0.2.0).
    pub fn license(&self) -> Option<&str> {
        match self {
            Self::V02(fm) => fm.license.as_deref(),
            Self::V10(fm) => Some(&fm.license),
        }
    }

    /// Check if this is the v1.0.0 vendor-neutral format.
    pub fn is_vendor_neutral(&self) -> bool {
        matches!(self, Self::V10(_))
    }

    /// Get the v1.0.0 frontmatter if available.
    pub fn as_v10(&self) -> Option<&VendorNeutralFrontmatter> {
        match self {
            Self::V10(fm) => Some(fm),
            _ => None,
        }
    }

    /// Get the v0.2.0 frontmatter if available.
    pub fn as_v02(&self) -> Option<&SkillFrontmatter> {
        match self {
            Self::V02(fm) => Some(fm),
            _ => None,
        }
    }

    /// Get the effective version string.
    pub fn version(&self) -> &str {
        match self {
            Self::V02(fm) => fm.version().unwrap_or("0.0.0"),
            Self::V10(fm) => &fm.version,
        }
    }

    /// Get the effective author string.
    pub fn author(&self) -> &str {
        match self {
            Self::V02(fm) => fm.author().unwrap_or("unknown"),
            Self::V10(fm) => &fm.author,
        }
    }

    /// Get the skill ID (namespaced for v1.0.0, plain name for v0.2.0).
    pub fn skill_id(&self) -> String {
        match self {
            Self::V02(fm) => fm.name.clone(),
            Self::V10(fm) => fm.id.clone(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
//  Error Types
// ═══════════════════════════════════════════════════════════════════

/// Comprehensive error type for the skills crate.
#[derive(Debug, thiserror::Error)]
pub enum SkillError {
    /// Invalid skill ID format.
    #[error("Invalid skill ID '{id}': {reason}")]
    InvalidSkillId {
        /// The malformed ID.
        id: String,
        /// Why it's invalid.
        reason: String,
    },

    /// Invalid capability format.
    #[error("Invalid capability '{capability}': {reason}")]
    InvalidCapability {
        /// The malformed capability string.
        capability: String,
        /// Why it's invalid.
        reason: String,
    },

    /// Invalid trust level.
    #[error("Invalid trust level '{level}': {reason}")]
    InvalidTrustLevel {
        /// The unrecognized level.
        level: String,
        /// Why it's invalid.
        reason: String,
    },

    /// Insufficient trust level for skill activation.
    #[error("Insufficient trust level: required {required:?}, actual {actual:?}")]
    InsufficientTrustLevel {
        /// The required trust level.
        required: TrustLevelRequired,
        /// The agent's actual trust level.
        actual: TrustLevelRequired,
    },

    /// Skill not found in registry.
    #[error("Skill not found: {0}")]
    SkillNotFound(String),

    /// Dependency not satisfied.
    #[error("Dependency not satisfied: {dependency_id} required by {skill_id}")]
    DependencyNotSatisfied {
        /// The skill requiring the dependency.
        skill_id: String,
        /// The missing dependency ID.
        dependency_id: String,
    },

    /// Dependency cycle detected.
    #[error("Dependency cycle detected: {cycle:?}")]
    DependencyCycle {
        /// The cycle as a list of skill IDs.
        cycle: Vec<String>,
    },

    /// Signature verification failed.
    #[error("Signature verification failed for skill: {0}")]
    SignatureVerificationFailed(String),

    /// Frontmatter parsing error.
    #[error("Frontmatter parsing error: {0}")]
    FrontmatterParseError(String),

    /// Validation error.
    #[error("Validation error in {field}: {message}")]
    ValidationError {
        /// The field that failed.
        field: String,
        /// The error message.
        message: String,
    },

    /// IO error.
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// YAML parsing error.
    #[error("YAML parsing error: {0}")]
    YamlError(String),
}

// ═══════════════════════════════════════════════════════════════════
//  Validation
// ═══════════════════════════════════════════════════════════════════

/// Validation error for a SKILL.md file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    /// The field that failed validation.
    pub field: String,
    /// The validation rule that was violated.
    pub rule: String,
    /// A human-readable error message.
    pub message: String,
}

/// Result of validating a skill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// The skill name that was validated.
    pub skill_name: String,
    /// The directory path of the skill.
    pub path: String,
    /// Whether the skill is valid (no errors).
    pub valid: bool,
    /// Validation errors (empty if valid).
    pub errors: Vec<ValidationError>,
    /// Validation warnings (non-fatal issues).
    pub warnings: Vec<String>,
}

impl ValidationResult {
    /// Whether the skill passed validation.
    pub fn is_valid(&self) -> bool {
        self.valid
    }
}

/// Validates a skill's frontmatter and directory structure.
pub struct SkillValidator;

impl SkillValidator {
    /// Validate a v0.2.0 skill directory.
    pub fn validate_v02(path: &Path, frontmatter: &SkillFrontmatter) -> ValidationResult {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        let dir_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        // Rule 1: Name matches directory name
        if frontmatter.name != dir_name {
            errors.push(ValidationError {
                field: "name".to_string(),
                rule: "name-matches-directory".to_string(),
                message: format!(
                    "Frontmatter name '{}' does not match directory name '{}'",
                    frontmatter.name, dir_name
                ),
            });
        }

        // Rule 2: Name follows convention
        let name_re = regex::Regex::new(r"^[a-z0-9]+(-[a-z0-9]+)*$").unwrap();
        if !name_re.is_match(&frontmatter.name) {
            errors.push(ValidationError {
                field: "name".to_string(),
                rule: "name-format".to_string(),
                message: format!(
                    "Name '{}' must be lowercase alphanumeric with hyphens",
                    frontmatter.name
                ),
            });
        }

        // Rule 3: Name length
        if frontmatter.name.is_empty() {
            errors.push(ValidationError {
                field: "name".to_string(),
                rule: "name-min-length".to_string(),
                message: "Name must be at least 1 character".to_string(),
            });
        } else if frontmatter.name.len() > 64 {
            errors.push(ValidationError {
                field: "name".to_string(),
                rule: "name-max-length".to_string(),
                message: format!("Name is {} chars, max is 64", frontmatter.name.len()),
            });
        }

        // Rule 4: Name does not start/end with hyphen
        if frontmatter.name.starts_with('-') || frontmatter.name.ends_with('-') {
            errors.push(ValidationError {
                field: "name".to_string(),
                rule: "name-hyphen-bounds".to_string(),
                message: "Name must not start or end with a hyphen".to_string(),
            });
        }

        // Rule 5: Name no consecutive hyphens
        if frontmatter.name.contains("--") {
            errors.push(ValidationError {
                field: "name".to_string(),
                rule: "name-consecutive-hyphens".to_string(),
                message: "Name must not contain consecutive hyphens".to_string(),
            });
        }

        // Rule 6: Description length
        if frontmatter.description.is_empty() {
            errors.push(ValidationError {
                field: "description".to_string(),
                rule: "description-min-length".to_string(),
                message: "Description must not be empty".to_string(),
            });
        } else if frontmatter.description.len() > 1024 {
            errors.push(ValidationError {
                field: "description".to_string(),
                rule: "description-max-length".to_string(),
                message: format!(
                    "Description is {} chars, max is 1024",
                    frontmatter.description.len()
                ),
            });
        }

        // Rule 7: No angle brackets in frontmatter
        let fm_str = serde_yaml::to_string(frontmatter).unwrap_or_default();
        if fm_str.contains('<') || fm_str.contains('>') {
            errors.push(ValidationError {
                field: "frontmatter".to_string(),
                rule: "no-angle-brackets".to_string(),
                message: "Angle brackets (< or >) in frontmatter may inject unintended instructions"
                    .to_string(),
            });
        }

        // Rule 8: Description should include "Use when"
        if !frontmatter.description.contains("Use when") && !frontmatter.description.contains("use when") {
            warnings.push(
                "Description should include 'Use when...' trigger phrases for better activation"
                    .to_string(),
            );
        }

        // Rule 9: Compatibility field length
        if let Some(ref compat) = frontmatter.compatibility {
            if compat.len() > 500 {
                errors.push(ValidationError {
                    field: "compatibility".to_string(),
                    rule: "compatibility-max-length".to_string(),
                    message: format!(
                        "Compatibility is {} chars, max is 500",
                        compat.len()
                    ),
                });
            }
        }

        let valid = errors.is_empty();
        ValidationResult {
            skill_name: frontmatter.name.clone(),
            path: path.to_string_lossy().to_string(),
            valid,
            errors,
            warnings,
        }
    }

    /// Validate a v1.0.0 vendor-neutral skill directory.
    ///
    /// Applies all 15 Layer 1 validation rules plus Layer 2 extension checks.
    pub fn validate_v10(path: &Path, frontmatter: &VendorNeutralFrontmatter) -> ValidationResult {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        let dir_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        // Rule 1: spec_version must be "1.0.0"
        if frontmatter.spec_version != "1.0.0" {
            errors.push(ValidationError {
                field: "spec_version".to_string(),
                rule: "spec-version".to_string(),
                message: format!(
                    "spec_version must be '1.0.0', got '{}'",
                    frontmatter.spec_version
                ),
            });
        }

        // Rule 2: id must follow skill:<category>:<name> format
        if let Err(e) = SkillId::parse(&frontmatter.id) {
            errors.push(ValidationError {
                field: "id".to_string(),
                rule: "id-format".to_string(),
                message: format!("id field invalid: {}", e),
            });
        }

        // Rule 3: name must follow naming convention
        let name_re = regex::Regex::new(r"^[a-z0-9]+(-[a-z0-9]+)*$").unwrap();
        if !name_re.is_match(&frontmatter.name) {
            errors.push(ValidationError {
                field: "name".to_string(),
                rule: "name-format".to_string(),
                message: format!(
                    "Name '{}' must be lowercase alphanumeric with hyphens",
                    frontmatter.name
                ),
            });
        }

        // Rule 4: name must match directory name
        if frontmatter.name != dir_name {
            errors.push(ValidationError {
                field: "name".to_string(),
                rule: "name-matches-directory".to_string(),
                message: format!(
                    "Frontmatter name '{}' does not match directory name '{}'",
                    frontmatter.name, dir_name
                ),
            });
        }

        // Rule 5: version must be semver
        let semver_re = regex::Regex::new(r"^\d+\.\d+\.\d+$").unwrap();
        if !semver_re.is_match(&frontmatter.version) {
            errors.push(ValidationError {
                field: "version".to_string(),
                rule: "version-semver".to_string(),
                message: format!(
                    "Version '{}' must follow MAJOR.MINOR.PATCH format",
                    frontmatter.version
                ),
            });
        }

        // Rule 6: description length and content
        if frontmatter.description.is_empty() {
            errors.push(ValidationError {
                field: "description".to_string(),
                rule: "description-min-length".to_string(),
                message: "Description must not be empty".to_string(),
            });
        } else if frontmatter.description.len() > 1024 {
            errors.push(ValidationError {
                field: "description".to_string(),
                rule: "description-max-length".to_string(),
                message: format!(
                    "Description is {} chars, max is 1024",
                    frontmatter.description.len()
                ),
            });
        }

        // Rule 7: description should include "Use when" (warning)
        if !frontmatter.description.contains("Use when") && !frontmatter.description.contains("use when") {
            warnings.push(
                "Description should include 'Use when...' trigger phrases for better activation"
                    .to_string(),
            );
        }

        // Rule 8: no angle brackets in frontmatter
        if frontmatter.description.contains('<') || frontmatter.description.contains('>') {
            errors.push(ValidationError {
                field: "description".to_string(),
                rule: "no-angle-brackets".to_string(),
                message: "Angle brackets in description may inject unintended instructions"
                    .to_string(),
            });
        }

        // Rule 9: permissions format
        if let Some(ref perms) = frontmatter.permissions {
            for perm in perms {
                if let Err(e) = Capability::parse(perm) {
                    errors.push(ValidationError {
                        field: "permissions".to_string(),
                        rule: "permission-format".to_string(),
                        message: format!("Permission '{}' invalid: {}", perm, e),
                    });
                }
            }
        }

        // Rule 10: inputs must have name, type, required
        if let Some(ref inputs) = frontmatter.inputs {
            for input in inputs {
                if input.name.is_empty() {
                    errors.push(ValidationError {
                        field: format!("inputs[{}]", input.name),
                        rule: "input-name-required".to_string(),
                        message: "Input name must not be empty".to_string(),
                    });
                }
                if input.input_type.is_empty() {
                    errors.push(ValidationError {
                        field: format!("inputs[{}]", input.name),
                        rule: "input-type-required".to_string(),
                        message: format!("Input '{}' must have a type", input.name),
                    });
                }
            }
        }

        // Rule 11: outputs must have name, type
        if let Some(ref outputs) = frontmatter.outputs {
            for output in outputs {
                if output.name.is_empty() {
                    errors.push(ValidationError {
                        field: format!("outputs[{}]", output.name),
                        rule: "output-name-required".to_string(),
                        message: "Output name must not be empty".to_string(),
                    });
                }
                if output.output_type.is_empty() {
                    errors.push(ValidationError {
                        field: format!("outputs[{}]", output.name),
                        rule: "output-type-required".to_string(),
                        message: format!("Output '{}' must have a type", output.name),
                    });
                }
            }
        }

        // Rule 12: dependency IDs format
        if let Some(ref deps) = frontmatter.dependencies {
            if let Some(ref skill_deps) = deps.skills {
                for dep in skill_deps {
                    if dep.dep_type == DependencyType::Skill {
                        if let Err(e) = SkillId::parse(&dep.id) {
                            errors.push(ValidationError {
                                field: "dependencies.skills".to_string(),
                                rule: "dependency-id-format".to_string(),
                                message: format!("Skill dependency '{}' invalid: {}", dep.id, e),
                            });
                        }
                    }
                }
            }
        }

        // Rule 13: trust level in extension (Layer 2 check)
        if let Some(ref ext) = frontmatter.ferris_aegis {
            if let Some(ref tl) = ext.trust_level {
                if let Err(e) = TrustLevelRequired::from_str_opt(tl) {
                    errors.push(ValidationError {
                        field: "ferris_aegis.trust_level".to_string(),
                        rule: "trust-level-valid".to_string(),
                        message: format!("Invalid trust level '{}': {}", tl, e),
                    });
                }
            }
        }

        // Rule 14: validation tests exist (warning)
        if frontmatter.validation.as_ref().and_then(|v| v.tests.as_ref()).is_none() {
            warnings.push(
                "No validation tests defined. Consider adding validation.tests for self-testing."
                    .to_string(),
            );
        }

        // Rule 15: sandbox domains non-empty if network permission declared
        if let Some(ref perms) = frontmatter.permissions {
            let has_network = perms.iter().any(|p| p.starts_with("network."));
            if has_network {
                let has_sandbox_domains = frontmatter
                    .sandbox
                    .as_ref()
                    .and_then(|s| s.network.as_ref())
                    .and_then(|n| n.allowed_domains.as_ref())
                    .map(|d| !d.is_empty())
                    .unwrap_or(false);

                if !has_sandbox_domains {
                    warnings.push(
                        "Network permissions declared but sandbox.network.allowed_domains is empty. \
                         Consider restricting allowed domains for security."
                            .to_string(),
                    );
                }
            }
        }

        let valid = errors.is_empty();
        ValidationResult {
            skill_name: frontmatter.name.clone(),
            path: path.to_string_lossy().to_string(),
            valid,
            errors,
            warnings,
        }
    }

    /// Validate a unified frontmatter (dispatches to the appropriate validator).
    pub fn validate(path: &Path, frontmatter: &UnifiedFrontmatter) -> ValidationResult {
        match frontmatter {
            UnifiedFrontmatter::V02(fm) => Self::validate_v02(path, fm),
            UnifiedFrontmatter::V10(fm) => Self::validate_v10(path, fm),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
//  Skill Metadata (Tier 1)
// ═══════════════════════════════════════════════════════════════════

/// Tier 1: Skill metadata for discovery.
///
/// This is what agents load at startup (~100 tokens per skill).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetadata {
    /// Unique skill identifier.
    pub name: String,
    /// When this skill should activate.
    pub description: String,
    /// The directory path where this skill lives.
    pub path: PathBuf,
    /// Content digest of the SKILL.md file (SHA-256).
    pub digest: String,
    /// The unified frontmatter (v0.2.0 or v1.0.0).
    pub frontmatter: UnifiedFrontmatterRepr,
}

/// Serializable representation of the unified frontmatter.
///
/// Stores the raw YAML to avoid complex enum serialization issues.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedFrontmatterRepr {
    /// The raw YAML string of the frontmatter.
    pub raw_yaml: String,
    /// Whether this is v1.0.0 format.
    pub is_v10: bool,
    /// The skill name.
    pub name: String,
    /// The skill description.
    pub description: String,
    /// The skill ID (namespaced for v1.0.0, plain name for v0.2.0).
    pub skill_id: String,
    /// The skill version.
    pub version: String,
    /// The skill author.
    pub author: String,
    /// The license.
    pub license: Option<String>,
}

impl UnifiedFrontmatterRepr {
    /// Create from a v0.2.0 frontmatter.
    pub fn from_v02(fm: &SkillFrontmatter) -> Self {
        Self {
            raw_yaml: serde_yaml::to_string(fm).unwrap_or_default(),
            is_v10: false,
            name: fm.name.clone(),
            description: fm.description.clone(),
            skill_id: fm.name.clone(),
            version: fm.version().unwrap_or("0.0.0").to_string(),
            author: fm.author().unwrap_or("unknown").to_string(),
            license: fm.license.clone(),
        }
    }

    /// Create from a v1.0.0 frontmatter.
    pub fn from_v10(fm: &VendorNeutralFrontmatter) -> Self {
        Self {
            raw_yaml: serde_yaml::to_string(fm).unwrap_or_default(),
            is_v10: true,
            name: fm.name.clone(),
            description: fm.description.clone(),
            skill_id: fm.id.clone(),
            version: fm.version.clone(),
            author: fm.author.clone(),
            license: Some(fm.license.clone()),
        }
    }
}

impl SkillMetadata {
    /// Compute the SHA-256 digest of the SKILL.md file content.
    pub fn compute_digest(content: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content);
        hex::encode(hasher.finalize())
    }
}

// ═══════════════════════════════════════════════════════════════════
//  Skill Instructions (Tier 2)
// ═══════════════════════════════════════════════════════════════════

/// Tier 2: Full skill with instructions loaded.
///
/// This is what agents load when they activate a skill (<5,000 tokens).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    /// The skill metadata (Tier 1).
    pub metadata: SkillMetadata,
    /// The markdown instruction body (after frontmatter).
    pub instructions: String,
    /// Available resource files (scripts/, references/, assets/).
    pub resources: SkillResources,
}

/// Resource files available in a skill directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillResources {
    /// Executable scripts in scripts/ directory.
    pub scripts: Vec<PathBuf>,
    /// Reference documents in references/ directory.
    pub references: Vec<PathBuf>,
    /// Static assets in assets/ directory.
    pub assets: Vec<PathBuf>,
}

impl Default for SkillResources {
    fn default() -> Self {
        Self {
            scripts: Vec::new(),
            references: Vec::new(),
            assets: Vec::new(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
//  Discovery Index
// ═══════════════════════════════════════════════════════════════════

/// A skill entry in the discovery index.
///
/// Complies with agentskills.io v0.2.0 and v1.0.0 discovery schemas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillIndexEntry {
    /// URL-safe slug from the skill name.
    pub name: String,
    /// Always "skill-md".
    #[serde(rename = "type")]
    pub entry_type: String,
    /// Brief description from frontmatter (truncated to 1024 chars).
    pub description: String,
    /// URL path to fetch the full SKILL.md.
    pub url: String,
    /// SHA-256 digest of the SKILL.md content.
    pub digest: String,

    // ── v1.0.0 extended fields ──────────────────────────────
    /// Namespaced skill ID (v1.0.0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Skill version (v1.0.0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Runtime type (v1.0.0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<String>,
    /// Declared permissions (v1.0.0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Vec<String>>,
    /// Author (v1.0.0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// License (v1.0.0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    /// Lifecycle state (v1.0.0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifecycle: Option<String>,
    /// Trust level (v1.0.0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trust_level: Option<String>,
}

/// The discovery index manifest.
///
/// Supports both v0.2.0 and v1.0.0 discovery schemas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillIndex {
    /// JSON Schema URL.
    #[serde(rename = "$schema")]
    pub schema: String,
    /// Spec version for the index format.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spec_version: Option<String>,
    /// Registry name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registry: Option<String>,
    /// Last update timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    /// List of available skills.
    pub skills: Vec<SkillIndexEntry>,
}

impl SkillIndex {
    /// Create a new empty discovery index (v0.2.0 format).
    pub fn new() -> Self {
        Self {
            schema: "https://schemas.agentskills.io/discovery/0.2.0/schema.json".to_string(),
            spec_version: None,
            registry: None,
            updated_at: None,
            skills: Vec::new(),
        }
    }

    /// Create a new v1.0.0 discovery index.
    pub fn new_v10(registry_name: &str) -> Self {
        Self {
            schema: "https://schemas.agentskills.io/discovery/1.0.0/schema.json".to_string(),
            spec_version: Some("1.0.0".to_string()),
            registry: Some(registry_name.to_string()),
            updated_at: Some(Utc::now().to_rfc3339()),
            skills: Vec::new(),
        }
    }

    /// Add a skill to the index (v0.2.0 compatible).
    pub fn add_skill(&mut self, metadata: &SkillMetadata) {
        let entry = SkillIndexEntry {
            name: metadata.name.clone(),
            entry_type: "skill-md".to_string(),
            description: metadata.description.chars().take(1024).collect(),
            url: format!("/.well-known/agent-skills/{}/SKILL.md", metadata.name),
            digest: format!("sha256:{}", metadata.digest),
            // v1.0.0 fields populated from metadata
            id: if metadata.frontmatter.is_v10 {
                Some(metadata.frontmatter.skill_id.clone())
            } else {
                None
            },
            version: if metadata.frontmatter.is_v10 {
                Some(metadata.frontmatter.version.clone())
            } else {
                None
            },
            runtime: None,
            permissions: None,
            author: if metadata.frontmatter.is_v10 {
                Some(metadata.frontmatter.author.clone())
            } else {
                None
            },
            license: metadata.frontmatter.license.clone(),
            lifecycle: None,
            trust_level: None,
        };
        self.skills.push(entry);
    }

    /// Add a skill to the index with full v1.0.0 metadata.
    pub fn add_skill_v10(
        &mut self,
        metadata: &SkillMetadata,
        vn_frontmatter: &VendorNeutralFrontmatter,
    ) {
        let entry = SkillIndexEntry {
            name: metadata.name.clone(),
            entry_type: "skill-md".to_string(),
            description: metadata.description.chars().take(1024).collect(),
            url: format!("/.well-known/agent-skills/{}/SKILL.md", metadata.name),
            digest: format!("sha256:{}", metadata.digest),
            id: Some(vn_frontmatter.id.clone()),
            version: Some(vn_frontmatter.version.clone()),
            runtime: vn_frontmatter.runtime.clone(),
            permissions: vn_frontmatter.permissions.clone(),
            author: Some(vn_frontmatter.author.clone()),
            license: Some(vn_frontmatter.license.clone()),
            lifecycle: Some(
                match vn_frontmatter.lifecycle_state() {
                    LifecycleState::Draft => "draft",
                    LifecycleState::Stable => "stable",
                    LifecycleState::Deprecated => "deprecated",
                    LifecycleState::Retired => "retired",
                }
                .to_string(),
            ),
            trust_level: vn_frontmatter
                .ferris_aegis
                .as_ref()
                .and_then(|ext| ext.trust_level.clone()),
        };
        self.skills.push(entry);
    }

    /// Serialize the index to JSON.
    pub fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}

impl Default for SkillIndex {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════
//  Skill Registry
// ═══════════════════════════════════════════════════════════════════

/// Configuration for the skill registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillRegistryConfig {
    /// Root directory for skill discovery.
    pub skills_dir: PathBuf,
    /// Whether to validate skills on discovery.
    pub validate_on_discovery: bool,
    /// Whether to compute digests.
    pub compute_digests: bool,
}

impl Default for SkillRegistryConfig {
    fn default() -> Self {
        Self {
            skills_dir: PathBuf::from(".agents/skills"),
            validate_on_discovery: true,
            compute_digests: true,
        }
    }
}

/// The skill registry: discovers, validates, and loads skills.
///
/// Supports both v0.2.0 and v1.0.0 SKILL.md formats.
pub struct SkillRegistry {
    config: SkillRegistryConfig,
    skills: HashMap<String, SkillMetadata>,
    parsed_frontmatters: HashMap<String, UnifiedFrontmatter>,
    validation_results: Vec<ValidationResult>,
}

impl SkillRegistry {
    /// Create a new skill registry with the given configuration.
    pub fn new(config: SkillRegistryConfig) -> Self {
        Self {
            config,
            skills: HashMap::new(),
            parsed_frontmatters: HashMap::new(),
            validation_results: Vec::new(),
        }
    }

    /// Create a registry with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(SkillRegistryConfig::default())
    }

    /// Discover skills in the configured directory.
    ///
    /// Scans for SKILL.md files in immediate subdirectories.
    /// Supports both v0.2.0 and v1.0.0 frontmatter formats.
    /// Returns the number of skills discovered.
    pub async fn discover(&mut self, root: &str) -> anyhow::Result<usize> {
        let root_path = Path::new(root);
        if !root_path.exists() {
            tracing::warn!("Skills directory does not exist: {}", root);
            return Ok(0);
        }

        let mut count = 0;
        let mut entries = tokio::fs::read_dir(root_path).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let skill_md = path.join("SKILL.md");
            if !skill_md.exists() {
                continue;
            }

            match self.parse_skill_metadata(&path, &skill_md).await {
                Ok((metadata, frontmatter)) => {
                    tracing::debug!(skill = %metadata.name, "Discovered skill");

                    if self.config.validate_on_discovery {
                        let result = SkillValidator::validate(&path, &frontmatter);
                        if !result.is_valid() {
                            tracing::warn!(
                                skill = %metadata.name,
                                errors = result.errors.len(),
                                "Skill validation failed"
                            );
                        }
                        self.validation_results.push(result);
                    }

                    let name = metadata.name.clone();
                    self.skills.insert(name.clone(), metadata);
                    self.parsed_frontmatters.insert(name, frontmatter);
                    count += 1;
                }
                Err(e) => {
                    tracing::warn!(
                        path = %path.display(),
                        error = %e,
                        "Failed to parse skill"
                    );
                }
            }
        }

        tracing::info!(count, root, "Skill discovery complete");
        Ok(count)
    }

    /// Parse a skill's frontmatter and compute metadata.
    ///
    /// Attempts v1.0.0 parsing first, falls back to v0.2.0.
    async fn parse_skill_metadata(
        &self,
        skill_dir: &Path,
        skill_md_path: &Path,
    ) -> anyhow::Result<(SkillMetadata, UnifiedFrontmatter)> {
        let content = tokio::fs::read_to_string(skill_md_path).await?;
        let digest = if self.config.compute_digests {
            SkillMetadata::compute_digest(content.as_bytes())
        } else {
            String::new()
        };

        let frontmatter = Self::parse_unified_frontmatter(&content)?;

        let fm_repr = match &frontmatter {
            UnifiedFrontmatter::V02(fm) => UnifiedFrontmatterRepr::from_v02(fm),
            UnifiedFrontmatter::V10(fm) => UnifiedFrontmatterRepr::from_v10(fm),
        };

        let metadata = SkillMetadata {
            name: frontmatter.name().to_string(),
            description: frontmatter.description().to_string(),
            path: skill_dir.to_path_buf(),
            digest,
            frontmatter: fm_repr,
        };

        Ok((metadata, frontmatter))
    }

    /// Parse unified frontmatter from SKILL.md content.
    ///
    /// First tries v1.0.0 (requires `spec_version`), then falls back to v0.2.0.
    pub fn parse_unified_frontmatter(content: &str) -> anyhow::Result<UnifiedFrontmatter> {
        let yaml_str = Self::extract_yaml(content)?;

        // Try v1.0.0 first (look for spec_version field)
        let yaml_value: serde_yaml::Value = serde_yaml::from_str(yaml_str)
            .map_err(|e| anyhow::anyhow!("YAML parse error: {}", e))?;

        let has_spec_version = yaml_value
            .get("spec_version")
            .is_some();

        if has_spec_version {
            let vn: VendorNeutralFrontmatter = serde_yaml::from_value(yaml_value)
                .map_err(|e| anyhow::anyhow!("v1.0.0 frontmatter parse error: {}", e))?;
            Ok(UnifiedFrontmatter::V10(vn))
        } else {
            // Fall back to v0.2.0
            let yaml_str_fresh = Self::extract_yaml(content)?;
            let fm: SkillFrontmatter = serde_yaml::from_str(yaml_str_fresh)
                .map_err(|e| anyhow::anyhow!("v0.2.0 frontmatter parse error: {}", e))?;
            Ok(UnifiedFrontmatter::V02(fm))
        }
    }

    /// Extract YAML string from between --- delimiters.
    fn extract_yaml(content: &str) -> anyhow::Result<&str> {
        let content = content.trim_start();
        if !content.starts_with("---") {
            return Err(anyhow::anyhow!("SKILL.md must start with YAML frontmatter (---)"));
        }

        let rest = &content[3..];
        let end = rest
            .find("\n---")
            .ok_or_else(|| anyhow::anyhow!("SKILL.md frontmatter must be closed with ---"))?;

        Ok(&rest[..end])
    }

    /// Parse v0.2.0 frontmatter from SKILL.md content (backward compat).
    pub fn parse_frontmatter(content: &str) -> anyhow::Result<SkillFrontmatter> {
        let yaml_str = Self::extract_yaml(content)?;
        let frontmatter: SkillFrontmatter = serde_yaml::from_str(yaml_str)?;
        Ok(frontmatter)
    }

    /// Load a skill's full instructions (Tier 2).
    pub async fn load_skill(&self, name: &str) -> anyhow::Result<Skill> {
        let metadata = self
            .skills
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("Skill not found: {}", name))?;

        let skill_md_path = metadata.path.join("SKILL.md");
        let content = tokio::fs::read_to_string(&skill_md_path).await?;

        // Extract body after frontmatter
        let instructions = Self::extract_body(&content)?;

        // Discover resources
        let resources = self.discover_resources(&metadata.path).await?;

        Ok(Skill {
            metadata: metadata.clone(),
            instructions,
            resources,
        })
    }

    /// Get the parsed unified frontmatter for a skill.
    pub fn get_frontmatter(&self, name: &str) -> Option<&UnifiedFrontmatter> {
        self.parsed_frontmatters.get(name)
    }

    /// Get the parsed v1.0.0 frontmatter for a skill (if v1.0.0).
    pub fn get_v10_frontmatter(&self, name: &str) -> Option<&VendorNeutralFrontmatter> {
        self.parsed_frontmatters.get(name).and_then(|fm| fm.as_v10())
    }

    /// Extract the markdown body (after frontmatter).
    fn extract_body(content: &str) -> anyhow::Result<String> {
        let content = content.trim_start();
        let rest = &content[3..]; // Skip opening ---
        let end = rest
            .find("\n---")
            .ok_or_else(|| anyhow::anyhow!("No closing --- found"))?;

        let body = &rest[end + 4..]; // Skip closing ---
        Ok(body.trim().to_string())
    }

    /// Discover resource files in a skill directory.
    async fn discover_resources(&self, skill_dir: &Path) -> anyhow::Result<SkillResources> {
        let mut resources = SkillResources::default();

        let scripts_dir = skill_dir.join("scripts");
        if scripts_dir.exists() {
            let mut entries = tokio::fs::read_dir(&scripts_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                if entry.path().is_file() {
                    resources.scripts.push(entry.path());
                }
            }
        }

        let refs_dir = skill_dir.join("references");
        if refs_dir.exists() {
            let mut entries = tokio::fs::read_dir(&refs_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                if entry.path().is_file() {
                    resources.references.push(entry.path());
                }
            }
        }

        let assets_dir = skill_dir.join("assets");
        if assets_dir.exists() {
            let mut entries = tokio::fs::read_dir(&assets_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                if entry.path().is_file() {
                    resources.assets.push(entry.path());
                }
            }
        }

        Ok(resources)
    }

    /// Get all discovered skill metadata.
    pub fn list_skills(&self) -> Vec<&SkillMetadata> {
        self.skills.values().collect()
    }

    /// Get metadata for a specific skill.
    pub fn get_skill(&self, name: &str) -> Option<&SkillMetadata> {
        self.skills.get(name)
    }

    /// Validate all discovered skills.
    pub fn validate_all(&self) -> Vec<ValidationResult> {
        self.skills
            .keys()
            .filter_map(|name| {
                let metadata = self.skills.get(name)?;
                let frontmatter = self.parsed_frontmatters.get(name)?;
                Some(SkillValidator::validate(&metadata.path, frontmatter))
            })
            .collect()
    }

    /// Get the validation results from the last discovery.
    pub fn validation_results(&self) -> &[ValidationResult] {
        &self.validation_results
    }

    /// Number of discovered skills.
    pub fn skill_count(&self) -> usize {
        self.skills.len()
    }

    /// Generate a discovery index from all discovered skills.
    pub fn generate_index(&self) -> SkillIndex {
        let mut index = SkillIndex::new();
        for metadata in self.skills.values() {
            index.add_skill(metadata);
        }
        index
    }

    /// Generate a v1.0.0 discovery index from all discovered skills.
    pub fn generate_index_v10(&self, registry_name: &str) -> SkillIndex {
        let mut index = SkillIndex::new_v10(registry_name);
        for (name, metadata) in &self.skills {
            if let Some(UnifiedFrontmatter::V10(vn)) = self.parsed_frontmatters.get(name) {
                index.add_skill_v10(metadata, vn);
            } else {
                // v0.2.0 skills get basic index entries
                index.add_skill(metadata);
            }
        }
        index
    }
}

// ═══════════════════════════════════════════════════════════════════
//  Tests
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── V0.2.0 Frontmatter Parsing Tests ─────────────────────

    #[test]
    fn parse_valid_frontmatter_v02() {
        let content = r#"---
name: test-skill
description: A test skill. Use when testing.
---
# Instructions
Do the thing."#;

        let fm = SkillRegistry::parse_frontmatter(content).unwrap();
        assert_eq!(fm.name, "test-skill");
        assert_eq!(fm.description, "A test skill. Use when testing.");
        assert!(fm.license.is_none());
    }

    #[test]
    fn parse_frontmatter_with_extensions() {
        let content = r#"---
name: aegis-durable-workflow
description: Creates durable workflows. Use when the user says "durable execution".
license: "MIT OR Apache-2.0"
compatibility: Requires Rust 1.82+
metadata:
  aegis-crate: "ferris-aegis-durable"
  aegis-phase: "5.1"
  version: "0.4.0"
  author: "ferris-aegis"
  tags: "durable checkpoint workflow"
allowed-tools: Bash(cargo:*) Read Write
---
# Instructions"#;

        let fm = SkillRegistry::parse_frontmatter(content).unwrap();
        assert_eq!(fm.name, "aegis-durable-workflow");
        assert_eq!(fm.aegis_crate(), Some("ferris-aegis-durable"));
        assert_eq!(fm.aegis_phase(), Some("5.1"));
        assert_eq!(fm.version(), Some("0.4.0"));
        assert_eq!(fm.author(), Some("ferris-aegis"));
        assert_eq!(fm.tags(), vec!["durable", "checkpoint", "workflow"]);
        assert_eq!(fm.allowed_tools_list(), vec!["Bash(cargo:*)", "Read", "Write"]);
    }

    // ── V1.0.0 Frontmatter Parsing Tests ─────────────────────

    #[test]
    fn parse_valid_frontmatter_v10() {
        let content = r#"---
spec_version: "1.0.0"
id: "skill:example:hello-world"
name: "hello-world"
version: "1.0.0"
description: "A minimal example skill. Use when greeting the user."
author: "example-org"
license: "MIT"
---
# Hello World
Say hello."#;

        let unified = SkillRegistry::parse_unified_frontmatter(content).unwrap();
        assert!(unified.is_vendor_neutral());
        assert_eq!(unified.name(), "hello-world");
        assert_eq!(unified.version(), "1.0.0");
        assert_eq!(unified.author(), "example-org");

        let vn = unified.as_v10().unwrap();
        assert_eq!(vn.id, "skill:example:hello-world");
        assert_eq!(vn.license, "MIT");
    }

    #[test]
    fn parse_frontmatter_auto_detects_v02() {
        let content = r#"---
name: test-skill
description: A test skill. Use when testing.
---
# Body"#;

        let unified = SkillRegistry::parse_unified_frontmatter(content).unwrap();
        assert!(!unified.is_vendor_neutral());
        assert!(unified.as_v02().is_some());
        assert!(unified.as_v10().is_none());
    }

    #[test]
    fn parse_frontmatter_auto_detects_v10() {
        let content = r#"---
spec_version: "1.0.0"
id: "skill:search:web-search"
name: "web-search"
version: "1.0.0"
description: "Searches the web. Use when the user needs information."
author: "example-org"
license: "MIT"
runtime: "mcp"
permissions:
  - "network.http.get"
---
# Web Search"#;

        let unified = SkillRegistry::parse_unified_frontmatter(content).unwrap();
        assert!(unified.is_vendor_neutral());

        let vn = unified.as_v10().unwrap();
        assert_eq!(vn.runtime, Some("mcp".to_string()));
        assert_eq!(vn.permissions, Some(vec!["network.http.get".to_string()]));
    }

    // ── SkillId Tests ────────────────────────────────────────

    #[test]
    fn skill_id_parse_valid() {
        let id = SkillId::parse("skill:research:research-planner").unwrap();
        assert_eq!(id.category, "research");
        assert_eq!(id.name, "research-planner");
        assert_eq!(id.to_string(), "skill:research:research-planner");
    }

    #[test]
    fn skill_id_parse_invalid_no_prefix() {
        let result = SkillId::parse("research:research-planner");
        assert!(result.is_err());
    }

    #[test]
    fn skill_id_parse_invalid_uppercase() {
        let result = SkillId::parse("skill:Research:Research-Planner");
        assert!(result.is_err());
    }

    // ── Capability Tests ─────────────────────────────────────

    #[test]
    fn capability_parse_valid() {
        let cap = Capability::parse("network.http.get").unwrap();
        assert_eq!(cap.domain, "network");
        assert_eq!(cap.operation, "http.get");
        assert_eq!(cap.to_string(), "network.http.get");
    }

    #[test]
    fn capability_parse_nested() {
        let cap = Capability::parse("filesystem.read.tmp").unwrap();
        assert_eq!(cap.domain, "filesystem");
        assert_eq!(cap.operation, "read.tmp");
    }

    #[test]
    fn capability_parse_no_dot() {
        let result = Capability::parse("network");
        assert!(result.is_err());
    }

    // ── TrustLevelRequired Tests ─────────────────────────────

    #[test]
    fn trust_level_ordering() {
        assert!(TrustLevelRequired::Unverified < TrustLevelRequired::Probationary);
        assert!(TrustLevelRequired::Probationary < TrustLevelRequired::Standard);
        assert!(TrustLevelRequired::Standard < TrustLevelRequired::Elevated);
        assert!(TrustLevelRequired::Elevated < TrustLevelRequired::Sovereign);
    }

    #[test]
    fn trust_level_min_scores() {
        assert_eq!(TrustLevelRequired::Unverified.min_score(), 0.00);
        assert_eq!(TrustLevelRequired::Probationary.min_score(), 0.20);
        assert_eq!(TrustLevelRequired::Standard.min_score(), 0.50);
        assert_eq!(TrustLevelRequired::Elevated.min_score(), 0.75);
        assert_eq!(TrustLevelRequired::Sovereign.min_score(), 0.95);
    }

    #[test]
    fn trust_level_from_str() {
        assert_eq!(
            TrustLevelRequired::from_str_opt("Standard").unwrap(),
            TrustLevelRequired::Standard
        );
        assert!(TrustLevelRequired::from_str_opt("Invalid").is_err());
    }

    // ── Validation Tests (V0.2.0) ────────────────────────────

    #[test]
    fn validate_valid_skill_v02() {
        let path = Path::new("/skills/aegis-trust-kernel");
        let fm = SkillFrontmatter {
            name: "aegis-trust-kernel".to_string(),
            description: "Manages trust scores. Use when the user mentions trust.".to_string(),
            license: None,
            compatibility: None,
            metadata: None,
            allowed_tools: None,
        };

        let result = SkillValidator::validate_v02(path, &fm);
        assert!(result.is_valid());
        assert!(result.errors.is_empty());
    }

    #[test]
    fn validate_name_mismatch() {
        let path = Path::new("/skills/wrong-name");
        let fm = SkillFrontmatter {
            name: "different-name".to_string(),
            description: "A skill. Use when testing.".to_string(),
            license: None,
            compatibility: None,
            metadata: None,
            allowed_tools: None,
        };

        let result = SkillValidator::validate_v02(path, &fm);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.rule == "name-matches-directory"));
    }

    // ── Validation Tests (V1.0.0) ────────────────────────────

    #[test]
    fn validate_valid_skill_v10() {
        let path = Path::new("/skills/web-search");
        let fm = VendorNeutralFrontmatter {
            spec_version: "1.0.0".to_string(),
            id: "skill:search:web-search".to_string(),
            name: "web-search".to_string(),
            version: "1.0.0".to_string(),
            description: "Searches the web. Use when the user needs information.".to_string(),
            author: "example-org".to_string(),
            license: "MIT".to_string(),
            runtime: Some("mcp".to_string()),
            platforms: None,
            entrypoint: None,
            timeout: None,
            permissions: Some(vec!["network.http.get".to_string()]),
            inputs: None,
            outputs: None,
            sandbox: None,
            required_context: None,
            optional_context: None,
            dependencies: None,
            validation: None,
            lifecycle: None,
            ferris_aegis: None,
            compatibility: None,
            metadata: None,
            allowed_tools: None,
        };

        let result = SkillValidator::validate_v10(path, &fm);
        assert!(result.is_valid());
        assert!(result.errors.is_empty());
    }

    #[test]
    fn validate_v10_bad_spec_version() {
        let path = Path::new("/skills/test");
        let fm = VendorNeutralFrontmatter {
            spec_version: "0.9.0".to_string(),
            id: "skill:test:test".to_string(),
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            description: "Test skill. Use when testing.".to_string(),
            author: "test".to_string(),
            license: "MIT".to_string(),
            runtime: None,
            platforms: None,
            entrypoint: None,
            timeout: None,
            permissions: None,
            inputs: None,
            outputs: None,
            sandbox: None,
            required_context: None,
            optional_context: None,
            dependencies: None,
            validation: None,
            lifecycle: None,
            ferris_aegis: None,
            compatibility: None,
            metadata: None,
            allowed_tools: None,
        };

        let result = SkillValidator::validate_v10(path, &fm);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.rule == "spec-version"));
    }

    #[test]
    fn validate_v10_bad_permission_format() {
        let path = Path::new("/skills/test");
        let fm = VendorNeutralFrontmatter {
            spec_version: "1.0.0".to_string(),
            id: "skill:test:test".to_string(),
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            description: "Test skill. Use when testing.".to_string(),
            author: "test".to_string(),
            license: "MIT".to_string(),
            runtime: None,
            platforms: None,
            entrypoint: None,
            timeout: None,
            permissions: Some(vec!["invalid-no-dot".to_string()]),
            inputs: None,
            outputs: None,
            sandbox: None,
            required_context: None,
            optional_context: None,
            dependencies: None,
            validation: None,
            lifecycle: None,
            ferris_aegis: None,
            compatibility: None,
            metadata: None,
            allowed_tools: None,
        };

        let result = SkillValidator::validate_v10(path, &fm);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.rule == "permission-format"));
    }

    // ── Body Extraction Tests ─────────────────────────────────

    #[test]
    fn extract_body_simple() {
        let content = "---\nname: test\ndescription: test\n---\n# Hello\n\nSome instructions.";
        let body = SkillRegistry::extract_body(content).unwrap();
        assert!(body.contains("# Hello"));
        assert!(body.contains("Some instructions."));
    }

    #[test]
    fn extract_body_no_body() {
        let content = "---\nname: test\ndescription: test\n---\n";
        let body = SkillRegistry::extract_body(content).unwrap();
        assert!(body.is_empty());
    }

    // ── Digest Tests ──────────────────────────────────────────

    #[test]
    fn digest_deterministic() {
        let content = b"test content for digest";
        let d1 = SkillMetadata::compute_digest(content);
        let d2 = SkillMetadata::compute_digest(content);
        assert_eq!(d1, d2);
    }

    #[test]
    fn digest_differs_on_content_change() {
        let d1 = SkillMetadata::compute_digest(b"content A");
        let d2 = SkillMetadata::compute_digest(b"content B");
        assert_ne!(d1, d2);
    }

    // ── SkillIndex Tests ──────────────────────────────────────

    #[test]
    fn skill_index_creation_v02() {
        let index = SkillIndex::new();
        assert_eq!(index.schema, "https://schemas.agentskills.io/discovery/0.2.0/schema.json");
        assert!(index.skills.is_empty());
    }

    #[test]
    fn skill_index_creation_v10() {
        let index = SkillIndex::new_v10("test-registry");
        assert_eq!(index.schema, "https://schemas.agentskills.io/discovery/1.0.0/schema.json");
        assert_eq!(index.spec_version, Some("1.0.0".to_string()));
        assert_eq!(index.registry, Some("test-registry".to_string()));
    }

    #[test]
    fn skill_index_add_skill() {
        let mut index = SkillIndex::new();
        let fm_repr = UnifiedFrontmatterRepr::from_v02(&SkillFrontmatter {
            name: "test-skill".to_string(),
            description: "A test skill".to_string(),
            license: None,
            compatibility: None,
            metadata: None,
            allowed_tools: None,
        });
        let metadata = SkillMetadata {
            name: "test-skill".to_string(),
            description: "A test skill".to_string(),
            path: PathBuf::from(".agents/skills/test-skill"),
            digest: "abc123".to_string(),
            frontmatter: fm_repr,
        };

        index.add_skill(&metadata);
        assert_eq!(index.skills.len(), 1);
        assert_eq!(index.skills[0].name, "test-skill");
        assert_eq!(index.skills[0].entry_type, "skill-md");
        assert_eq!(index.skills[0].url, "/.well-known/agent-skills/test-skill/SKILL.md");
        assert_eq!(index.skills[0].digest, "sha256:abc123");
    }

    #[test]
    fn skill_index_serialization() {
        let mut index = SkillIndex::new();
        let fm_repr = UnifiedFrontmatterRepr::from_v02(&SkillFrontmatter {
            name: "test-skill".to_string(),
            description: "A test skill".to_string(),
            license: None,
            compatibility: None,
            metadata: None,
            allowed_tools: None,
        });
        let metadata = SkillMetadata {
            name: "test-skill".to_string(),
            description: "A test skill".to_string(),
            path: PathBuf::from(".agents/skills/test-skill"),
            digest: "abc123".to_string(),
            frontmatter: fm_repr,
        };

        index.add_skill(&metadata);
        let json = index.to_json().unwrap();
        assert!(json.contains("\"$schema\""));
        assert!(json.contains("test-skill"));
    }

    // ── Frontmatter Extension Tests ───────────────────────────

    #[test]
    fn extension_fields_with_no_metadata() {
        let fm = SkillFrontmatter {
            name: "test".to_string(),
            description: "Test".to_string(),
            license: None,
            compatibility: None,
            metadata: None,
            allowed_tools: None,
        };

        assert!(fm.aegis_crate().is_none());
        assert!(fm.aegis_phase().is_none());
        assert!(fm.tags().is_empty());
        assert!(fm.allowed_tools_list().is_empty());
    }

    // ── Effective Trust Level Tests ──────────────────────────

    #[test]
    fn effective_trust_level_from_permissions() {
        let fm = VendorNeutralFrontmatter {
            spec_version: "1.0.0".to_string(),
            id: "skill:test:test".to_string(),
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            description: "Test. Use when testing.".to_string(),
            author: "test".to_string(),
            license: "MIT".to_string(),
            runtime: None,
            platforms: None,
            entrypoint: None,
            timeout: None,
            permissions: Some(vec!["filesystem.write.tmp".to_string()]),
            inputs: None,
            outputs: None,
            sandbox: None,
            required_context: None,
            optional_context: None,
            dependencies: None,
            validation: None,
            lifecycle: None,
            ferris_aegis: None,
            compatibility: None,
            metadata: None,
            allowed_tools: None,
        };

        // filesystem.write requires Elevated
        assert_eq!(fm.effective_trust_level(), TrustLevelRequired::Elevated);
    }

    #[test]
    fn effective_trust_level_explicit_override() {
        let fm = VendorNeutralFrontmatter {
            spec_version: "1.0.0".to_string(),
            id: "skill:test:test".to_string(),
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            description: "Test. Use when testing.".to_string(),
            author: "test".to_string(),
            license: "MIT".to_string(),
            runtime: None,
            platforms: None,
            entrypoint: None,
            timeout: None,
            permissions: Some(vec!["network.http.get".to_string()]),
            inputs: None,
            outputs: None,
            sandbox: None,
            required_context: None,
            optional_context: None,
            dependencies: None,
            validation: None,
            lifecycle: None,
            ferris_aegis: Some(FerrisAegisExtension {
                trust_level: Some("Sovereign".to_string()),
                policies: None,
                audit: None,
                signature: None,
                sandbox: None,
                crate_name: None,
                phase: None,
                invariants: None,
            }),
            compatibility: None,
            metadata: None,
            allowed_tools: None,
        };

        // Explicit Sovereign overrides computed Standard
        assert_eq!(fm.effective_trust_level(), TrustLevelRequired::Sovereign);
    }

    // ── SkillRegistry Tests ───────────────────────────────────

    #[test]
    fn registry_creation() {
        let registry = SkillRegistry::with_defaults();
        assert_eq!(registry.skill_count(), 0);
    }

    #[test]
    fn registry_config_default() {
        let config = SkillRegistryConfig::default();
        assert_eq!(config.skills_dir, PathBuf::from(".agents/skills"));
        assert!(config.validate_on_discovery);
        assert!(config.compute_digests);
    }

    // ── LifecycleState Tests ──────────────────────────────────

    #[test]
    fn lifecycle_state_default() {
        let fm = VendorNeutralFrontmatter {
            spec_version: "1.0.0".to_string(),
            id: "skill:test:test".to_string(),
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            description: "Test. Use when testing.".to_string(),
            author: "test".to_string(),
            license: "MIT".to_string(),
            runtime: None,
            platforms: None,
            entrypoint: None,
            timeout: None,
            permissions: None,
            inputs: None,
            outputs: None,
            sandbox: None,
            required_context: None,
            optional_context: None,
            dependencies: None,
            validation: None,
            lifecycle: None,
            ferris_aegis: None,
            compatibility: None,
            metadata: None,
            allowed_tools: None,
        };

        // Default lifecycle is Stable
        assert_eq!(fm.lifecycle_state(), LifecycleState::Stable);
    }
}
