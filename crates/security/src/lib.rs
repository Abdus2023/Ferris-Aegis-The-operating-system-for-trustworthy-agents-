//! Ferris Aegis Security — Policy engine, credential vault, injection scanner, SSRF guard.
//!
//! This crate provides the security layer that sits between the agent runtime
//! and the outside world. Every tool call passes through the allowlist check,
//! every URL is validated by the SSRF guard, and every LLM prompt is scanned
//! for injection patterns.
//!
//! # The Credential Vault Invariant
//!
//! The vault's core claim — "the types make it impossible" for a credential
//! to reach LLM context — is enforced structurally, not by convention:
//!
//! - [`AuthenticatedCall`] carries the LLM-visible call and the injected
//!   credential as **separate fields**. The `call` field is safe to trace
//!   freely; the `credential` field structurally cannot be (nothing on
//!   `Secret<String>` implements `Debug` or `Serialize` in a way that
//!   prints the contents).
//!
//! - The tool executor is the **only** place that ever calls
//!   `.expose_secret()` — at the point of actual use (e.g. building an
//!   HTTP header), never before.
//!
//! - Tool-call tracing spans are populated from `call.arguments` only —
//!   never from a post-injection copy that could carry `credential`.

pub mod allowlist;
pub mod injection;
pub mod ssrf;
pub mod vault;

pub use allowlist::{ToolAllowlist, AllowlistVerdict};
pub use injection::{InjectionScanner, InjectionVerdict};
pub use ssrf::{SsrfGuard, SsrfVerdict};
pub use vault::{CredentialVault, AuthenticatedCall, StoredCredential, ProtectedSecret};
