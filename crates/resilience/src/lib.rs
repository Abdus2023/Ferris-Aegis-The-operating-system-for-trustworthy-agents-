//! Ferris Aegis Resilience — Production reliability primitives.
//!
//! This crate provides the foundational reliability patterns needed for
//! production-grade agent operations:
//!
//! - **CircuitBreaker** — Prevents cascading failures by detecting when a
//!   downstream dependency is unhealthy and failing fast instead of piling
//!   on retries.
//!
//! - **Retry** — Exponential backoff with jitter for transient failures.
//!   Never retries without a backoff, never retries forever.
//!
//! - **Timeout** — Every async operation has a deadline. Operations that
//!   exceed their deadline are cancelled deterministically.
//!
//! - **RateLimiter** — Token bucket algorithm. Protects downstream services
//!   from being overwhelmed and provides predictable throughput.
//!
//! - **HealthCheck** — Trait + registry for component health. Every
//!   subsystem can report its health status, and the system can query
//!   aggregate health.
//!
//! # Design Principles
//!
//! 1. **Fail fast, recover gracefully** — Circuits open quickly on failure
//!    and test recovery with controlled half-open probes.
//! 2. **Jitter everywhere** — Exponential backoff always includes random
//!    jitter to avoid thundering herd problems.
//! 3. **Observable** — Every state transition, retry attempt, and rate
//!    limit event is traced.
//! 4. **Composable** — Primitives are independent and can be layered
//!    (e.g. retry inside a circuit breaker with a timeout wrapper).

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

// ── Circuit Breaker ────────────────────────────────────────────────

/// The state of a circuit breaker.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CircuitState {
    /// The circuit is closed — requests flow normally.
    Closed,
    /// The circuit is open — requests fail immediately.
    Open,
    /// The circuit is half-open — a single probe request is allowed.
    HalfOpen,
}

impl std::fmt::Display for CircuitState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CircuitState::Closed => write!(f, "closed"),
            CircuitState::Open => write!(f, "open"),
            CircuitState::HalfOpen => write!(f, "half-open"),
        }
    }
}

/// Configuration for a circuit breaker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    /// Number of consecutive failures to trip the circuit.
    pub failure_threshold: u32,
    /// Duration to wait before testing recovery (half-open).
    pub recovery_timeout_ms: u64,
    /// Number of successful probes in half-open needed to close.
    pub half_open_success_threshold: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            recovery_timeout_ms: 30_000, // 30 seconds
            half_open_success_threshold: 2,
        }
    }
}

