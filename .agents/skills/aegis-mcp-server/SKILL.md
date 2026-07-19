---
name: aegis-mcp-server
description: >
  Builds and configures MCP (Model Context Protocol) servers in Ferris Aegis with
  security-instrumented tool handlers and observability. Use when the user says
  "MCP server", "file_read tool", "tool handler", "MCP protocol", "rmcp server",
  or "MCP stdio". Do NOT use for general security scanning or trust operations.
license: "MIT OR Apache-2.0"
compatibility: Requires Rust 1.82+, ferris-aegis-mcp, ferris-aegis-observability crates
metadata:
  aegis-crate: "ferris-aegis-mcp ferris-aegis-observability"
  aegis-phase: "2"
  aegis-depends: "aegis-security-pipeline aegis-trust-kernel"
  aegis-invariants: "INV-004 ADR-004 ADR-007"
  version: "0.4.0"
  author: "ferris-aegis"
  tags: "mcp server tool handler stdio protocol"
allowed-tools: Bash(cargo:*) Read Write
---

# Ferris Aegis — MCP Server

Build and configure MCP stdio servers with security-instrumented tool handlers.

## When to Use

- Creating MCP tool handlers for agent interactions
- Configuring the MCP stdio server
- Understanding MCP protocol version pinning
- Setting up observability for MCP tools

## Key Constraints

1. **Protocol version pinned to `V_2025_11_25`** — never `.LATEST` (ADR-004)
2. **All output to stderr only** — MCP owns stdout (INV-004, ADR-007)
3. **OTel tracing on every tool handler** — instrumented with spans + counters
4. **Prometheus metrics on every tool call** — `tool_ok()`, `tool_error()`

## Workflow

1. Initialize observability first (stderr logging + OTel + Prometheus)
2. Create MCP server with `ferris_aegis_mcp::serve(metrics)`
3. Define tool handlers with security checks
4. Instrument each handler with tracing spans and Prometheus counters

## Code Pattern — MCP Server

```rust
use ferris_aegis_observability;
use ferris_aegis_mcp;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let handle = ferris_aegis_observability::init().await?;
    let metrics = handle.metrics.clone();

    // MCP stdio server — all output to stderr
    ferris_aegis_mcp::serve(metrics).await?;

    handle.shutdown();
    Ok(())
}
```

## Code Pattern — File Read Security

```rust
// Absolute paths only, no directory traversal
let result = ferris_aegis_mcp::read_file_inner("/workspace/data.txt", 65536);
assert!(result.is_ok());

// Relative paths rejected
let result = ferris_aegis_mcp::read_file_inner("relative/path.txt", 65536);
assert!(result.is_err());

// Nonexistent paths rejected
let result = ferris_aegis_mcp::read_file_inner("/nonexistent", 65536);
assert!(result.is_err());
```

## Code Pattern — Observability Init

```rust
// Initialize observability — stderr only, batch OTel export
let handle = ferris_aegis_observability::init().await?;
handle.metrics.requests_total.inc();
handle.metrics.tool_ok("file_read");
handle.metrics.tool_error("file_read");
```

## Invariants

- **INV-004**: All observability writes to stderr. MCP owns stdout. Never mix.
- **ADR-004**: MCP protocol version `V_2025_11_25` explicitly pinned
- **ADR-007**: OTel uses `install_batch(Tokio)`, logging uses `with_writer(std::io::stderr)`

## Edge Cases

- MCP server blocks on stdin — run in dedicated task or process
- If observability init fails, MCP server should not start
- Tool handler errors are logged and counted, never crash the server
