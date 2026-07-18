# Verification Notes — Week 3 Observability Crate

## What was verified

The observability crate (`crates/observability`) was built following the
Phase 2 Week 3 specification. The following invariants are enforced:

### 1. stderr-only, enforced once

The subscriber is constructed with `with_writer(std::io::stderr)` in the
single `init()` function. No other code in this crate can emit to stdout.
This is critical for Phase 2 Week 4's MCP stdio server, which owns stdout
for the JSON-RPC protocol wire.

### 2. Metrics are defined once

`CoreMetrics` in `metrics.rs` is the only place a Prometheus metric name
is spelled out. The kernel and MCP crates receive a `&CoreMetrics` or
`CoreMetrics` clone and increment counters — they never define metric
names. This prevents metric-name drift across the codebase.

All metric names are prefixed with `ferris_aegis_` to avoid collisions
when running alongside other services in the same Prometheus scrape target.

### 3. No HTTP surface

`init()` returns a `Registry` via `ObservabilityHandle`. Mounting it at
`/metrics` is the gateway/CLI crate's responsibility. This crate never
binds a port or starts an HTTP server. This keeps it usable in contexts
that never want a port open — e.g. the stdio-only MCP server.

### 4. Batch export, not simple/sync

The OTel pipeline uses `install_batch(Tokio)` rather than simple export.
Per-span synchronous network hops would let tracing overhead dominate
agent latency under load.

### 5. TracerProvider lifetime

`ObservabilityHandle` holds `tracer_provider` for the process lifetime.
Dropping it early silently stops span export. The `shutdown()` method
must be called after the agent loop has drained, not before.

## What was NOT verified (requires compilation)

- `{ workspace = true }` dependency entries require an actual workspace
  root to resolve. These are inherited from the workspace Cargo.toml but
  cannot be tested standalone.
- The exact `opentelemetry_sdk` and `opentelemetry-otlp` API shapes
  at versions 0.27 and 0.27 respectively. The `TracerProvider::builder()`
  and `install_batch()` patterns are based on the documented API, but
  minor version drift could require compile-and-fix passes.
- `ProtocolVersion::V_2025_11_25` in the MCP crate depends on `rmcp 2.2+`.
  This constant is confirmed to exist in the rmcp source on GitHub but
  has not been compiled against.

## Integration steps completed

1. ✅ `crates/observability` added to `[workspace] members`
2. ✅ `crates/mcp` added to `[workspace] members`
3. ✅ `crates/kernel` added to `[workspace] members`
4. ✅ Root binary crate (`src/main.rs`) depends on all three crates
5. ✅ Integration tests import from workspace crates

## Dependency version notes

| Crate | Pinned Version | Notes |
|-------|---------------|-------|
| `tracing-opentelemetry` | 0.28 | Bridges tracing spans to OTel |
| `opentelemetry` | 0.27 | Core OTel types |
| `opentelemetry-otlp` | 0.27 | OTLP exporter (gRPC to collector) |
| `opentelemetry_sdk` | 0.27 | SDK with `rt-tokio` for batch export |
| `prometheus` | 0.13 | Metrics registry and exposition |
| `rmcp` | 2.2 | MCP SDK with `server`, `macros`, `transport-io` |
| `schemars` | 0.8 | JSON Schema generation for MCP tool params |

## Phase 2 Week 4b Gate

The decision to defer external interoperability (legacy fallback,
conformance suite, HTTP/SSE) is valid for Phase 2. This gate will be
revisited at Phase 4, where A2A and AgentCard force the same question.

**Gate question:** *"Will Ferris Aegis be consumed by tools we do not control?"*
**Current answer:** No — internal consumer only. Revisit at Phase 4.