/// A circuit breaker that prevents cascading failures.
///
/// Tracks failure counts and transitions between closed, open, and
/// half-open states to protect downstream dependencies.
pub struct CircuitBreaker {
    /// The circuit breaker configuration.
    config: CircuitBreakerConfig,
    /// The current state of the circuit.
    state: CircuitState,
    /// Consecutive failure count.
    failure_count: u32,
    /// Consecutive success count in half-open state.
    half_open_successes: u32,
    /// When the circuit last tripped to open.
    last_failure_time: Option<DateTime<Utc>>,
    /// Total successful calls (for metrics).
    total_successes: u64,
    /// Total failed calls (for metrics).
    total_failures: u64,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with the given configuration.
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: CircuitState::Closed,
            failure_count: 0,
            half_open_successes: 0,
            last_failure_time: None,
            total_successes: 0,
            total_failures: 0,
        }
    }

    /// Create a circuit breaker with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(CircuitBreakerConfig::default())
    }

    /// Check if a request should be allowed through.
    ///
    /// Returns `true` if the request can proceed, `false` if the
    /// circuit is open and the request should be rejected immediately.
    pub fn allow_request(&mut self) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::HalfOpen => {
                // In half-open, allow a single probe
                self.half_open_successes == 0
            }
            CircuitState::Open => {
                // Check if recovery timeout has elapsed
                if let Some(last_fail) = self.last_failure_time {
                    let elapsed = Utc::now() - last_fail;
                    let recovery_duration =
                        chrono::Duration::milliseconds(self.config.recovery_timeout_ms as i64);

                    if elapsed > recovery_duration {
                        tracing::info!("Circuit transitioning from open to half-open");
                        self.state = CircuitState::HalfOpen;
                        self.half_open_successes = 0;
                        return true;
                    }
                }
                false
            }
        }
    }

    /// Record a successful operation.
    pub fn record_success(&mut self) {
        self.total_successes += 1;

        match self.state {
            CircuitState::HalfOpen => {
                self.half_open_successes += 1;
                if self.half_open_successes >= self.config.half_open_success_threshold {
                    tracing::info!("Circuit closing after {} successful half-open probes",
                        self.half_open_successes);
                    self.state = CircuitState::Closed;
                    self.failure_count = 0;
                    self.half_open_successes = 0;
                }
            }
            CircuitState::Closed => {
                // Reset failure count on success
                self.failure_count = 0;
            }
            CircuitState::Open => {
                // Should not happen — allow_request returns false when open
                tracing::warn!("Success recorded while circuit is open — unexpected");
            }
        }
    }

    /// Record a failed operation.
    pub fn record_failure(&mut self) {
        self.total_failures += 1;

        match self.state {
            CircuitState::Closed => {
                self.failure_count += 1;
                if self.failure_count >= self.config.failure_threshold {
                    tracing::warn!(
                        failures = self.failure_count,
                        threshold = self.config.failure_threshold,
                        "Circuit tripped to open"
                    );
                    self.state = CircuitState::Open;
                    self.last_failure_time = Some(Utc::now());
                }
            }
            CircuitState::HalfOpen => {
                tracing::warn!("Half-open probe failed — circuit re-opening");
                self.state = CircuitState::Open;
                self.half_open_successes = 0;
                self.failure_count = self.config.failure_threshold;
                self.last_failure_time = Some(Utc::now());
            }
            CircuitState::Open => {
                self.last_failure_time = Some(Utc::now());
            }
        }
    }

    /// Get the current circuit state.
    pub fn state(&self) -> CircuitState {
        self.state
    }

    /// Get total success count.
    pub fn total_successes(&self) -> u64 {
        self.total_successes
    }

    /// Get total failure count.
    pub fn total_failures(&self) -> u64 {
        self.total_failures
    }

    /// Force the circuit open (for testing or manual intervention).
    pub fn force_open(&mut self) {
        tracing::warn!("Circuit manually forced open");
        self.state = CircuitState::Open;
        self.failure_count = self.config.failure_threshold;
        self.last_failure_time = Some(Utc::now());
    }

    /// Force the circuit closed (for testing or manual intervention).
    pub fn force_closed(&mut self) {
        tracing::info!("Circuit manually forced closed");
        self.state = CircuitState::Closed;
        self.failure_count = 0;
        self.half_open_successes = 0;
    }

    /// Reset all counters (keeps current state).
    pub fn reset_counters(&mut self) {
        self.failure_count = 0;
        self.half_open_successes = 0;
        self.total_successes = 0;
        self.total_failures = 0;
    }
}

// ── Retry with Exponential Backoff + Jitter ────────────────────────

/// Configuration for retry behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retries.
    pub max_retries: u32,
    /// Base delay between retries in milliseconds.
    pub base_delay_ms: u64,
    /// Maximum delay between retries in milliseconds.
    pub max_delay_ms: u64,
    /// Whether to use jitter (randomization) in backoff.
    pub use_jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay_ms: 100,
            max_delay_ms: 10_000,
            use_jitter: true,
        }
    }
}

/// A retry policy with exponential backoff and jitter.
pub struct RetryPolicy {
    /// The retry configuration.
    config: RetryConfig,
}

impl RetryPolicy {
    /// Create a new retry policy.
    pub fn new(config: RetryConfig) -> Self {
        Self { config }
    }

