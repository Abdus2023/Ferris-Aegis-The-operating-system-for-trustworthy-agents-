//! Ferris Aegis MCP Server — Instrumented stdio MCP server.
//!
//! This crate exposes Ferris Aegis tools via the Model Context Protocol
//! (MCP) over a stdio transport. Every tool handler is instrumented with
//! OTel tracing spans and Prometheus metrics from birth.
//!
//! # Protocol Version
//!
//! Targets `V_2025_11_25` explicitly — the current stable MCP spec
//! revision. We bind the named constant, not `.LATEST`, so a future
//! `rmcp` release moving what `LATEST` points to doesn't silently
//! renegotiate our protocol version.
//!
//! # Scope
//!
//! This is the **Week 4 core** deliverable:
//! - Stdio transport only
//! - Single tool: `file_read`
//! - Fully instrumented (OTel spans + Prometheus counters)
//!
//! Explicitly **excluded**:
//! - HTTP/SSE transport
//! - Legacy version fallback (`V_2024_11_05`, `V_2025_03_26`, etc.)
//! - OAuth 2.1 authentication
//! - Resource or prompt surfaces
//! - Any client-side code
//!
//! # Stderr-Only Invariant
//!
//! MCP over stdio uses stdout for the protocol wire. All logging and
//! tracing output goes to stderr — enforced by the observability crate's
//! subscriber, which sets `with_writer(std::io::stderr)`. A single
//! `println!` or `print!` in this crate would corrupt the MCP wire
//! protocol. The `#[warn(clippy::print_stdout)]` lint is enabled to
//! catch this at compile time.

mod server;
mod tools;

pub use server::AegisMcpServer;
pub use tools::{FileReadParams, read_file_inner};

use anyhow::Context;
use ferris_aegis_observability::CoreMetrics;
use rmcp::ServiceExt;

/// Run the MCP server on stdio.
///
/// This is the primary entry point. It:
/// 1. Constructs the [`AegisMcpServer`] with the given metrics handle
/// 2. Binds the stdio transport
/// 3. Serves until the client disconnects or a shutdown signal is received
///
/// # Errors
///
/// Returns an error if:
/// - The stdio transport fails to initialize
/// - The server encounters a fatal protocol error
pub async fn serve(metrics: CoreMetrics) -> anyhow::Result<()> {
    let server = AegisMcpServer::new(metrics);

    let service = server
        .serve(rmcp::transport::stdio())
        .await
        .context("failed to start MCP stdio server")?;

    tracing::info!("Ferris Aegis MCP server started on stdio transport");

    service.waiting().await.context("MCP server error")?;

    tracing::info!("Ferris Aegis MCP server shut down gracefully");
    Ok(())
}
