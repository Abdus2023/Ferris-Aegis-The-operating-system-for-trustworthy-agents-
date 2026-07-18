//! Prometheus metric definitions.
//!
//! Metrics are defined and owned here, not in the kernel. The kernel
//! receives a `&CoreMetrics` and increments counters — it never
//! defines a metric name. This is the single choke point that
//! prevents metric-name drift across the codebase.

use prometheus::{Counter, CounterVec, Opts, Registry};

/// The three metrics that must never be removed: they answer
/// "how much traffic, how much spend, how much is failing" —
/// the 80% of production visibility that pays for itself first.
///
/// Prometheus counters and counter vectors are internally reference-counted
/// (they wrap `Arc<AtomicF64>` etc.), so cloning is cheap and shares the
/// same underlying metric. This is important: the MCP server receives a
/// clone of `CoreMetrics`, but both copies increment the same counters.
#[derive(Clone)]
pub struct CoreMetrics {
    /// Total agent requests handled, across all ReAct rounds.
    pub requests_total: Counter,

    /// Total tokens consumed across all provider calls. A cost proxy,
    /// not a billing figure — reconcile against the provider's own
    /// usage dashboard before treating this as authoritative.
    pub tokens_used_total: Counter,

    /// Tool calls, labeled by tool name and outcome ("ok" | "error").
    /// Cardinality is bounded by the number of registered tools,
    /// which is fixed at startup — safe for Prometheus.
    pub tool_calls_total: CounterVec,
}

impl CoreMetrics {
    /// Construct the core metrics and register them against `registry`.
    ///
    /// `registry` is owned by the caller. This crate never binds a
    /// port or mounts `/metrics` itself — see [crate-level docs](crate).
    pub fn new(registry: &Registry) -> anyhow::Result<Self> {
        let requests_total = Counter::with_opts(Opts::new(
            "ferris_aegis_requests_total",
            "Total number of agent requests handled",
        ))?;
        registry.register(Box::new(requests_total.clone()))?;

        let tokens_used_total = Counter::with_opts(Opts::new(
            "ferris_aegis_tokens_used_total",
            "Total tokens consumed across all provider calls",
        ))?;
        registry.register(Box::new(tokens_used_total.clone()))?;

        let tool_calls_total = CounterVec::new(
            Opts::new(
                "ferris_aegis_tool_calls_total",
                "Tool calls by tool name and outcome",
            ),
            &["tool", "outcome"],
        )?;
        registry.register(Box::new(tool_calls_total.clone()))?;

        Ok(Self {
            requests_total,
            tokens_used_total,
            tool_calls_total,
        })
    }

    /// Record a successful tool call.
    pub fn tool_ok(&self, tool_name: &str) {
        self.tool_calls_total
            .with_label_values(&[tool_name, "ok"])
            .inc();
    }

    /// Record a failed tool call.
    pub fn tool_error(&self, tool_name: &str) {
        self.tool_calls_total
            .with_label_values(&[tool_name, "error"])
            .inc();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_without_name_collisions() {
        let registry = Registry::new();
        let metrics = CoreMetrics::new(&registry).expect("registration must not fail");
        metrics.requests_total.inc();
        metrics
            .tool_calls_total
            .with_label_values(&["file_read", "ok"])
            .inc();

        // A second CoreMetrics on a *different* registry must also
        // succeed — this catches accidental use of the global default
        // registry instead of the one passed in.
        let registry2 = Registry::new();
        CoreMetrics::new(&registry2).expect("second registry must not collide with the first");
    }

    #[test]
    fn tool_helpers_increment_correctly() {
        let registry = Registry::new();
        let metrics = CoreMetrics::new(&registry).expect("registration must not fail");

        metrics.tool_ok("file_read");
        metrics.tool_ok("file_read");
        metrics.tool_error("file_read");

        let ok_count = metrics
            .tool_calls_total
            .with_label_values(&["file_read", "ok"])
            .get();
        let err_count = metrics
            .tool_calls_total
            .with_label_values(&["file_read", "error"])
            .get();

        assert_eq!(ok_count, 2.0);
        assert_eq!(err_count, 1.0);
    }
}
