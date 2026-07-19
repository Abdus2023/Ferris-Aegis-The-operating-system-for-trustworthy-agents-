---
name: aegis-resilience-ops
description: >
  Operates Ferris Aegis resilience primitives: circuit breaker, retry with backoff,
  timeout enforcement, rate limiting, and health checks. Use when the user says
  "circuit breaker", "rate limit", "retry policy", "timeout", "health check", or
  "resilience pattern". Do NOT use for durable workflows or checkpoint operations.
license: "MIT OR Apache-2.0"
compatibility: Requires Rust 1.82+, ferris-aegis-resilience, ferris-aegis-kernel crates
metadata:
  aegis-crate: "ferris-aegis-resilience ferris-aegis-kernel"
  aegis-phase: "5"
  aegis-depends: "aegis-trust-kernel"
  aegis-invariants: "INV-011 INV-012"
  version: "0.4.0"
  author: "ferris-aegis"
  tags: "circuit-breaker retry timeout rate-limiter health-check resilience"
allowed-tools: Bash(cargo:*) Read Write
---

# Ferris Aegis — Resilience Operations

Operate circuit breakers, retry policies, timeouts, rate limiters, and health checks.

## When to Use

- Protecting downstream dependencies with circuit breakers
- Implementing retry with exponential backoff + jitter
- Enforcing operation deadlines with timeouts
- Rate limiting API calls with token bucket
- Checking aggregate system health

## Resilience Stack

```
┌─────────────────────────────┐
│   Circuit Breaker (gate)    │  ← Fail fast when unhealthy
├─────────────────────────────┤
│   Timeout (deadline)        │  ← Cancel slow operations
├─────────────────────────────┤
│   Retry + Backoff + Jitter  │  ← Recover from transient failures
├─────────────────────────────┤
│   Operation                 │  ← The actual work
└─────────────────────────────┘
```

Or use the composite: `execute_resilient()` which layers all three.

## Workflow

1. Choose resilience primitive(s) for your use case
2. Configure with appropriate thresholds
3. Wrap operations with resilience layers
4. Monitor state transitions and health

## Code Pattern — Circuit Breaker

```rust
use ferris_aegis_resilience::{CircuitBreaker, CircuitBreakerConfig, CircuitState};

let config = CircuitBreakerConfig {
    failure_threshold: 5,
    recovery_timeout_ms: 30_000,
    half_open_success_threshold: 2,
};
let mut cb = CircuitBreaker::new(config);

if cb.allow_request() {
    // Execute operation
    match operation().await {
        Ok(result) => cb.record_success(),
        Err(e) => cb.record_failure(),
    }
} else {
    // Fail fast — circuit is open
}
```

## Code Pattern — Retry with Backoff

```rust
use ferris_aegis_resilience::{RetryPolicy, RetryConfig};

let policy = RetryPolicy::new(RetryConfig {
    max_retries: 3,
    base_delay_ms: 100,
    max_delay_ms: 10_000,
    use_jitter: true,
});

let result = policy.execute("api-call", || async {
    // Operation that may fail transiently
    api_call().await.map_err(|e| e.to_string())
}).await;
```

## Code Pattern — Timeout

```rust
use ferris_aegis_resilience::with_timeout;
use std::time::Duration;

let result = with_timeout("slow-op", Duration::from_secs(5), async {
    potentially_slow_operation().await
}).await;
// Returns Err(TimeoutError) if deadline exceeded
```

## Code Pattern — Rate Limiter

```rust
use ferris_aegis_resilience::{RateLimiter, RateLimiterConfig};

let mut rl = RateLimiter::new(RateLimiterConfig {
    capacity: 100,
    refill_rate: 10.0,
    refill_interval_ms: 1000,
});

if rl.try_acquire() {
    // Process request
} else {
    // Rate limited — reject or queue
}
```

## Code Pattern — Health Check

```rust
use ferris_aegis_resilience::{HealthRegistry, HealthCheck, HealthStatus};

let mut registry = HealthRegistry::new();
registry.register(Box::new(my_component));
let status = registry.aggregate_status().await;
```

## Invariants

- **INV-011**: Circuit breaker trips before cascading failure (N consecutive failures → Open)
- **INV-012**: Rate limiter enforces token bucket (capacity + refill rate)

## Edge Cases

- Circuit breaker with 0ms recovery timeout → immediately half-open (useful for testing)
- Retry with `use_jitter: false` → deterministic delays (useful for testing, bad for production)
- Rate limiter tokens refill over time — capacity is not instantly restored
