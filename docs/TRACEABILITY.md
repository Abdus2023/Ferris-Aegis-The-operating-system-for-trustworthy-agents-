# Ferris Aegis — Traceability Documentation

> End-to-end traceability: decisions → implementation → verification.
> Branch: `arena/019f7994-ferris-aegis-the-operating-sys`
> Version: 0.3.0 | Edition: 2021 (MSRV 1.82) | Last updated: 2026-07-19

## 1. Phase Delivery Map

| Phase | Scope | Crate(s) | Status |
|-------|-------|----------|--------|
| 1 | Trust Kernel, Agent Runtime, Policy, Audit, Sandbox, Guard, Config | `kernel` | Merged |
| 2W3 | OTel tracing, Prometheus metrics, JSON stderr | `observability` | Merged |
| 2W4 | MCP stdio server, `file_read`, `V_2025_11_25` | `mcp` | Merged |
| 3 | Allowlist, Injection, SSRF, Vault, WASM, Memory, Plugin | `security`, `sandbox-wasm`, `memory`, `plugin` | Merged |
| 4 | Session, Supervisor, Semantic Memory, A2A | `session`, `supervisor`, `semantic-memory`, `a2a` | In PR #4 |
| 5 | Resilience, Health, Config Validation, CLI hardening | `resilience`, `kernel/health.rs`, `kernel/config.rs` | In PR #4 |

## 2. Crate Inventory

| # | Crate | Lines | Description |
|---|-------|-------|-------------|
| 1 | `ferris-aegis-kernel` | 3,556 | Trust kernel, agent runtime, policy, audit, sandbox, guard, config, health |
| 2 | `ferris-aegis-observability` | 290 | OTel tracing, Prometheus metrics, JSON stderr |
| 3 | `ferris-aegis-mcp` | 343 | MCP stdio server with `file_read` |
| 4 | `ferris-aegis-security` | 1,098 | Allowlist, injection scanner, SSRF guard, credential vault |
| 5 | `ferris-aegis-sandbox-wasm` | 362 | WASM sandbox with fuel/memory/epoch |
| 6 | `ferris-aegis-memory` | 421 | SQLite episodic memory |
| 7 | `ferris-aegis-plugin` | 334 | Ed25519 manifest signing |
| 8 | `ferris-aegis-session` | 289 | Session with 4-field budget |
| 9 | `ferris-aegis-supervisor` | 458 | Anomaly detection oversight |
| 10 | `ferris-aegis-semantic-memory` | 629 | Concepts, embeddings, summaries |
| 11 | `ferris-aegis-a2a` | 1,287 | AgentCard + trust-gated routing + Branch A/B |
| 12 | `ferris-aegis-resilience` | 1,046 | Circuit breaker, retry, timeout, rate limiter, health registry |
| — | `ferris-aegis` (CLI) | 593 | Root binary |
| — | Integration tests | 882 | 38 end-to-end tests |
| | | **11,758** | |

## 3. Architectural Decision Records

### ADR-001: `unsafe_code = "forbid"`
Workspace-level lint. No crate can introduce undefined behavior.
**Traced to:** `Cargo.toml` → `[workspace.lints.rust]`

### ADR-002: `ProtectedSecret` Newtype
Local newtype over `SecretString`, no `Serialize`/`Deserialize`. Defense against `secrecy/serde` feature unification.
**Traced to:** `crates/security/src/vault.rs` line 217, `security/lib.rs` line 34

### ADR-003: `AuthenticatedCall` Separates Call from Credential
`call: &'a ToolCall` (trace freely) + `credential: Option<ProtectedSecret>` (never serializable).
**Traced to:** `vault.rs` lines 151-163, criteria 5/8/18

### ADR-004: MCP Protocol `V_2025_11_25` Explicitly Pinned
Never `.LATEST`. **Traced to:** `crates/mcp/Cargo.toml` and `lib.rs`

### ADR-005: AgentCard at `/.well-known/agent-card.json`
Per A2A spec + RFC 8615. NOT `/.well-known/agent.json`.
**Traced to:** `a2a/src/agent_card.rs` → `AGENT_CARD_PATH`

### ADR-006: tokio `1.51` LTS Pin
Not floating `"1"`. **Traced to:** Workspace `Cargo.toml`

### ADR-007: Observability stderr Only, Batch Export
MCP owns stdout. `with_writer(std::io::stderr)`. OTel uses `install_batch(Tokio)`.
**Traced to:** `observability/src/lib.rs`

### ADR-008: A2A Fork — Branch A (standalone) + Branch B (MCP)
Both implemented. Choice remains open.
**Traced to:** `a2a/src/branch_a.rs`, `a2a/src/branch_b.rs`

### ADR-009: Supervisor — Anomaly Detection
Rate, trust decay, context drift monitoring with recommendations. Not ractor DAG.
**Traced to:** `supervisor/src/lib.rs`

## 4. Dependency Version Traceability

| Crate | Pinned | Originally | Corrected? |
|-------|--------|-----------|------------|
| `tokio` | `1.51` | `"1"` | Yes |
| `rmcp` | `2.2` | `0.16.0` | Yes |
| `wasmtime` | `46` | `24` | Yes |
| `sqlx` | `0.9` | `0.8` | Yes |
| `aes-gcm` | `0.11` | `0.10` | Yes |
| `secrecy` | `0.10` | `0.10` | No |
| `ed25519-dalek` | `3.0` | unpinned | Yes |
| `schemars` | `0.8` | — | Added for A2A |

## 5. Compile-Fix History

| # | Symptom | Fix |
|---|---------|-----|
| 1 | `Secret` not found in secrecy 0.10 | → `SecretBox<T>` |
| 2 | `rand_nonce` module not found in aes-gcm 0.11 | → `Aes256Gcm::generate_nonce()` via `AeadCore` |
| 3 | `SecretString::new(String)` type error | → `.into()` |
| 4 | Missing `schemars` in a2a | → added to Cargo.toml |
| 5 | `Session` missing `Clone` | → `#[derive(Clone)]` |
| 6 | `AuthenticatedCall.credential: SecretString` | → `ProtectedSecret` |

## 6. Verification Status

| Item | Status |
|------|--------|
| Vault tests (external) | ✅ 9 pass |
| Full workspace compile | ❌ No Rust in sandbox |
| Git push | ✅ At `c84413d` |
| PR #4 | ✅ OPEN, 4 commits |

## 7. Open Items

1. **A2A branch** — Ship A, B, or both? Open.
2. **PR #3** — Closed (ractor supervisor, `SecretString` vault). Superseded by #4.
3. **Merge PR #4** — Awaiting review.

*Maintained alongside the codebase. Update when adding crates, types, or invariants.*
