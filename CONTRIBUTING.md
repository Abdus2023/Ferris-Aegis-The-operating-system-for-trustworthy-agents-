# Contributing to Ferris Aegis

**Version**: 0.3.0  
**Status**: All 5 phases declared complete — focus now on **hardening, completeness, and production readiness**

Thank you for your interest in contributing to Ferris Aegis — the Rust Guardian for Autonomous Intelligence.

This document outlines contribution guidelines, current implementation gaps, and high-impact opportunities based on the formal specification.

---

## 1. Contribution Philosophy

Ferris Aegis follows strict principles derived from its formal specification:

- **Safety First**: Every change must preserve the 12 Security Invariants (see `SPECIFICATION.md`).
- **Type Safety & Compile-Time Guarantees**: Prefer structural solutions (`ProtectedSecret`, capability tokens) over runtime checks.
- **Observability Contract**: Never write to stdout except through the MCP layer.
- **Test Everything**: All new features must include unit + integration tests.
- **Documentation**: Changes must be reflected in `SPECIFICATION.md` and/or `DIAGRAMS.md`.

---

## 2. Current Implementation Gaps (High-Priority Opportunities)

Based on code review and the formal specification, the following areas are **partially implemented** or **stubbed**:

### 2.1 CLI Completeness (High Impact)

Many commands in `src/main.rs` are currently stubs:

| Command                        | Current State     | Opportunity                                      | Difficulty |
|--------------------------------|-------------------|--------------------------------------------------|----------|
| `aegis agent list`             | Stub              | Real agent registry + status display             | Medium   |
| `aegis security vault-store`   | Stub              | Interactive secret input + encryption            | Medium   |
| `aegis audit`                  | Stub              | Query + pretty-print from `AuditLedger`          | Medium   |
| `aegis memory record/recent`   | In-memory only    | Persistent SQLite backend                        | Medium   |
| `aegis start` (daemon mode)    | `--foreground` only | Proper daemonization + signal handling         | Hard     |

**Contribution Opportunity**: Implement real persistence and output for these commands.

### 2.2 A2A Protocol (Open Decision)

- **Branch A** (standalone AgentCard server at `/.well-known/agent-card.json`) is implemented but not wired into the CLI or main binary.
- **Branch B** (MCP-integrated) is the current default path.

**Opportunity**:
- Decide on (or support both) deployment modes
- Add CLI command: `aegis a2a serve` (Branch A)
- Add MCP tools for A2A capabilities (Branch B)

### 2.3 WASM Plugin System

- `crates/sandbox-wasm` and `crates/plugin` exist with fuel/memory/epoch limits and Ed25519 verification.
- **Gap**: Real plugin loading, manifest resolution, and execution inside `SkillExecutor` or `AgentRuntime` is not yet integrated in production flows.

**Opportunity**: Wire WASM execution into the skill execution path with proper attestation.

### 2.4 Persistence Layer

Several components use in-memory or stub storage:

- Episodic Memory (`crates/memory`)
- Audit Ledger (currently in-memory only)
- Semantic Memory
- Session state

**Opportunity**: Implement production-grade SQLite persistence with proper migrations and encryption-at-rest for sensitive data.

### 2.5 Daemonization & Production Hardening

- `aegis start --foreground` works, but background daemon mode is missing.
- No PID file, log rotation, or graceful shutdown handling.

### 2.6 Testing & Integration

- 38 integration tests exist, but coverage is uneven across phases.
- Missing: End-to-end tests involving `Guard` + `A2A` + `Resilience` together.

---

## 3. Contribution Workflow

1. **Fork** the repository
2. **Create a feature branch** from `main`
3. **Implement** your change while preserving all security invariants
4. **Add tests** (unit + integration)
5. **Update documentation**:
   - `SPECIFICATION.md` (if architecture changes)
   - `DIAGRAMS.md` (if new flows are introduced)
   - `CONTRIBUTING.md` (if new contribution patterns emerge)
6. **Run**:
   ```bash
   cargo test --workspace
   cargo clippy --workspace -- -D warnings
   cargo doc --workspace --no-deps
   ```
7. **Open a Pull Request** with a clear description referencing the relevant section of the spec.

---

## 4. Priority Contribution Areas (Ranked)

| Priority | Area                        | Impact     | Notes |
|----------|-----------------------------|------------|-------|
| 1        | CLI command completion      | High       | Immediate usability improvement |
| 2        | A2A Branch A server         | High       | Unblocks external agent discovery |
| 3        | WASM plugin integration     | High       | Completes Phase 3 vision |
| 4        | Persistent storage          | High       | Required for production |
| 5        | Daemon mode                 | Medium     | Production deployment |
| 6        | Expanded test coverage      | Medium     | Especially cross-phase integration |
| 7        | Documentation & examples    | Medium     | Helps onboard new contributors |

---

## 5. Code Style & Linting

- Workspace lints are enforced:
  ```toml
  unsafe_code = "forbid"
  unused_must_use = "deny"
  missing_docs = "warn"
  ```
- All public APIs must be documented.
- Prefer `thiserror` for error types.
- Use `tracing` for all logging (never `println!`).

---

## 6. Security Review Process

Any PR that touches the following must receive extra scrutiny:

- `ProtectedSecret` / `CredentialVault`
- `AuditLedger` hashing logic
- `Sandbox` / `Guard`
- `PolicyEngine`
- `A2aRouter` trust checks

Contributors are expected to add or update tests that verify the relevant security invariant.

---

## 7. Getting Started (Good First Issues)

Look for issues labeled `good-first-issue` or areas marked as "Stub" in `src/main.rs`.

Recommended starting points:
- Implement `aegis agent list` using the existing `AgentRuntime`
- Add pretty-printing to `aegis audit`
- Wire the existing `WasmSandbox` into `SkillExecutor`

---

## 8. Questions & Discussion

- Open an issue with the label `question` or `discussion`
- Reference the relevant section in `SPECIFICATION.md` or `A2A_AND_RESILIENCE_SPEC.md`

---

**Thank you for helping make autonomous agents trustworthy by design.**

*Maintained alongside the formal specification.*