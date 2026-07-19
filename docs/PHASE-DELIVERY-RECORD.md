# Ferris Aegis — Phase Delivery Record

> Detailed per-phase record of what was delivered, when, and how.
> Branch: `arena/019f7a09-ferris-aegis-the-operating-sys`
> Version: 0.4.0 | ~39 Rust files | ~13,100 lines | 13 crates

---

## Phase 1 — Core Kernel

**Status:** ✅ Complete (merged as PR #1, `8be46a6`)
**Crate:** `crates/kernel` (3,556 lines)

| Module | Types | Lines |
|--------|-------|-------|
| `kernel.rs` | `TrustKernel`, `TrustLevel`, `TrustScore`, `TrustRecord` | ~470 |
| `agent.rs` | `AgentRuntime`, `AgentId`, `Agent`, `AgentState`, `AgentStatus` | ~460 |
| `policy.rs` | `PolicyEngine`, `Policy`, `PolicyVerdict`, `Effect`, `Priority` | ~445 |
| `audit.rs` | `AuditLedger`, `AuditEntry`, `AuditSeverity` | ~430 |
| `sandbox.rs` | `Sandbox`, `Capability` (12 variants), `SandboxBoundary` | ~495 |
| `guard.rs` | `Guard`, `GuardAction`, `GuardConfig`, `GuardAlert` | ~505 |
| `config.rs` | `AegisConfig`, `SystemConfig`, `TrustConfig`, `SandboxConfig`, `AuditConfig`, `ConfigError`, `validate()`, `warnings()` | ~390 |
| `health.rs` | `ComponentHealth`, `HealthReport`, `SystemHealth` | ~240 |

### Trust Levels

| Level | Score | Capabilities |
|-------|-------|-------------|
| Unverified | 0.00–0.19 | Timer, Inter-agent comm |
| Probationary | 0.20–0.49 | + Filesystem read |
| Standard | 0.50–0.74 | + Network, Environment, Audit |
| Elevated | 0.75–0.94 | + Filesystem write, Process spawn, Crypto |
| Sovereign | 0.95–1.00 | All |

### Guard Escalation

```
Alert → Throttle → Quarantine → Terminate
```

### Acceptance Tests

- `test_full_trust_lifecycle` — 10 reinforcements → Standard trust
- `test_agent_lifecycle` — spawn → suspend → resume → terminate
- `test_audit_ledger_chain` — 5 entries → chain verifies
- `test_policy_default_safety` — file:read allowed, file:write to /etc denied

---

## Phase 2 — Observability + MCP

**Status:** ✅ Complete (merged as PR #1, `8be46a6`)
**Crates:** `crates/observability` (290 lines), `crates/mcp` (343 lines)

### Week 3: Observability

- OTel tracing via `tracing-opentelemetry 0.28` + `opentelemetry_sdk 0.27` with `install_batch(Tokio)`
- Prometheus metrics: `requests_total`, `tokens_used_total`, `tool_calls_total`
- JSON stderr logging via `tracing-subscriber 0.3`
- **Key invariant:** All output to stderr only. MCP owns stdout.

### Week 4: MCP Server

- `rmcp 2.2` with `server`, `macros`, `transport-io` features
- Protocol version: `V_2025_11_25` (explicitly pinned, never `.LATEST`)
- Tool: `file_read` — absolute paths only, no traversal, max bytes configurable
- Every tool handler instrumented with OTel spans + Prometheus counters

### Acceptance Tests

- `test_observability_metrics` — counter increments work
- `test_mcp_file_read_security` — relative paths rejected, nonexistent paths rejected

---

## Phase 3 — Security + Memory + Plugin

**Status:** ✅ Complete (merged as PR #1, `8be46a6`)
**Crates:** `crates/security` (1,098 lines), `crates/sandbox-wasm` (362), `crates/memory` (421), `crates/plugin` (334)

### Security Pipeline

| Component | Key Types | Lines |
|-----------|-----------|-------|
| Tool Allowlist | `ToolAllowlist`, `AllowlistVerdict` | ~130 |
| Injection Scanner | `InjectionScanner`, `InjectionVerdict` — 11 regex patterns, 9 attack categories | ~225 |
| SSRF Guard | `SsrfGuard`, `SsrfVerdict` — IPv4/IPv6, hostname, URL validation | ~340 |
| Credential Vault | `CredentialVault`, `ProtectedSecret`, `AuthenticatedCall`, `ToolCall`, `StoredCredential` | ~400 |

### Injection Patterns (11 patterns, 9 categories)

System prompt override (ignore-previous, disregard-previous, forget-previous), role manipulation, output format manipulation, delimiter injection, command injection, data exfiltration, shell injection

### SSRF Blocked Ranges

`127.0.0.0/8`, `10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`, `169.254.0.0/16` (cloud metadata), `::1`, `fc00::/7`, `fe80::/10`

### WASM Sandbox

Fuel metering (default 10M instructions), memory cap (default 64 MiB), epoch interruption (deadline = 1)

### Plugin Verification Flow

Load manifest → verify Ed25519 signature → compute SHA-256 of WASM → compare hash → load

### Acceptance Tests (8 criteria)

allowlist check, injection scanner, WASM fuel exhaustion, SQLite persistence, no keys in trace, Ed25519 manifest verification, SSRF guard, tool-call spans from call.arguments only

---

## Phase 4 — Session + Supervisor + Semantic Memory + A2A

**Status:** ✅ Complete (PR #4, merged)
**Crates:** `crates/session` (289 lines), `crates/supervisor` (458), `crates/semantic-memory` (629), `crates/a2a` (1,287)

### 4.2 — Session Manager

- `Session` with `Clone` derive, 4-field budget (tokens, cost, rounds, wall-clock), `SessionManager` with `HashMap<String, Session>`
- Budget checked at each round; budget-exhausted sessions transition to terminal

### 4.4 — Supervisor (Anomaly Detection)

- Rate anomaly monitoring (turns/minute), trust decay detection (score thresholds), context drift (placeholder), severity grading (Info→Critical)
- Recommendation engine: Log → Notify → SuspendSession → QuarantineAgent → TerminateAgent

### 4.3 — Semantic Memory

- `Concept` extraction from conversation text (keyword matching), `StoredEmbedding` with cosine similarity, `Summary` for conversation compression
- SQLite-backed via `sqlx 0.9`: concepts, embeddings, summaries tables

### 4.1 — A2A Protocol

- `agent_card.rs`: `AgentCard` with `AgentSkill`, `AgentCapabilities`, `AgentProvider`, `AgentAuthentication`, `AgentCardBuilder`, `default_aegis_card()`, `AGENT_CARD_PATH`
- `task.rs`: `A2aTask` lifecycle (Submitted→Working→Completed/Cancelled/Failed), `TaskResult`
- `branch_a.rs`: `A2aServerConfig`, standalone AgentCard server
- `branch_b.rs`: MCP tool params (`SessionCreateParams`, `SupervisorInspectParams`, `MemorySearchParams`), `BudgetStatus`

### Trust-Gated Routing (`lib.rs`)

`A2aRouter` with registry, trust-level filters, skill-based discovery, `route_message()` with protocol/trust/sender checks, `RouteError` enum

### Acceptance Tests (10 criteria)

Session lifecycle + clone, session manager, supervisor rate anomaly, trust decay recommendation, semantic memory concepts/embeddings/similarity, A2A AgentCard + JSON Schema, A2A routing, A2A trust gating, A2A serialization, ProtectedSecret structural test

---

## Phase 5 — Production Hardening

**Status:** ✅ Complete (PR #4, merged)
**Crates:** `crates/resilience` (1,046 lines), `kernel/src/health.rs` (~240 lines)

### Resilience Primitives

| Primitive | Key Types | Algorithm |
|-----------|-----------|-----------|
| Circuit Breaker | `CircuitBreaker`, `CircuitBreakerConfig`, `CircuitState` | Closed→Open on N failures→HalfOpen after timeout→Closed on M successes |
| Retry | `RetryPolicy`, `RetryConfig` | Exponential backoff: `base * 2^attempt`, ±25% jitter, configurable max |
| Timeout | `with_timeout()`, `TimeoutError` | `tokio::time::timeout` wrapper, deadline enforcement |
| Rate Limiter | `RateLimiter`, `RateLimiterConfig` | Token bucket: configurable capacity + refill rate |
| Health Check | `HealthRegistry`, `HealthCheck` trait, `HealthCheckResult` | Register components → aggregate status (Healthy/Degraded/Unhealthy) |
| Composite | `execute_resilient()` | Layers: circuit breaker → timeout → retry |

### Kernel Hardening

| Module | What |
|--------|------|
| `health.rs` | `ComponentHealth`, `HealthReport`, `SystemHealth` — per-component + aggregate health |
| `config.rs` | `validate()` — range/format checks on all config sections; `warnings()` — edge case detection; `ConfigError` type |

### CLI Hardening (`src/main.rs`)

- `aegis health` — runs `SystemHealth::report()`, displays component status
- `aegis status` — expanded to show all 5 phases with readiness indicators

### Acceptance Tests (11 criteria)

Config validation, circuit breaker trip/recovery, retry backoff + jitter, rate limiter token bucket, timeout enforcement, health registry aggregation, kernel system health, config warnings, circuit breaker force open/close, rate limiter refill, full pipeline end-to-end

---

## Phase 5.1 — Durable Execution

**Status:** ✅ Complete (this PR)
**Crate:** `crates/durable` (~1,200 lines)

### Core Abstractions

| Component | Key Types | Purpose |
|-----------|-----------|---------|
| Step | `Step`, `StepFn`, `StepOutcome` | Unit of durable work |
| Workflow | `Workflow`, `WorkflowId`, `WorkflowStatus` | Ordered sequence of steps |
| Checkpoint | `Checkpoint`, `CheckpointData` | Snapshot of workflow state at step boundary |
| CheckpointStore | `CheckpointStore` trait, `InMemoryCheckpointStore`, `SqliteCheckpointStore` | Pluggable persistence backend |
| DurableExecutor | `DurableExecutor`, `DurableExecutorConfig` | Runs workflows with checkpoint durability |
| CrashRecovery | `CrashRecovery`, `RecoveryResult`, `RecoveryDetail` | Scans for incomplete workflows, prepares recovery |

### StepOutcome

Every step produces a `StepOutcome`:
- `success(step_name, output)` — step completed with JSON output
- `failure(step_name, error)` — step failed with error message
- Content hash via SHA-256 for tamper evidence
- Serialized and persisted as part of each checkpoint

### Checkpoint Lifecycle

```
Step executes → StepOutcome produced → Checkpoint created →
Checkpoint.saved() → hash verified → next step begins
```

If crash occurs:
```
Process restarts → DurableExecutor.run() → recover_state() →
load_latest() → verify_hash() → resume from next_step_index
```

### DurableExecutorConfig

| Field | Default | Purpose |
|-------|---------|---------|
| `checkpoint_enabled` | `true` | Toggle checkpoint persistence |
| `max_step_retries` | `0` | Retry transient step failures |
| `step_timeout_ms` | `0` | Per-step timeout (0 = no timeout) |
| `verify_hashes` | `true` | Verify checkpoint hashes on load |

### CrashRecovery

Scans `CheckpointStore::find_incomplete()` for workflows where:
- `step_index + 1 < total_steps` (more steps remain)
- Last step outcome was a success (not a failure)

Returns `RecoveryResult` with:
- `found`: count of incomplete workflows
- `recovered`: count successfully prepared for recovery
- `details`: per-workflow `RecoveryDetail` with `resume_from_step`

### SQLite Schema

```sql
CREATE TABLE checkpoints (
    workflow_id TEXT NOT NULL,
    step_index INTEGER NOT NULL,
    checkpoint_data TEXT NOT NULL,  -- serialized Checkpoint
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (workflow_id, step_index)
);
```

### In-Memory Store

`InMemoryCheckpointStore` uses `HashMap<String, Vec<Checkpoint>>` — suitable for
testing, not production. All methods are async-compatible.

### Design Decisions

1. **Checkpoint after every step** — Guarantees at most one step re-execution after crash
2. **Pluggable storage** — Trait-based, in-memory for tests, SQLite for production
3. **Hash verification** — SHA-256 content hash detects checkpoint tampering
4. **CrashRecovery is metadata-only** — Returns recovery info; caller reconstructs
   `Workflow` and passes to `DurableExecutor::run()`. Future: store step definitions
   in checkpoints for automatic re-execution.

### Acceptance Tests (10 criteria, #30–#39)

| # | Criterion | Test |
|---|-----------|------|
| 30 | StepOutcome durability | Checkpoints written after each step |
| 31 | Crash recovery | Resume from last checkpoint after simulated crash |
| 32 | Step chaining | Output of step N feeds step N+1 |
| 33 | Hash verification | Original verifies, tampered fails |
| 34 | Failure stops workflow | Failed step halts, no further steps execute |
| 35 | CrashRecovery scan | Finds incomplete, skips complete workflows |
| 36 | Step retry | Transient failures retried until success |
| 37 | Empty workflow rejected | No steps → error |
| 38 | Serialization roundtrip | StepOutcome/Checkpoint serialize/deserialize correctly |
| 39 | Full pipeline with durable | All phases including 5.1 integrated |

### Unit Tests (30+ in-crate)

WorkflowId, StepOutcome (success/failure/hash), Step (execute/chaining), Workflow
(creation/steps/metadata), Checkpoint (hash/tamper/complete/incomplete/next_step),
InMemoryStore (save/load/latest/find_incomplete/count/delete), DurableExecutor
(simple/failure/empty/chained/durability/crash-recovery/retry/no-checkpoint/cancel),
CrashRecovery (empty/finds-incomplete/skips-complete/get-checkpoint),
WorkflowStatus (terminal/display), WorkflowResult (helpers),
DurableExecutorConfig (default).

---

## Compile Fix History

| # | Symptom | Root Cause | Fix |
|---|---------|-----------|-----|
| FIX-001 | `Secret` not found | `Secret<T>` removed in secrecy 0.10 | Use `SecretBox<T>` |
| FIX-002 | `rand_nonce` module not found | API changed in aes-gcm 0.11 | Use `Aes256Gcm::generate_nonce()` |
| FIX-003 | `SecretString::new(String)` type error | `SecretBox<str>` wraps `Box<str>` | Add `.into()` |
| FIX-004 | Missing `schemars` dep | a2a crate omitted it | Add `schemars = { workspace = true }` |
| FIX-005 | `Session` missing `Clone` | `SessionManager` clones it | Add `#[derive(Clone)]` |
| FIX-006 | `AuthenticatedCall.credential: SecretString` | `secrecy/serde` unification risk | Changed to `ProtectedSecret` |

---

## PR Status

| PR | Branch | State | Content |
|----|--------|-------|---------|
| #1 | `arena/019f710a` | **MERGED** | Phases 1–3 on `main` |
| #3 | `arena/019f710a` | **CLOSED** | ractor supervisor + `Option<SecretString>` vault (superseded) |
| #4 | `arena/019f7994` | **MERGED** | Phases 4+5: ProtectedSecret fix, anomaly supervisor, a2a, resilience, health |
| #5 | `arena/019f7a09` | **OPEN** — this PR | Phase 5.1: Durable execution, checkpoint durability, crash recovery |

*Updated: 2026-07-19. Version 0.4.0. 13 crates, ~39 Rust files, ~13,100 lines, 47 integration tests.*
