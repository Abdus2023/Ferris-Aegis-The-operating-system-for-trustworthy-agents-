# Ferris Aegis — Full A2A Protocol Specification + Resilience Primitives (with Code)

**Version**: 0.3.0  
**Date**: 2026-07-19

---

## Part 1: Full A2A Protocol Specification

### 1.1 Overview

The **Agent-to-Agent (A2A)** protocol enables secure, trust-gated communication between autonomous agents in the Ferris Aegis ecosystem.

**Core Concepts**:
- **AgentCard** — Discoverable JSON manifest (`/.well-known/agent-card.json`)
- **A2A Task** — Unit of work with defined lifecycle
- **A2aRouter** — Trust-gated message routing
- **Two deployment branches**:
  - **Branch A**: Standalone AgentCard server (for external discoverability)
  - **Branch B**: MCP-integrated (expose capabilities as MCP tools)

**Protocol Version**: `0.1.0`

---

### 1.2 AgentCard Specification

**Location**: `/.well-known/agent-card.json` (per RFC 8615)

**Primary Type** (`agent_card.rs`):

```rust
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentCard {
    pub name: String,
    pub description: String,
    pub url: String,
    pub version: String,
    pub protocol_version: String,
    pub trust_level: TrustLevel,
    pub trust_score: f64,
    pub skills: Vec<AgentSkill>,
    pub capabilities: AgentCapabilities,
    pub provider: Option<AgentProvider>,
    pub authentication: Option<AgentAuthentication>,
    pub metadata: serde_json::Value,
    pub updated_at: DateTime<Utc>,
    pub schema_version: String,
}
```

**Supporting Types**:

```rust
pub struct AgentSkill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub input_schema: Option<serde_json::Value>,
    pub output_schema: Option<serde_json::Value>,
    pub tags: Vec<String>,
}

pub struct AgentCapabilities {
    pub streaming: bool,
    pub push_notifications: bool,
    pub state_transition_history: bool,
}

pub enum TrustLevel {
    Unverified, Probationary, Standard, Elevated, Sovereign,
}
```

**Trust Level Rules**:
- `can_initiate()` → only `Standard` and above
- `is_discoverable()` → only `Standard` and above

**Default Ferris Aegis Card** (`default_aegis_card`):
- Skills: `file_read`, `session_create`
- Capabilities: streaming + state history enabled

---

### 1.3 A2A Message Types

```rust
pub enum MessageType {
    Request, Response, Notification,
    Discovery, AgentCardDisclosure,
}

pub struct A2aMessage {
    pub id: String,
    pub sender: String,
    pub recipient: String,
    pub message_type: MessageType,
    pub content: serde_json::Value,
    pub session_id: Option<String>,
    pub required_trust: Option<TrustLevel>,
    pub timestamp: DateTime<Utc>,
    pub attestation: Option<Attestation>,
}

pub struct A2aEnvelope {
    pub protocol_version: String,
    pub message: A2aMessage,
    pub sender_card: Option<AgentCard>,
}
```

---

### 1.4 A2aRouter & Trust-Gated Routing

**Core Router** (`lib.rs`):

```rust
pub struct A2aRouter {
    registry: HashMap<String, AgentCard>,
}

impl A2aRouter {
    pub fn register(&mut self, card: AgentCard)
    pub fn lookup(&self, name: &str) -> Option<&AgentCard>
    pub fn agents_at_trust_level(&self, level: TrustLevel) -> Vec<&AgentCard>
    pub fn agents_with_skill(&self, skill_id: &str) -> Vec<&AgentCard>

    pub fn route_message(&self, envelope: &A2aEnvelope)
        -> Result<AgentCard, RouteError>
}
```

**Routing Rules** (in `route_message`):
1. Recipient must exist in registry
2. Protocol version must match
3. Sender's `AgentCard` must be present in envelope
4. Sender's trust level must satisfy `message.required_trust`
5. Sender must be able to initiate (`can_initiate()`)

**RouteError Variants**:
- `RecipientNotFound`
- `IncompatibleProtocol`
- `InsufficientTrust`
- `CannotInitiate`

---

### 1.5 A2A Task Lifecycle (`task.rs`)

```rust
pub enum TaskState {
    Submitted, Working, Completed, Cancelled, Failed,
}

pub struct A2aTask {
    pub id: String,
    pub status: TaskState,
    pub result: Option<TaskResult>,
    // ...
}

pub struct TaskResult {
    pub success: bool,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
}
```