    /// Create a retry policy with defaults.
    pub fn with_defaults() -> Self {
        Self::new(RetryConfig::default())
    }

    /// Calculate the delay for a given retry attempt (0-indexed).
    ///
    /// Uses exponential backoff: `base_delay * 2^attempt`.
    /// Capped at `max_delay`. Jitter adds ±25% randomization.
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let base = self.config.base_delay_ms as f64;
        let exponential = base * 2.0_f64.powi(attempt as i32);
        let capped = exponential.min(self.config.max_delay_ms as f64);

        if self.config.use_jitter {
            // Add ±25% jitter
            let jitter_range = capped * 0.25;
            let jitter = (rand::random::<f64>() * 2.0 - 1.0) * jitter_range;
            let with_jitter = (capped + jitter).max(0.0);
            Duration::from_millis(with_jitter as u64)
        } else {
            Duration::from_millis(capped as u64)
        }
    }

    /// Execute an async operation with retries.
    ///
    /// The operation is called at least once. If it fails, it is retried
    /// up to `max_retries` times with exponential backoff between attempts.
    pub async fn execute<F, Fut, T, E>(
        &self,
        operation_name: &str,
        mut f: F,
    ) -> Result<T, E>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
        E: std::fmt::Display,
    {
        let mut last_error: Option<E> = None;

        for attempt in 0..=self.config.max_retries {
            match f().await {
                Ok(value) => {
                    if attempt > 0 {
                        tracing::info!(
                            operation = operation_name,
                            attempt = attempt,
                            "Operation succeeded after {} retries", attempt
                        );
                    }
                    return Ok(value);
                }
                Err(e) => {
                    last_error = Some(e);
                    if attempt < self.config.max_retries {
                        let delay = self.delay_for_attempt(attempt);
                        tracing::warn!(
                            operation = operation_name,
                            attempt = attempt + 1,
                            max_retries = self.config.max_retries,
                            delay_ms = delay.as_millis(),
                            error = %last_error.as_ref().unwrap(),
                            "Retry attempt {}/{} — waiting {:?}",
                            attempt + 1,
                            self.config.max_retries,
                            delay,
                        );
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        tracing::error!(
            operation = operation_name,
            max_retries = self.config.max_retries,
            error = %last_error.as_ref().unwrap(),
            "Operation failed after {} attempts", self.config.max_retries + 1
        );

        Err(last_error.unwrap())
    }

    /// Get the maximum number of retries.
    pub fn max_retries(&self) -> u32 {
        self.config.max_retries
    }
}

// ── Timeout ────────────────────────────────────────────────────────

/// Configuration for timeout behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutConfig {
    /// Default timeout in milliseconds.
    pub default_timeout_ms: u64,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            default_timeout_ms: 30_000, // 30 seconds
        }
    }
}

/// Error returned when an operation times out.
#[derive(Debug, Clone, thiserror::Error)]
#[error("operation timed out after {elapsed_ms}ms (limit: {limit_ms}ms)")]
pub struct TimeoutError {
    /// The timeout limit that was exceeded.
    pub limit_ms: u64,
    /// How long the operation actually ran.
    pub elapsed_ms: u64,
}

/// Execute an async operation with a timeout deadline.
///
/// If the operation does not complete within the given duration,
/// it is cancelled and a `TimeoutError` is returned.
pub async fn with_timeout<F, T>(
    operation_name: &str,
    duration: Duration,
    f: F,
) -> Result<T, TimeoutError>
where
    F: std::future::Future<Output = T>,
{
    let start = std::time::Instant::now();

    match tokio::time::timeout(duration, f).await {
        Ok(result) => Ok(result),
        Err(_elapsed) => {
            let elapsed = start.elapsed();
            let limit_ms = duration.as_millis() as u64;
            let elapsed_ms = elapsed.as_millis() as u64;

            tracing::warn!(
                operation = operation_name,
                limit_ms = limit_ms,
                elapsed_ms = elapsed_ms,
                "Operation timed out"
            );

            Err(TimeoutError {
                limit_ms,
                elapsed_ms,
            })
        }
    }
}

