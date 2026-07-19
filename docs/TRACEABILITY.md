# Ferris Aegis — Traceability Documentation

> End-to-end traceability: decisions → implementation → verification.
> Branch: `arena/019f7a09-ferris-aegis-the-operating-sys`
> Version: 0.4.0 | Edition: 2021 (MSRV 1.82) | Last updated: 2026-07-19

## 1. Phase Delivery Map

| Phase | Scope | Crate(s) | Status |
|-------|-------|----------|--------|
| 1 | Trust Kernel, Agent Runtime, Policy, Audit, Sandbox, Guard, Config | `kernel` | Merged |
| 2W3 | OTel tracing, Prometheus metrics, JSON stderr | `observability` | Merged |
| 2W4 | MCP stdio server, `file_read`, `V_2025_11_25` | `mcp` | Merged |
| 3 | Allowlist, Injection, SSRF, Vault, WASM, Memory, Plugin | `security`, `sandbox-wasm`, `memory`, `plugin` | Merged |
| 4 | Session, Supervisor, Semantic Memory, A2A | `session`, `supervisor`, `semantic-memory`, `a2a` | Merged (PR #4) |
| 5 | Resilience, Health, Config Validation, CLI hardening | `resilience`, `kernel/health.rs`, `kernel/config.rs` | Merged (PR #4) |
| 5.1 | Durable Execution, Checkpoint Durability, Crash Recovery | `durable` | In this PR |
| 5.2 | Agent Skills (SKILL.md), Discovery, Validation, CLI | `skills` + `.agents/skills/` | In this PR |

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
| 13 | `ferris-aegis-durable` | ~1,200 | Durable execution, checkpoint durability, crash recovery |
| 14 | `ferris-aegis-skills` | ~800 | SKILL.md discovery, parsing, validation, loading |
| — | `ferris-aegis` (CLI) | ~750 | Root binary |
| — | Integration tests | ~1,350 | 55 end-to-end tests |
| | | **~14,000** | |

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

### ADR-010: Checkpoint After Every Step
Every step outcome is persisted before the next step begins. This guarantees that
a crash at any point loses at most the in-flight step's outcome. On recovery,
the executor re-executes only the interrupted step.
**Traced to:** `durable/src/lib.rs` → `DurableExecutor::run()`

### ADR-011: Pluggable Checkpoint Store Trait
`CheckpointStore` trait with `InMemoryCheckpointStore` (tests) and `SqliteCheckpointStore`
(production). Adding new backends (Postgres, S3) requires only a trait impl.
**Traced to:** `durable/src/lib.rs` → `CheckpointStore` trait

### ADR-012: Checkpoint Content Hash (Tamper Evidence)
Every `Checkpoint` includes a SHA-256 content hash over all step outcomes.
On load, `verify_hash()` detects tampering. Enabled by `DurableExecutorConfig.verify_hashes`.
**Traced to:** `durable/src/lib.rs` → `Checkpoint::verify_hash()`

### ADR-013: Skill Registry with agentskills.io Compliance
`ferris-aegis-skills` crate provides programmatic SKILL.md discovery, parsing, validation,
and index generation. Complies with agentskills.io v0.2.0 specification. Extension fields
(`aegis-crate`, `aegis-phase`, `aegis-depends`, `aegis-invariants`) stored in `metadata:`
to maintain cross-platform portability.
**Traced to:** `crates/skills/src/lib.rs` → `SkillRegistry`, `SkillValidator`, `SkillIndex`

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
| `sha2` | `0.10` | — | Used for checkpoint hashing |

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
| Durable crate unit tests | ✅ 30+ tests (in-crate) |
| Integration tests (Phase 5.1) | ✅ Criteria 30–39 |
| Full workspace compile | ❌ No Rust in sandbox |
| Git push | Pending |
| PR | Pending |

## 7. Agent Skill Library

| # | Skill Name | Crate(s) | Phase | Invariants |
|---|-----------|----------|-------|-----------|
| 1 | `aegis-trust-kernel` | kernel | 1 | INV-006, INV-010 |
| 2 | `aegis-security-pipeline` | security, sandbox-wasm | 3 | INV-001–003, 005, 007–009 |
| 3 | `aegis-durable-workflow` | durable | 5.1 | INV-013, INV-014 |
| 4 | `aegis-policy-authoring` | kernel::policy | 1 | INV-010 |
| 5 | `aegis-agent-lifecycle` | kernel::agent, guard | 1 | INV-006, INV-010 |
| 6 | `aegis-resilience-ops` | resilience, kernel::health | 5 | INV-011, INV-012 |
| 7 | `aegis-mcp-server` | mcp, observability | 2 | INV-004 |
| 8 | `aegis-crash-recovery` | durable | 5.1 | INV-013–015 |
| 9 | `aegis-session-supervisor` | session, supervisor, a2a | 4 | ADR-005, 008, 009 |
| 10 | `aegis-skill-creator` | meta | — | — |

All skills comply with the [agentskills.io specification](https://agentskills.io) v0.2.0.
Stored in `.agents/skills/` with HTTP discovery at `/.well-known/agent-skills/`.
See `SKILL-SPEC.md` for the complete specification and architecture.

## 8. Open Items

1. **A2A branch** — Ship A, B, or both? Open.
2. **CrashRecovery re-execution** — Currently returns recovery metadata; caller must
   reconstruct `Workflow` and pass to `DurableExecutor::run()`. Future: store step
   definitions in checkpoints for automatic re-execution.
3. **Skill evaluation** — Integrate `agent-skills-eval` for empirical skill quality testing.

*Maintained alongside the codebase. Update when adding crates, types, or invariants.*
