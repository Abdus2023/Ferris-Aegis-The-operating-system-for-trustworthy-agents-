//! Ferris Aegis Skills — Agent Skill discovery, parsing, validation, and loading.
//!
//! This crate provides programmatic access to the SKILL.md ecosystem for
//! Ferris Aegis. It implements the [Agent Skills Open Specification](https://agentskills.io)
//! v0.2.0, enabling:
//!
//! - **Discovery** — Scan directories for SKILL.md files, parse frontmatter
//! - **Validation** — Validate skill names, descriptions, directory structure
//! - **Loading** — Load skill metadata (Tier 1) and instructions (Tier 2)
//! - **Hashing** — Compute content digests for integrity verification
//! - **Indexing** — Generate `.well-known/agent-skills/index.json` manifests
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

// ── Frontmatter Types ────────────────────────────────────────────

/// YAML frontmatter of a SKILL.md file.
///
/// Complies with the agentskills.io v0.2.0 specification.
/// Only `name` and `description` are required; all other fields are optional.
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

// ── Validation ───────────────────────────────────────────────────

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
    /// Validate a skill directory.
    ///
    /// Checks:
    /// 1. SKILL.md file exists
    /// 2. Frontmatter is valid YAML with required fields
    /// 3. `name` matches directory name
    /// 4. `name` follows naming convention
    /// 5. `description` is within length limits
    /// 6. No angle brackets in frontmatter (injection risk)
    pub fn validate(path: &Path, frontmatter: &SkillFrontmatter) -> ValidationResult {
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
                    "Name '{}' must be lowercase alphanumeric with hyphens (^[a-z0-9]+(-[a-z0-9]+)*$)",
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

        // Rule 7: No angle brackets in frontmatter (injection risk)
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
}

// ── Skill Metadata (Tier 1) ──────────────────────────────────────

/// Tier 1: Skill metadata for discovery.
///
/// This is what agents load at startup (~100 tokens per skill).
/// Contains only the name and description from frontmatter.
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
    /// The full frontmatter (for detailed inspection).
    pub frontmatter: SkillFrontmatter,
}

impl SkillMetadata {
    /// Compute the SHA-256 digest of the SKILL.md file content.
    pub fn compute_digest(content: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content);
        hex::encode(hasher.finalize())
    }
}

// ── Skill Instructions (Tier 2) ──────────────────────────────────

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

// ── Discovery Index ──────────────────────────────────────────────

/// A skill entry in the discovery index.
///
/// Complies with the agentskills.io v0.2.0 discovery schema.
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
}

/// The discovery index manifest.
///
/// Complies with the agentskills.io v0.2.0 discovery schema.
/// Served at `/.well-known/agent-skills/index.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillIndex {
    /// JSON Schema URL.
    #[serde(rename = "$schema")]
    pub schema: String,
    /// List of available skills.
    pub skills: Vec<SkillIndexEntry>,
}

impl SkillIndex {
    /// Create a new empty discovery index.
    pub fn new() -> Self {
        Self {
            schema: "https://schemas.agentskills.io/discovery/0.2.0/schema.json".to_string(),
            skills: Vec::new(),
        }
    }