// ── Rate Limiter (Token Bucket) ─────────────────────────────────────

/// Configuration for a rate limiter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimiterConfig {
    /// Maximum number of tokens (burst capacity).
    pub capacity: u32,
    /// Tokens refilled per second.
    pub refill_rate: f64,
    /// Refill interval in milliseconds.
    pub refill_interval_ms: u64,
}

impl Default for RateLimiterConfig {
    fn default() -> Self {
        Self {
            capacity: 100,
            refill_rate: 10.0, // 10 tokens per second
            refill_interval_ms: 100,
        }
    }
}

/// A token bucket rate limiter.
pub struct RateLimiter {
    /// Rate limiter configuration.
    config: RateLimiterConfig,
    /// Current number of available tokens.
    tokens: f64,
    /// Last time tokens were refilled.
    last_refill: std::time::Instant,
    /// Total requests allowed.
    total_allowed: u64,
    /// Total requests denied.
    total_denied: u64,
}

impl RateLimiter {
    /// Create a new rate limiter with the given configuration.
    pub fn new(config: RateLimiterConfig) -> Self {
        Self {
            config,
            tokens: config.capacity as f64,
            last_refill: std::time::Instant::now(),
            total_allowed: 0,
            total_denied: 0,
        }
    }

    /// Create a rate limiter with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(RateLimiterConfig::default())
    }

    /// Try to acquire a token.
    ///
    /// Returns `true` if a token was acquired, `false` if the
    /// rate limit has been exceeded.
    pub fn try_acquire(&mut self) -> bool {
        self.refill();

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            self.total_allowed += 1;
            true
        } else {
            self.total_denied += 1;
            tracing::warn!(
                tokens = self.tokens,
                capacity = self.config.capacity,
                "Rate limit exceeded"
            );
            false
        }
    }

    /// Acquire a token, waiting if necessary.
    pub async fn acquire(&mut self) {
        loop {
            self.refill();
            if self.tokens >= 1.0 {
                self.tokens -= 1.0;
                self.total_allowed += 1;
                return;
            }
            // Wait for the refill interval before trying again
            tokio::time::sleep(Duration::from_millis(self.config.refill_interval_ms)).await;
        }
    }

    /// Refill tokens based on elapsed time.
    fn refill(&mut self) {
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(self.last_refill);
        let elapsed_secs = elapsed.as_secs_f64();

        if elapsed_secs > 0.0 {
            let new_tokens = elapsed_secs * self.config.refill_rate;
            self.tokens = (self.tokens + new_tokens).min(self.config.capacity as f64);
            self.last_refill = now;
        }
    }

    /// Get the current number of available tokens.
    pub fn available_tokens(&self) -> f64 {
        self.tokens
    }

    /// Get total requests allowed.
    pub fn total_allowed(&self) -> u64 {
        self.total_allowed
    }

    /// Get total requests denied.
    pub fn total_denied(&self) -> u64 {
        self.total_denied
    }
}

// ── Health Check ───────────────────────────────────────────────────

/// The health status of a component.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum HealthStatus {
    /// Component is healthy.
    Healthy,
    /// Component is degraded — operating but with reduced capacity.
    Degraded,
    /// Component is unhealthy — not functioning.
    Unhealthy,
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthStatus::Healthy => write!(f, "healthy"),
            HealthStatus::Degraded => write!(f, "degraded"),
            HealthStatus::Unhealthy => write!(f, "unhealthy"),
        }
    }
}

/// Result of a health check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    /// The component name.
    pub component: String,
    /// The health status.
    pub status: HealthStatus,
    /// Optional diagnostic message.
    pub message: Option<String>,
    /// When the check was performed.
    pub checked_at: DateTime<Utc>,
    /// Duration of the check in milliseconds.
    pub duration_ms: u64,
}

