//! MCP server entry point and lifecycle management.
//!
//! This module provides the binary entry point for running the MCP
//! server, and is also used by the library's `serve()` function.

use ferris_aegis_observability::CoreMetrics;

use crate::AegisMcpServer;

/// Run the MCP server with the given metrics handle and stdio transport.
///
/// This is the main loop — it blocks until the client disconnects
/// or a fatal error occurs.
pub async fn run_server(metrics: CoreMetrics) -> anyhow::Result<()> {
    let server = AegisMcpServer::new(metrics);

    let service = server
        .serve(rmcp::transport::stdio())
        .await
        .map_err(|e| anyhow::anyhow!("MCP stdio server failed to start: {e}"))?;

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        "Ferris Aegis MCP server listening on stdio"
    );

    service.waiting().await.map_err(|e| {
        anyhow::anyhow!("MCP server error during operation: {e}")
    })?;

    tracing::info!("Ferris Aegis MCP server shut down");
    Ok(())
}
