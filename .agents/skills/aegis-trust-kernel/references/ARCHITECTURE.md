# Ferris Aegis — Architecture Reference

> Loaded on demand by skills that need architectural context.

## Crate Map (13 crates)

| # | Crate | Lines | Phase |
|---|-------|-------|-------|
| 1 | `ferris-aegis-kernel` | 3,556 | 1 |
| 2 | `ferris-aegis-observability` | 290 | 2W3 |
| 3 | `ferris-aegis-mcp` | 343 | 2W4 |
| 4 | `ferris-aegis-security` | 1,098 | 3 |
| 5 | `ferris-aegis-sandbox-wasm` | 362 | 3 |
| 6 | `ferris-aegis-memory` | 421 | 3 |
| 7 | `ferris-aegis-plugin` | 334 | 3 |
| 8 | `ferris-aegis-session` | 289 | 4 |
| 9 | `ferris-aegis-supervisor` | 458 | 4 |
| 10 | `ferris-aegis-semantic-memory` | 629 | 4 |
| 11 | `ferris-aegis-a2a` | 1,287 | 4 |
| 12 | `ferris-aegis-resilience` | 1,046 | 5 |
| 13 | `ferris-aegis-durable` | ~1,200 | 5.1 |

## Dependency Graph

```
kernel ← security, session, supervisor, a2a, resilience, durable
security ← sandbox-wasm, memory, plugin
observability ← mcp
resilience ← kernel::health
durable ← (standalone, depends on sqlx)
session ← (standalone)
supervisor ← session
semantic-memory ← (standalone, depends on sqlx)
a2a ← session, supervisor, semantic-memory
```

## Security Invariant Index

| ID | Invariant | Crate |
|----|-----------|-------|
| INV-001 | Credentials cannot reach LLM context | security |
| INV-002 | ProtectedSecret cannot be serialized | security |
| INV-003 | No secrecy/serde feature enabled | workspace |
| INV-004 | Observability writes stderr only | observability |
| INV-005 | Tool allowlist is deny-by-default | security |
| INV-006 | Audit ledger is tamper-evident | kernel |
| INV-007 | WASM execution terminates on fuel exhaustion | sandbox-wasm |
| INV-008 | SSRF guard blocks private IPs | security |
| INV-009 | Plugins must be Ed25519-signed | plugin |
| INV-010 | Config validated before use | kernel |
| INV-011 | Circuit breaker trips before cascading failure | resilience |
| INV-012 | Rate limiter enforces token bucket | resilience |
| INV-013 | Checkpoints verify content hash on load | durable |
| INV-014 | Every step writes a checkpoint before proceeding | durable |
| INV-015 | Crash recovery resumes from last checkpoint | durable |

## Workspace Lints

```toml
[workspace.lints.rust]
unsafe_code = "forbid"
unused_must_use = "deny"
missing_docs = "warn"
```