/// Trait for components that can report their health.
///
/// Implement this trait for any subsystem that should be included
/// in system-wide health checks.
#[async_trait::async_trait]
pub trait HealthCheck: Send + Sync {
    /// Return the name of this component.
    fn name(&self) -> &str;

    /// Perform a health check and return the result.
    async fn check_health(&self) -> HealthCheckResult;
}

/// A registry of health-checkable components.
pub struct HealthRegistry {
    /// Registered health checks.
    checks: Vec<Box<dyn HealthCheck>>,
}

impl HealthRegistry {
    /// Create a new empty health registry.
    pub fn new() -> Self {
        Self { checks: Vec::new() }
    }

    /// Register a health check.
    pub fn register(&mut self, check: Box<dyn HealthCheck>) {
        tracing::debug!(component = check.name(), "Health check registered");
        self.checks.push(check);
    }

    /// Run all registered health checks and return results.
    pub async fn check_all(&self) -> Vec<HealthCheckResult> {
        let mut results = Vec::with_capacity(self.checks.len());
        for check in &self.checks {
            let start = std::time::Instant::now();
            let result = check.check_health().await;
            let duration_ms = start.elapsed().as_millis() as u64;

            results.push(HealthCheckResult {
                duration_ms,
                ..result
            });
        }
        results
    }

    /// Check if all components are healthy.
    pub async fn is_healthy(&self) -> bool {
        self.check_all()
            .await
            .iter()
            .all(|r| r.status == HealthStatus::Healthy)
    }

    /// Get the aggregate health status.
    ///
    /// - If any component is unhealthy → Unhealthy
    /// - If any component is degraded → Degraded
    /// - Otherwise → Healthy
    pub async fn aggregate_status(&self) -> HealthStatus {
        let results = self.check_all().await;
        if results.iter().any(|r| r.status == HealthStatus::Unhealthy) {
            HealthStatus::Unhealthy
        } else if results.iter().any(|r| r.status == HealthStatus::Degraded) {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        }
    }

    /// Number of registered checks.
    pub fn count(&self) -> usize {
        self.checks.len()
    }
}

impl Default for HealthRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ── Composite: Resilient Operation ─────────────────────────────────

/// Execute an operation with full resilience: circuit breaker, retries, and timeout.
///
/// This is the recommended way to call external dependencies. It layers:
/// 1. Circuit breaker — fail fast if the dependency is known-bad
/// 2. Timeout — every call has a deadline
/// 3. Retry — transient failures get retried with backoff
pub async fn execute_resilient<F, Fut, T, E>(
    operation_name: &str,
    circuit_breaker: &Arc<Mutex<CircuitBreaker>>,
    retry_policy: &RetryPolicy,
    timeout_duration: Duration,
    mut f: F,
) -> Result<T, ResilientError<E>>
where
    F: FnMut() -> Fut + Send,
    Fut: std::future::Future<Output = Result<T, E>> + Send,
    T: Send,
    E: std::fmt::Display + Send,
{
    // 1. Check circuit breaker
    {
        let mut cb = circuit_breaker.lock().await;
        if !cb.allow_request() {
            tracing::warn!(
                operation = operation_name,
                state = %cb.state(),
                "Circuit breaker rejected request"
            );
            return Err(ResilientError::CircuitOpen);
        }
    }

    // 2. Execute with retries and timeout
    let result = retry_policy
        .execute(operation_name, || async {
            match with_timeout(operation_name, timeout_duration, f()).await {
                Ok(inner_result) => inner_result.map_err(ResilientError::OperationFailed),
                Err(timeout_err) => Err(ResilientError::Timeout(timeout_err)),
            }
        })
        .await;

    // 3. Update circuit breaker
    let mut cb = circuit_breaker.lock().await;
    match &result {
        Ok(_) => cb.record_success(),
        Err(_) => cb.record_failure(),
    }

    result
}

