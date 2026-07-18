//! # MCP Stdio Server Example
//!
//! Demonstrates running the Ferris Aegis MCP server over stdio.
//! This server can be connected to by any MCP client (Claude Desktop,
//! custom orchestrator, test harness) via the stdio transport.
//!
//! # Usage
//!
//! ```bash
//! cargo run --example mcp-stdio
//! ```
//!
//! Then pipe JSON-RPC messages to stdin, or connect via an MCP client.

use ferris_aegis_mcp;
use ferris_aegis_observability;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize observability — stderr only, MCP owns stdout
    let handle = ferris_aegis_observability::init().await?;
    tracing::info!("Starting Ferris Aegis MCP stdio server");

    let metrics = handle.metrics.clone();

    // Serve the MCP server on stdio
    ferris_aegis_mcp::serve(metrics).await?;

    handle.shutdown();
    Ok(())
}
