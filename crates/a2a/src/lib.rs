//! Ferris Aegis A2A — Agent-to-Agent Protocol.
//!
//! This crate implements A2A (Agent-to-Agent) protocol support with
//! both branches:
//!
//! # Branch A — Standalone (AgentCard + A2A Server)
//!
//! Build the full A2A protocol stack: AgentCard discovery server,
//! task handler, and A2A client. Use when Ferris Aegis needs to be
//! discoverable by agents you don't control.
//!
//! # Branch B — Integrated (MCP Tool Exposure)
//!
//! Skip the AgentCard server. Expose Phase 4 capabilities (session
//! management, supervision, semantic memory) as MCP tools that the
//! TypeScript orchestrator calls through the existing MCP interface
//! from Phase 2. No new protocol surface, no AgentCard.
//!
//! # Path Convention
//!
//! The AgentCard is served at `/.well-known/agent-card.json` per the
//! A2A spec and RFC 8615. The older `/.well-known/agent.json` (no
//! hyphen, singular "agent") is a pre-1.0 path and must NOT be used.

pub mod agent_card;
pub mod branch_a;
pub mod branch_b;
pub mod task;

pub use agent_card::{AgentCard, AgentSkill, AgentCapabilities, AgentCardBuilder};
pub use task::{A2aTask, TaskState, TaskResult};
