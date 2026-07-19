---
name: aegis-trust-kernel
description: >
  Manages agent trust scores, trust levels, and audit ledger integrity in Ferris Aegis.
  Use when the user mentions "trust score", "trust level", "reinforce trust",
  "penalize agent", "audit ledger", "trust lifecycle", "TrustKernel", or
  "audit chain verification". Do NOT use for policy rules or agent spawning.
license: "MIT OR Apache-2.0"
compatibility: Requires Rust 1.82+ and the ferris-aegis-kernel crate
metadata:
  aegis-crate: "ferris-aegis-kernel"
  aegis-phase: "1"
  aegis-invariants: "INV-006 INV-010"
  version: "0.4.0"
  author: "ferris-aegis"
  tags: "trust kernel audit ledger verification"
allowed-tools: Bash(cargo:*) Read Write
---

# Ferris Aegis — Trust Kernel

Manage agent trust scores, trust levels, and audit ledger integrity.

## When to Use

- Computing or adjusting agent trust scores
- Checking trust level thresholds (Unverified → Probationary → Standard → Elevated → Sovereign)
- Verifying audit ledger chain integrity
- Configuring trust parameters (initial_score, decay_factor)

## Trust Level Thresholds

| Level | Score Range | Key Capabilities |
|-------|------------|-----------------|
| Unverified | 0.00–0.19 | Timer, Inter-agent comm |
| Probationary | 0.20–0.49 | + Filesystem read |
| Standard | 0.50–0.74 | + Network, Environment, Audit |
| Elevated | 0.75–0.94 | + Filesystem write, Process spawn, Crypto |
| Sovereign | 0.95–1.00 | All capabilities |

## Workflow

1. Initialize `TrustKernel` with config from `AegisConfig`
2. Register agent with `kernel.register(&agent_id)`
3. Reinforce trust with `kernel.reinforce(&agent_id, delta)` — positive actions
4. Penalize with `kernel.penalize(&agent_id, delta)` — policy violations
5. Read trust record with `kernel.get_record(&agent_id)`
6. Verify audit chain with `ledger.verify_chain()`

## Code Pattern

```rust
use ferris_aegis_kernel::{
    kernel::{TrustKernel, TrustLevel, TrustScore},
    audit::{AuditLedger, AuditSeverity},
    config::AegisConfig,
};

let config = AegisConfig::default_config();
let mut kernel = TrustKernel::new()
    .with_initial_score(config.trust.initial_score)
    .with_decay_factor(config.trust.decay_factor);

let agent_id = AgentId::new("my-agent");
kernel.register(&agent_id);

// Build trust through positive actions
for _ in 0..10 {
    kernel.reinforce(&agent_id, 0.05);
}

// Check level
let record = kernel.get_record(&agent_id).unwrap();
assert!(record.score.value() >= 0.5);
assert_eq!(record.level, TrustLevel::Standard);

// Audit every action
ledger.append(agent_id.clone(), "action:good", "target:x", true, AuditSeverity::Info);

// Verify chain integrity (INV-006)
assert!(ledger.verify_chain());
```

## Invariants

- **INV-006**: Audit ledger is tamper-evident. Always call `verify_chain()` after operations.
- **INV-010**: Config must validate before use. Call `config.validate()` before passing to kernel.

## Edge Cases

- Trust score clamped to [0.0, 1.0] — reinforce/penalize never exceed bounds
- Decay is applied per interval, not per call
- Empty ledger `verify_chain()` returns `true` (vacuous truth)
