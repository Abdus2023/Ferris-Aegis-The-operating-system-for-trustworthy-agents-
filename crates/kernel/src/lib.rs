//! # Ferris Aegis Kernel — The Rust Guardian for Autonomous Intelligence
//!
//! An operating system for trustworthy agents. Ferris Aegis provides a
//! comprehensive framework for building, running, and monitoring autonomous
//! AI agents with strong guarantees around safety, auditability, and policy
//! compliance.
//!
//! # Architecture
//!
//! The system is organized around six core pillars:
//!
//! - **Trust Kernel** ([`kernel`]) — Identity verification, trust scoring, and
//!   capability attestation for every agent in the system.
//! - **Agent Runtime** ([`agent`]) — Lifecycle management: spawn, suspend,
//!   resume, and terminate agents with full state tracking.
//! - **Policy Engine** ([`policy`]) — Declarative policy definition and
//!   enforcement. Policies govern what agents can see, do, and communicate.
//! - **Audit Ledger** ([`audit`]) — An append-only, cryptographically chained
//!   ledger recording every significant agent action for full accountability.
//! - **Sandbox** ([`sandbox`]) — Capability-based isolation boundaries that
//!   constrain agent execution environments.
//! - **Guard** ([`guard`]) — Real-time monitoring, anomaly detection, and
//!   intervention when agents deviate from expected behavior.

pub mod agent;
pub mod audit;
pub mod config;
pub mod guard;
pub mod kernel;
pub mod policy;
pub mod sandbox;

/// The version of Ferris Aegis
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The codename for this release
pub const CODENAME: &str = "AEGIS-PRIME";

/// Re-export of core types for convenience
pub mod prelude {
    pub use crate::agent::{Agent, AgentId, AgentRuntime, AgentState, AgentStatus};
    pub use crate::audit::{AuditEntry, AuditLedger};
    pub use crate::kernel::{TrustKernel, TrustLevel, TrustScore};
    pub use crate::policy::{Policy, PolicyEngine, PolicyVerdict};
    pub use crate::sandbox::{Capability, Sandbox, SandboxBoundary};
    pub use crate::guard::{Guard, GuardAlert, GuardAction};
}
