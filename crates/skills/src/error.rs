use thiserror::Error;

/// Errors that can occur during skill parsing, validation, or execution.
#[derive(Error, Debug)]
pub enum SkillError {
    #[error("Skill not found: {0}")]
    NotFound(String),

    #[error("Failed to parse frontmatter: {0}")]
    FrontmatterParseError(String),

    #[error("Invalid skill ID: {0} (expected: skill:<category>:<name>)")]
    InvalidSkillId(String),

    #[error("Skill ID mismatch: expected {expected}, got {got}")]
    SkillIdMismatch { expected: String, got: String },

    #[error("Missing required field: {0}")]
    MissingRequiredField(String),

    #[error("Invalid version constraint: {0}")]
    InvalidVersionConstraint(String),

    #[error("Version mismatch: expected {constraint}, got {actual}")]
    VersionMismatch { constraint: String, actual: String },

    #[error("Circular dependency detected: {0}")]
    CircularDependency(String),

    #[error("Dependency not found: {skill} (version {version})")]
    DependencyNotFound { skill: String, version: String },

    #[error("System tool not available: {tool} (version {version})")]
    SystemToolMissing { tool: String, version: String },

    #[error("Trust level insufficient: required {required}, got {actual}")]
    TrustLevelInsufficient { required: String, actual: String },

    #[error("Capability denied: {0}")]
    CapabilityDenied(String),

    #[error("Policy violation: {policy_id}")]
    PolicyViolation { policy_id: String },

    #[error("Path traversal attempt: {0}")]
    PathTraversal(String),

    #[error("Resource limit exceeded: {resource} ({actual} > {limit})")]
    ResourceLimitExceeded { resource: String, actual: String, limit: String },

    #[error("Execution timeout")]
    ExecutionTimeout,

    #[error("Invalid signature")]
    SignatureVerificationFailed,

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("YAML error: {0}")]
    YamlError(String),

    #[error("Skill execution failed: {0}")]
    ExecutionError(String),

    #[error("Skill validation failed: {0}")]
    ValidationError(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<yaml_rust2::ScanError> for SkillError {
    fn from(e: yaml_rust2::ScanError) -> Self {
        SkillError::YamlError(e.to_string())
    }
}

impl From<yaml_rust2::EmitError> for SkillError {
    fn from(e: yaml_rust2::EmitError) -> Self {
        SkillError::YamlError(e.to_string())
    }
}

pub type SkillResult<T> = Result<T, SkillError>;