    /// Add a skill to the index.
    pub fn add_skill(&mut self, metadata: &SkillMetadata) {
        let entry = SkillIndexEntry {
            name: metadata.name.clone(),
            entry_type: "skill-md".to_string(),
            description: metadata.description.chars().take(1024).collect(),
            url: format!("/.well-known/agent-skills/{}/SKILL.md", metadata.name),
            digest: format!("sha256:{}", metadata.digest),
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

// ── Skill Registry ───────────────────────────────────────────────

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
pub struct SkillRegistry {
    config: SkillRegistryConfig,
    skills: HashMap<String, SkillMetadata>,
    validation_results: Vec<ValidationResult>,
}

impl SkillRegistry {
    /// Create a new skill registry with the given configuration.
    pub fn new(config: SkillRegistryConfig) -> Self {
        Self {
            config,
            skills: HashMap::new(),
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
                Ok(metadata) => {
                    tracing::debug!(skill = %metadata.name, "Discovered skill");

                    if self.config.validate_on_discovery {
                        let result = SkillValidator::validate(&path, &metadata.frontmatter);
                        if !result.is_valid() {
                            tracing::warn!(
                                skill = %metadata.name,
                                errors = result.errors.len(),
                                "Skill validation failed"
                            );
                        }
                        self.validation_results.push(result);
                    }

                    self.skills.insert(metadata.name.clone(), metadata);
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
    async fn parse_skill_metadata(
        &self,
        skill_dir: &Path,
        skill_md_path: &Path,
    ) -> anyhow::Result<SkillMetadata> {
        let content = tokio::fs::read_to_string(skill_md_path).await?;
        let digest = if self.config.compute_digests {
            SkillMetadata::compute_digest(content.as_bytes())
        } else {
            String::new()
        };

        let frontmatter = Self::parse_frontmatter(&content)?;

        Ok(SkillMetadata {
            name: frontmatter.name.clone(),
            description: frontmatter.description.clone(),
            path: skill_dir.to_path_buf(),
            digest,
            frontmatter,
        })
    }

    /// Parse YAML frontmatter from SKILL.md content.
    pub fn parse_frontmatter(content: &str) -> anyhow::Result<SkillFrontmatter> {
        // Extract content between --- delimiters
        let content = content.trim_start();
        if !content.starts_with("---") {
            return Err(anyhow::anyhow!("SKILL.md must start with YAML frontmatter (---)"));
        }

        let rest = &content[3..];
        let end = rest
            .find("\n---")
            .ok_or_else(|| anyhow::anyhow!("SKILL.md frontmatter must be closed with ---"))?;

        let yaml_str = &rest[..end];
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
            .values()
            .map(|m| SkillValidator::validate(&m.path, &m.frontmatter))
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
}

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Frontmatter Parsing Tests ─────────────────────────────

    #[test]
    fn parse_valid_frontmatter() {
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

    #[test]
    fn parse_frontmatter_missing_opening_delimiter() {
        let content = "name: test\ndescription: test\n---\n# Body";
        let result = SkillRegistry::parse_frontmatter(content);
        assert!(result.is_err());
    }

    #[test]
    fn parse_frontmatter_missing_closing_delimiter() {
        let content = "---\nname: test\ndescription: test\n# No closing delimiter";
        let result = SkillRegistry::parse_frontmatter(content);
        assert!(result.is_err());
    }

    #[test]
    fn parse_frontmatter_missing_name() {
        let content = "---\ndescription: test only\n---\n# Body";
        let result = SkillRegistry::parse_frontmatter(content);
        assert!(result.is_err());
    }

    #[test]
    fn parse_frontmatter_missing_description() {
        let content = "---\nname: test\n---\n# Body";
        let result = SkillRegistry::parse_frontmatter(content);
        assert!(result.is_err());
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

    // ── Validation Tests ──────────────────────────────────────

    #[test]
    fn validate_valid_skill() {
        let path = Path::new("/skills/aegis-trust-kernel");
        let fm = SkillFrontmatter {
            name: "aegis-trust-kernel".to_string(),
            description: "Manages trust scores. Use when the user mentions trust.".to_string(),
            license: None,
            compatibility: None,
            metadata: None,
            allowed_tools: None,
        };

        let result = SkillValidator::validate(path, &fm);
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

        let result = SkillValidator::validate(path, &fm);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.rule == "name-matches-directory"));
    }

    #[test]
    fn validate_name_invalid_format() {
        let path = Path::new("/skills/INVALID");
        let fm = SkillFrontmatter {
            name: "INVALID".to_string(),
            description: "A skill. Use when testing.".to_string(),
            license: None,
            compatibility: None,
            metadata: None,
            allowed_tools: None,
        };

        let result = SkillValidator::validate(path, &fm);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.rule == "name-format"));
    }

    #[test]
    fn validate_name_too_long() {
        let long_name = "a".repeat(65);
        let path = Path::new(&format!("/skills/{}", long_name));
        let fm = SkillFrontmatter {
            name: long_name,
            description: "A skill. Use when testing.".to_string(),
            license: None,
            compatibility: None,
            metadata: None,
            allowed_tools: None,
        };

        let result = SkillValidator::validate(path, &fm);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.rule == "name-max-length"));
    }

    #[test]
    fn validate_description_too_long() {
        let long_desc = "x".repeat(1025);
        let path = Path::new("/skills/test-skill");
        let fm = SkillFrontmatter {
            name: "test-skill".to_string(),
            description: long_desc,
            license: None,
            compatibility: None,
            metadata: None,
            allowed_tools: None,
        };

        let result = SkillValidator::validate(path, &fm);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.rule == "description-max-length"));
    }

    #[test]
    fn validate_name_consecutive_hyphens() {
        let path = Path::new("/skills/test--skill");
        let fm = SkillFrontmatter {
            name: "test--skill".to_string(),
            description: "A skill. Use when testing.".to_string(),
            license: None,
            compatibility: None,
            metadata: None,
            allowed_tools: None,
        };

        let result = SkillValidator::validate(path, &fm);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.rule == "name-consecutive-hyphens"));
    }

    #[test]
    fn validate_name_leading_hyphen() {
        let path = Path::new("/skills/-test");
        let fm = SkillFrontmatter {
            name: "-test".to_string(),
            description: "A skill. Use when testing.".to_string(),
            license: None,
            compatibility: None,
            metadata: None,
            allowed_tools: None,
        };

        let result = SkillValidator::validate(path, &fm);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.rule == "name-hyphen-bounds"));
    }

    #[test]
    fn validate_warning_missing_use_when() {
        let path = Path::new("/skills/test-skill");
        let fm = SkillFrontmatter {
            name: "test-skill".to_string(),
            description: "A skill that does things.".to_string(), // No "Use when"
            license: None,
            compatibility: None,
            metadata: None,
            allowed_tools: None,
        };

        let result = SkillValidator::validate(path, &fm);
        assert!(result.is_valid()); // Still valid, just warned
        assert!(!result.warnings.is_empty());
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
    fn skill_index_creation() {
        let index = SkillIndex::new();
        assert_eq!(index.schema, "https://schemas.agentskills.io/discovery/0.2.0/schema.json");
        assert!(index.skills.is_empty());
    }

    #[test]
    fn skill_index_add_skill() {
        let mut index = SkillIndex::new();
        let metadata = SkillMetadata {
            name: "test-skill".to_string(),
            description: "A test skill".to_string(),
            path: PathBuf::from(".agents/skills/test-skill"),
            digest: "abc123".to_string(),
            frontmatter: SkillFrontmatter {
                name: "test-skill".to_string(),
                description: "A test skill".to_string(),
                license: None,
                compatibility: None,
                metadata: None,
                allowed_tools: None,
            },
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
        let metadata = SkillMetadata {
            name: "test-skill".to_string(),
            description: "A test skill".to_string(),
            path: PathBuf::from(".agents/skills/test-skill"),
            digest: "abc123".to_string(),
            frontmatter: SkillFrontmatter {
                name: "test-skill".to_string(),
                description: "A test skill".to_string(),
                license: None,
                compatibility: None,
                metadata: None,
                allowed_tools: None,
            },
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
}