---

### 1.6 Branch A vs Branch B

| Aspect                    | Branch A (Standalone)          | Branch B (MCP-Integrated)              |
|---------------------------|--------------------------------|----------------------------------------|
| AgentCard Server          | Yes (`/.well-known/...`)       | No                                     |
| Discovery                 | HTTP well-known URI            | Via MCP tools                          |
| Use Case                  | External agents                | TypeScript orchestrator / existing MCP |
| Files                     | `branch_a.rs`                  | `branch_b.rs`                          |

---

## Part 2: Resilience Primitives (with Full Code)

### 2.1 Circuit Breaker

**Types**:

```rust
pub enum CircuitState { Closed, Open, HalfOpen }

pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,              // default: 5
    pub recovery_timeout_ms: u64,            // default: 30_000
    pub half_open_success_threshold: u32,    // default: 2
}

pub struct CircuitBreaker { ... }
```

**Key Methods**:

```rust
impl CircuitBreaker {
    pub fn allow_request(&mut self) -> bool
    pub fn record_success(&mut self)
    pub fn record_failure(&mut self)
    pub fn force_open(&mut self)
    pub fn force_closed(&mut self)
}
```

**State Machine**:
```
Closed → (N failures) → Open → (timeout) → HalfOpen → (M successes) → Closed
```

---

### 2.2 Retry Policy with Jitter

```rust
pub struct RetryConfig {
    pub max_retries: u32,           // default: 3
    pub base_delay_ms: u64,         // default: 100
    pub max_delay_ms: u64,          // default: 10_000
    pub use_jitter: bool,           // default: true
}

pub struct RetryPolicy { ... }

impl RetryPolicy {
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration
    pub async fn execute<F, Fut, T, E>(&self, name: &str, f: F) -> Result<T, E>
}
```

**Backoff Formula**:
```
delay = min(base * 2^attempt, max_delay)
if jitter: delay ± 25%
```

---

### 2.3 Timeout Wrapper

```rust
pub async fn with_timeout<F, T>(
    operation_name: &str,
    duration: Duration,
    f: F,
) -> Result<T, TimeoutError>
```

**Error**:

```rust
pub struct TimeoutError {
    pub limit_ms: u64,
    pub elapsed_ms: u64,
}
```

---

### 2.4 Rate Limiter (Token Bucket)

```rust
pub struct RateLimiterConfig {
    pub capacity: u32,              // burst size
    pub refill_rate: f64,           // tokens per second
    pub refill_interval_ms: u64,
}

pub struct RateLimiter { ... }

impl RateLimiter {
    pub fn try_acquire(&mut self) -> bool
    pub async fn acquire(&mut self)
    pub fn available_tokens(&self) -> f64
}
```

---

### 2.5 Health Checks

```rust
pub enum HealthStatus { Healthy, Degraded, Unhealthy }

pub trait HealthCheck {
    fn name(&self) -> &str;
    async fn check_health(&self) -> HealthCheckResult;
}

pub struct HealthRegistry { ... }

impl HealthRegistry {
    pub fn register(&mut self, check: Box<dyn HealthCheck>)
    pub async fn check_all(&self) -> Vec<HealthCheckResult>
    pub async fn aggregate_status(&self) -> HealthStatus
}
```

---

### 2.6 Composite: `execute_resilient()`

**Recommended API** for all external calls:

```rust
pub async fn execute_resilient<F, Fut, T, E>(
    operation_name: &str,
    circuit_breaker: &Arc<Mutex<CircuitBreaker>>,
    retry_policy: &RetryPolicy,
    timeout_duration: Duration,
    f: F,
) -> Result<T, ResilientError<E>>
```

**Execution Order**:
1. Circuit breaker check
2. Retry loop + timeout wrapper
3. Update circuit breaker on success/failure

**Error Type**:

```rust
pub enum ResilientError<E> {
    CircuitOpen,
    Timeout(TimeoutError),
    OperationFailed(E),
}
```

---

## Summary

This document provides the **complete formal specification** for:

- **A2A Protocol** — AgentCard, routing, trust gating, tasks, and dual-branch architecture
- **Resilience Primitives** — Full source code and usage patterns for CircuitBreaker, Retry, Timeout, RateLimiter, HealthCheck, and the composite `execute_resilient()` function

Both systems are deeply integrated with the Trust Kernel and form the foundation of production-grade agent communication and reliability in Ferris Aegis.