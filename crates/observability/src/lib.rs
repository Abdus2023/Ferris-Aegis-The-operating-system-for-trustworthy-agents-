//! Ferris Aegis Observability — OTel tracing, Prometheus metrics, JSON stderr logging.
//!
//! This crate has **zero dependency on `kernel` or `cli`.** It is pure
//! infrastructure: it builds and tests before a single line of agent
//! code exists. The kernel depends on this crate, never the reverse.
//!
//! # Three Invariants
//!
//! 1. **stderr-only, enforced once, here.** The subscriber is built
//!    with `with_writer(std::io::stderr)` at the one place a
//!    subscriber is constructed. Nothing in this crate can emit to
//!    stdout — which is what Phase 2's MCP stdio transport needs to
//!    stay true for the whole binary.
//!
//! 2. **Metrics are defined once, here.** [`CoreMetrics`] is the only
//!    place a Prometheus metric name is spelled out.
//!
//! 3. **No HTTP surface.** [`init`] hands back a [`Registry`]; mounting
//!    it at `/metrics` is the gateway crate's job. Keeping the
//!    bind-a-port decision out of this crate keeps it usable in
//!    contexts that never want a port open — e.g. the stdio-only MCP
//!    server from Week 4 core.

mod metrics;

pub use metrics::CoreMetrics;

use anyhow::Context;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::trace::TracerProvider;
use prometheus::Registry;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Everything [`init`] hands back to the caller.
///
/// Holding `tracer_provider` alive for the life of the process is
/// required — dropping it early silently stops span export with no
/// error, which is exactly the kind of blind failure Phase 2 exists
/// to rule out.
pub struct ObservabilityHandle {
    /// The Prometheus registry. Hand this to the gateway crate to mount
    /// at `/metrics` — this crate never binds a port itself.
    pub registry: Registry,

    /// The registered core counters, passed by reference into the
    /// kernel so call sites can increment them without redefining them.
    pub metrics: CoreMetrics,

    tracer_provider: TracerProvider,
}

impl ObservabilityHandle {
    /// Flush and shut down the OTel pipeline.
    ///
    /// Call this on the shutdown path in the CLI, *after* the agent
    /// loop has drained — shutting down before the last round completes
    /// drops that round's spans ungracefully instead of exporting them.
    pub fn shutdown(self) {
        let _ = self.tracer_provider.shutdown();
    }
}

/// Initialize tracing, OTel export, and Prometheus metrics.
///
/// Must be called exactly once, before any agent code runs, and
/// before the MCP server binds its stdio transport in Phase 2 Week 4.
///
/// # Errors
///
/// Returns an error if:
/// - The OTLP pipeline fails to install (e.g. the collector is unreachable
///   and the gRPC connection cannot be established)
/// - The tracing subscriber is already initialized (double-init guard)
/// - Core metric registration fails (should never happen with a fresh registry)
pub async fn init() -> anyhow::Result<ObservabilityHandle> {
    // JSON structured logging for machine parsing (Datadog, Loki, etc.)
    // NEVER stdout: Phase 2's MCP stdio transport owns that stream.
    let json_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_target(true)
        .with_thread_ids(true)
        .with_writer(std::io::stderr);

    // OTel tracer — local Jaeger in dev, OTLP collector in prod.
    let endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:4317".into());

    let otlp_exporter = opentelemetry_otlp::new_exporter()
        .tonic()
        .with_endpoint(endpoint);

    // Batch export, not simple/sync export — a per-span network hop
    // would let tracing overhead dominate agent latency under load.
    let tracer_provider = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(otlp_exporter)
        .install_batch(opentelemetry_sdk::runtime::Tokio)
        .map_err(|e| anyhow::anyhow!("failed to install OTLP batch pipeline: {e}"))?;

    let tracer = tracer_provider.tracer("ferris-aegis");

    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,ferris_aegis=debug"));

    tracing_subscriber::registry()
        .with(filter)
        .with(json_layer)
        .with(otel_layer)
        .try_init()
        .context(
            "tracing subscriber already initialized — init() must be called exactly once",
        )?;

    let registry = Registry::new();
    let metrics = CoreMetrics::new(&registry).context("failed to register core metrics")?;

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        "Ferris Aegis observability initialized"
    );

    Ok(ObservabilityHandle {
        registry,
        metrics,
        tracer_provider,
    })
}

/// Initialize observability for testing without a real OTel collector.
///
/// This sets up JSON stderr logging and Prometheus metrics, but skips
/// the OTel exporter — no network calls, no collector needed. Use this
/// in unit tests and integration tests where tracing spans just need
/// to be captured by the subscriber, not exported.
pub fn init_test() -> anyhow::Result<ObservabilityHandle> {
    let json_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_target(true)
        .with_writer(std::io::stderr);

    let filter = tracing_subscriber::EnvFilter::new("debug");

    // Try to init — if already initialized (e.g. another test ran first),
    // that's fine, we just won't get a second subscriber.
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(json_layer)
        .try_init();

    // Create a minimal tracer provider with no exporters for testing
    let tracer_provider = TracerProvider::builder().build();

    let registry = Registry::new();
    let metrics = CoreMetrics::new(&registry).context("failed to register core metrics")?;

    Ok(ObservabilityHandle {
        registry,
        metrics,
        tracer_provider,
    })
}
