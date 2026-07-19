//! # Ferris Aegis Skills — SKILL.md Loader, Validator, and Executor
//!
//! This crate implements the SKILL.md specification: parsing frontmatter,
//! validating dependencies, enforcing policies, and executing skills
//! within Aegis sandbox boundaries.
//!
//! # Architecture
//!
//! - **Loader** ([`loader`]) — Parse SKILL.md files and frontmatter
//! - **Validator** ([`validator`]) — Static + runtime validation
//! - **Executor** ([`executor`]) — Execute skills in sandbox with audit
//! - **Registry** ([`registry`]) — Skill discovery and caching
//! - **Types** ([`types`]) — Core skill data structures
//! - **MCP Export** ([`mcp_export`]) — Auto-generate MCP tools from SKILL.md

pub mod error;
pub mod executor;
pub mod loader;
pub mod mcp_export;
pub mod registry;
pub mod types;
pub mod validator;

pub use executor::{ExecutionContext, SkillExecutor, SkillExecutionResult, ExecutionStatus, SkillImpl};
pub use loader::{SkillLoader, FrontmatterParser};
pub use mcp_export::{McpToolDefinition, McpToolGenerator, SkillMcpHandler, SkillInputSpec, SkillOutputSpec};
pub use registry::SkillRegistry;
pub use types::{Skill, SkillId, Capability, TrustLevelRequired, SkillMetadata, SkillExecutionContext, SkillInputSpec, SkillOutputSpec};
pub use validator::{SkillValidator, DependencyResolver};