/// Errors that can occur during a resilient operation.
#[derive(Debug, thiserror::Error)]
pub enum ResilientError<E: std::fmt::Display> {
    /// The circuit breaker is open.
    #[error("circuit breaker is open — request rejected")]
    CircuitOpen,

    /// The operation timed out.
    #[error("operation timed out: {0}")]
    Timeout(#[from] TimeoutError),

    /// The operation failed after all retries.
    #[error("operation failed: {0}")]
    OperationFailed(#[source] E),
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Circuit Breaker Tests ──────────────────────────────────

    #[test]
    fn circuit_starts_closed() {
        let cb = CircuitBreaker::with_defaults();
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn circuit_trips_after_threshold_failures() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            recovery_timeout_ms: 60_000,
            half_open_success_threshold: 2,
        };
        let mut cb = CircuitBreaker::new(config);

        // First 2 failures — still closed
        for _ in 0..2 {
            assert!(cb.allow_request());
            cb.record_failure();
        }
        assert_eq!(cb.state(), CircuitState::Closed);

        // 3rd failure trips the circuit
        assert!(cb.allow_request());
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn circuit_rejects_when_open() {
        let config = CircuitBreakerConfig {
            failure_threshold: 1,
            recovery_timeout_ms: 60_000,
            half_open_success_threshold: 1,
        };
        let mut cb = CircuitBreaker::new(config);

        // Trip the circuit
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        // Requests should be rejected
        assert!(!cb.allow_request());
    }

    #[test]
    fn circuit_transitions_to_half_open_after_timeout() {
        let config = CircuitBreakerConfig {
            failure_threshold: 1,
            recovery_timeout_ms: 0, // Immediate recovery for testing
            half_open_success_threshold: 1,
        };
        let mut cb = CircuitBreaker::new(config);

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        // With 0ms timeout, next request should go half-open
        assert!(cb.allow_request());
        assert_eq!(cb.state(), CircuitState::HalfOpen);
    }

    #[test]
    fn circuit_closes_after_successful_half_open_probes() {
        let config = CircuitBreakerConfig {
            failure_threshold: 1,
            recovery_timeout_ms: 0,
            half_open_success_threshold: 2,
        };
        let mut cb = CircuitBreaker::new(config);

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        // First probe
        assert!(cb.allow_request());
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        // Second probe closes the circuit
        assert!(cb.allow_request());
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn force_open_and_closed() {
        let mut cb = CircuitBreaker::with_defaults();
        assert_eq!(cb.state(), CircuitState::Closed);

        cb.force_open();
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.allow_request());

        cb.force_closed();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.allow_request());
    }

    // ── Retry Tests ────────────────────────────────────────────

    #[test]
    fn retry_delay_exponential_backoff() {
        let config = RetryConfig {
            base_delay_ms: 100,
            max_delay_ms: 10_000,
            max_retries: 3,
            use_jitter: false,
        };
        let policy = RetryPolicy::new(config);

        assert_eq!(policy.delay_for_attempt(0).as_millis(), 100);
        assert_eq!(policy.delay_for_attempt(1).as_millis(), 200);
        assert_eq!(policy.delay_for_attempt(2).as_millis(), 400);
        assert_eq!(policy.delay_for_attempt(3).as_millis(), 800);
    }

    #[test]
    fn retry_delay_capped_at_max() {
        let config = RetryConfig {
            base_delay_ms: 1000,
            max_delay_ms: 5_000,
            max_retries: 5,
            use_jitter: false,
        };
        let policy = RetryPolicy::new(config);

        // With base 1000, attempt 3 = 8000ms, but capped at 5000
        assert_eq!(policy.delay_for_attempt(3).as_millis(), 5_000);
    }

    #[tokio::test]
    async fn retry_succeeds_on_first_attempt() {
        let policy = RetryPolicy::with_defaults();
        let result = policy
            .execute("test", || async { Ok::<_, &str>("success") })
            .await;
        assert_eq!(result.unwrap(), "success");
    }

    #[tokio::test]
    async fn retry_succeeds_after_failures() {
        let mut attempts = 0;
        let policy = RetryPolicy::new(RetryConfig {
            max_retries: 3,
            base_delay_ms: 1,
            max_delay_ms: 10,
            use_jitter: false,
        });

        let result = policy
            .execute("test", || {
                attempts += 1;
                async move {
                    if attempts < 3 {
                        Err("transient error")
                    } else {
                        Ok("recovered")
                    }
                }
            })
            .await;

        assert_eq!(result.unwrap(), "recovered");
        assert_eq!(attempts, 3);
    }

    #[tokio::test]
    async fn retry_exhausted() {
        let config = RetryConfig {
            max_retries: 2,
            base_delay_ms: 1,
            max_delay_ms: 10,
            use_jitter: false,
        };
        let policy = RetryPolicy::new(config);

        let result = policy
            .execute("test", || async { Err::<(), _>("always fails") })
            .await;

        assert!(result.is_err());
    }

    // ── Timeout Tests ──────────────────────────────────────────

    #[tokio::test]
    async fn timeout_completes_within_deadline() {
        let result = with_timeout("test", Duration::from_secs(5), async { 42 }).await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn timeout_exceeded() {
        let result = with_timeout("test", Duration::from_millis(1), async {
            tokio::time::sleep(Duration::from_millis(100)).await;
            42
        })
        .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.limit_ms, 1);
    }

    // ── Rate Limiter Tests ─────────────────────────────────────

    #[test]
    fn rate_limiter_allows_within_burst() {
        let mut rl = RateLimiter::with_defaults();
        // Burst of 100 should all pass
        for _ in 0..100 {
            assert!(rl.try_acquire());
        }
        // 101st should be denied (no refill yet)
        assert!(!rl.try_acquire());
    }

    #[test]
    fn rate_limiter_counts() {
        let config = RateLimiterConfig {
            capacity: 10,
            refill_rate: 100.0,
            refill_interval_ms: 10,
        };
        let mut rl = RateLimiter::new(config);

        for _ in 0..5 {
            rl.try_acquire();
        }
        assert_eq!(rl.total_allowed(), 5);

        for _ in 0..10 {
            rl.try_acquire();
        }
        assert_eq!(rl.total_allowed(), 10); // 5 more allowed, 5 denied
        assert_eq!(rl.total_denied(), 5);
    }

    // ── Health Check Tests ─────────────────────────────────────

    struct AlwaysHealthy;
    #[async_trait::async_trait]
    impl HealthCheck for AlwaysHealthy {
        fn name(&self) -> &str { "always-healthy" }
        async fn check_health(&self) -> HealthCheckResult {
            HealthCheckResult {
                component: self.name().to_string(),
                status: HealthStatus::Healthy,
                message: None,
                checked_at: Utc::now(),
                duration_ms: 0,
            }
        }
    }

    struct AlwaysUnhealthy;
    #[async_trait::async_trait]
    impl HealthCheck for AlwaysUnhealthy {
        fn name(&self) -> &str { "always-unhealthy" }
        async fn check_health(&self) -> HealthCheckResult {
            HealthCheckResult {
                component: self.name().to_string(),
                status: HealthStatus::Unhealthy,
                message: Some("simulated failure".to_string()),
                checked_at: Utc::now(),
                duration_ms: 0,
            }
        }
    }

    #[tokio::test]
    async fn health_registry_aggregates() {
        let mut registry = HealthRegistry::new();
        registry.register(Box::new(AlwaysHealthy));
        registry.register(Box::new(AlwaysUnhealthy));

        assert_eq!(registry.aggregate_status().await, HealthStatus::Unhealthy);
    }

    #[tokio::test]
    async fn health_registry_all_healthy() {
        let mut registry = HealthRegistry::new();
        registry.register(Box::new(AlwaysHealthy));
        registry.register(Box::new(AlwaysHealthy));

        assert!(registry.is_healthy().await);
    }
}
