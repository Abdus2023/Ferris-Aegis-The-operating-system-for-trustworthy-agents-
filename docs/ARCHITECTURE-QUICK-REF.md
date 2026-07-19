# Ferris Aegis — Architecture & Security Invariant Reference Card

> Quick-reference companion to [TRACEABILITY.md](./TRACEABILITY.md).
> Print this. Keep it next to your terminal.

## Security Invariants

| ID | Invariant | Where Enforced | Key Type | Verified By |
|----|-----------|----------------|----------|-------------|
| INV-001 | Credentials cannot reach LLM context | `vault.rs` → `AuthenticatedCall` | `ProtectedSecret` | 6 tests |
| INV-002 | `ProtectedSecret` cannot be serialized | `vault.rs` → no `Serialize` impl | `ProtectedSecret` | Compile-time |
| INV-003 | No `secrecy/serde` feature enabled | `Cargo.toml` + `ProtectedSecret` | — | `cargo tree` |
| INV-004 | Observability writes stderr only | `observability/lib.rs` | — | Code review |
| INV-005 | Tool allowlist is deny-by-default | `allowlist.rs` | `ToolAllowlist` | 3 tests |
| INV-006 | Audit ledger is tamper-evident | `audit.rs` | `AuditLedger` | Chain test |
| INV-007 | WASM execution terminates on fuel exhaustion | `sandbox-wasm/lib.rs` | `WasmSandbox` | 2 tests |
| INV-008 | SSRF guard blocks private IPs | `ssrf.rs` | `SsrfGuard` | 12 tests |
| INV-009 | Plugins must be Ed25519-signed | `plugin/lib.rs` | `PluginKeyring` | 8 tests |
| INV-010 | Config validated before use | `config.rs` | `AegisConfig::validate()` | 5 tests |
| INV-011 | Circuit breaker trips before cascading failure | `resilience/lib.rs` | `CircuitBreaker` | 3 tests |
| INV-012 | Rate limiter enforces token bucket | `resilience/lib.rs` | `RateLimiter` | 3 tests |

## Credential Flow (INV-001 + INV-002)

```
┌──────────────┐    ┌─────────────────────┐    ┌──────────────────┐
│ LLM proposes │    │ with_credential()   │    │ Tool executor    │
│ ToolCall     │───▶│ AuthenticatedCall   │───▶│ .expose_secret() │
│ (no secret)  │    │ .call (safe)        │    │ (only here!)     │
│              │    │ .credential:        │    │                  │
│              │    │   ProtectedSecret   │    │                  │
└──────────────┘    └─────────────────────┘    └──────────────────┘
     │                       │
     │  Serialize: ✅        │  Serialize: ❌ (compile error)
     │  Debug: ✅            │  Debug: [REDACTED]
     │  Trace: ✅            │  Trace: N/A
     ▼                       ▼
  Logs, spans,          Never appears in
  audit entries         any output ever
```

## Dependency Pins

| Must Pin | Version | Why |
|----------|---------|-----|
| `tokio` | `1.51` | LTS pin, not floating `"1"` |
| `rmcp` | `2.2` | `V_2025_11_25` only in 2.2+ |
| `wasmtime` | `46` | Fuel/memory/epoch APIs |
| `aes-gcm` | `0.11` | `AeadCore::generate_nonce()` API |
| `secrecy` | `0.10` | Uses `SecretBox<T>`, not old `Secret<T>` |
| `ed25519-dalek` | `3.0` | API stability |

| Must NOT Enable | Feature | Why |
|-----------------|---------|-----|
| `secrecy/serde` | On any crate | Gives `SecretString` a leaking `Serialize` impl everywhere |

## Path & Protocol Conventions

| Resource | Value | Spec |
|----------|-------|------|
| AgentCard | `/.well-known/agent-card.json` | A2A spec + RFC 8615 |
| MCP protocol | `V_2025_11_25` | Explicit pin — never `.LATEST` |

## Workspace Lints

```toml
[workspace.lints.rust]
unsafe_code = "forbid"
unused_must_use = "deny"
missing_docs = "warn"
```

## Phase Checklist

- [x] **Phase 1** — Trust Kernel, Agent Runtime, Policy, Audit, Sandbox, Guard
- [x] **Phase 2** — Observability (OTel + Prometheus + JSON stderr) + MCP stdio
- [x] **Phase 3** — Security pipeline + WASM sandbox + Memory + Plugin signing
- [x] **Phase 4** — Session + Supervisor (anomaly detection) + Semantic Memory + A2A
- [x] **Phase 5** — Resilience (circuit breaker, retry, timeout, rate limiter) + Health + Config validation

## Open Decisions

1. **A2A branch** — Ship Branch A (standalone AgentCard server), Branch B (MCP-integrated), or both?
2. **Supervisor architecture** — Anomaly detection (current) vs. ractor DAG (PR #3, closed)

## Verification Commands

```bash
cargo check --workspace
cargo test --workspace
cargo tree -e features -i secrecy
cargo clippy --workspace -- -D warnings
cargo doc --workspace --no-deps
```
