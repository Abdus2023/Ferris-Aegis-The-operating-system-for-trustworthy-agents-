# Ferris Aegis — Project Context for AI Agents

> This file provides project-level context for AI coding agents (Claude Code, Codex, Cursor, etc.)
> working on the Ferris Aegis operating system for trustworthy agents.

## Quick Reference

- **Language**: Rust (edition 2021, MSRV 1.82)
- **Workspace**: 14 crates in `crates/` + root CLI binary
- **Version**: 0.4.0
- **Lint**: `unsafe_code = "forbid"`, `unused_must_use = "deny"`, `missing_docs = "warn"`
- **License**: MIT OR Apache-2.0

## Crate Map

| Crate | Phase | Description |
|-------|-------|-------------|
| `ferris-aegis-kernel` | 1 | Trust, agent, policy, audit, sandbox, guard, config, health |
| `ferris-aegis-observability` | 2 | OTel tracing, Prometheus metrics, JSON stderr |
| `ferris-aegis-mcp` | 2 | MCP stdio server with `file_read` |
| `ferris-aegis-security` | 3 | Allowlist, injection, SSRF, credential vault |
| `ferris-aegis-sandbox-wasm` | 3 | WASM sandbox with fuel/memory/epoch |
| `ferris-aegis-memory` | 3 | SQLite episodic memory |
| `ferris-aegis-plugin` | 3 | Ed25519 manifest signing |
| `ferris-aegis-session` | 4 | Session with 4-field budget |
| `ferris-aegis-supervisor` | 4 | Anomaly detection oversight |
| `ferris-aegis-semantic-memory` | 4 | Concepts, embeddings, summaries |
| `ferris-aegis-a2a` | 4 | AgentCard + trust-gated routing |
| `ferris-aegis-resilience` | 5 | Circuit breaker, retry, timeout, rate limiter |
| `ferris-aegis-durable` | 5.1 | Durable execution, checkpoint durability |
| `ferris-aegis-skills` | 5.2 | SKILL.md discovery, parsing, validation |

## Critical Invariants

1. **Never enable `secrecy/serde`** — breaks `ProtectedSecret` across the workspace
2. **MCP protocol pinned to `V_2025_11_25`** — never `.LATEST`
3. **All observability to stderr** — MCP owns stdout
4. **AgentCard at `/.well-known/agent-card.json`** — not `agent.json`
5. **Tokio pinned to `1.51`** — not floating `"1"`
6. **Checkpoint hash verification** — every checkpoint has SHA-256 content hash

## Build & Test

```bash
cargo check --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

## Agent Skills

Skills are in `.agents/skills/` — 10 SKILL.md files compliant with agentskills.io v0.2.0.

The `ferris-aegis-skills` crate supports both v0.2.0 and v1.0.0 (vendor-neutral) formats.
Skills with `spec_version: "1.0.0"` in frontmatter use the richer format with IDs, permissions,
inputs/outputs, dependencies, and runtime extension blocks.

Vendor-neutral example skills are in `skills/examples/`.

```bash
aegis skills list       # List discovered skills
aegis skills validate   # Validate all skills
aegis skills show NAME  # Show skill details
aegis skills index      # Generate discovery index JSON
aegis skill run <id>    # Run a skill with inputs (v1.0.0)
aegis skill sign <id>   # Sign a skill with Ed25519 (v1.0.0)
aegis skill verify <id> # Verify a skill's signature (v1.0.0)
```

## Documentation

- `docs/TRACEABILITY.md` — End-to-end traceability: decisions → implementation → verification
- `docs/ARCHITECTURE-QUICK-REF.md` — Security invariant reference card
- `docs/PHASE-DELIVERY-RECORD.md` — Per-phase delivery record
- `SKILL_STANDARD_SPECIFICATION.md` — **Vendor-neutral SKILL.md spec (v1.0.0, 10-layer architecture)**
- `SKILL_AEGIS_IMPLEMENTATION.md` — Ferris Aegis reference implementation guide (7-layer execution model)
- `SKILL_ECOSYSTEM_SUMMARY.md` — Ecosystem overview (specs, skills, crate, observability)
- `SKILL-SPEC.md` — Original Aegis-specific spec (agentskills.io v0.2.0, backward compat)
- `skills/examples/` — 3 vendor-neutral example skills (research-planner, web-search, code-reviewer)

## Compile-Fix History

When adding new crates, watch for:
- `secrecy 0.10` uses `SecretBox<T>`, not `Secret<T>`
- `aes-gcm 0.11` uses `AeadCore::generate_nonce()`
- `Session` must derive `Clone`
- `ProtectedSecret` newtype prevents serde unification attacks
