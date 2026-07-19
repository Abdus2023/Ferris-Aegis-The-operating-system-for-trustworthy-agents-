---
name: aegis-agent-lifecycle
description: >
  Manages the full agent lifecycle in Ferris Aegis: spawning, suspending, resuming,
  terminating, and guarding agents. Use when the user says "spawn agent", "agent
  lifecycle", "quarantine agent", "guard action", "agent status", or "suspend agent".
  Do NOT use for policy authoring or durable workflows.
license: "MIT OR Apache-2.0"
compatibility: Requires Rust 1.82+ and ferris-aegis-kernel crate
metadata:
  aegis-crate: "ferris-aegis-kernel"
  aegis-phase: "1"
  aegis-depends: "aegis-trust-kernel"
  aegis-invariants: "INV-006 INV-010"
  version: "0.4.0"
  author: "ferris-aegis"
  tags: "agent spawn suspend resume terminate guard quarantine"
allowed-tools: Bash(cargo:*) Read Write
---

# Ferris Aegis — Agent Lifecycle

Manage the full agent lifecycle: spawn → run → suspend → resume → quarantine → terminate.

## When to Use

- Spawning a new agent with name and version
- Suspending or resuming agents
- Checking if the Guard should intervene
- Quarantining agents with low trust scores
- Terminating misbehaving agents

## Agent Lifecycle

```
             ┌──────────┐
    spawn ──▶│  Active   │◀── resume
             └────┬─────┘
                  │ suspend
                  ▼
             ┌──────────┐
             │ Suspended │
             └────┬─────┘
                  │
   guard check ───┤
                  ▼
             ┌────────────┐
             │ Quarantined │──▶ terminate
             └────────────┘
```

## Guard Escalation Ladder

```
Alert → Throttle → Quarantine → Terminate
```

The Guard monitors trust scores and violation rates. When an agent crosses thresholds, the Guard recommends escalating actions.

## Workflow

1. Create `AgentRuntime::new(kernel, policy_engine)`
2. Spawn agent: `runtime.spawn(name, version).await`
3. Register with Guard: `guard.register_agent(&agent_id)`
4. Monitor trust score and guard actions
5. If Guard recommends Quarantine: `runtime.quarantine(&agent_id).await`
6. Terminate if needed: `runtime.terminate(&agent_id).await`

## Code Pattern

```rust
use ferris_aegis_kernel::{
    agent::AgentRuntime,
    guard::{Guard, GuardAction},
    kernel::TrustKernel,
    policy::PolicyEngine,
};

let kernel = TrustKernel::new();
let policy = PolicyEngine::with_defaults();
let mut runtime = AgentRuntime::new(kernel, policy);
let mut guard = Guard::new();

// Spawn
let agent_id = runtime.spawn("data-agent", "1.0.0").await?;
guard.register_agent(&agent_id);

// Build trust
runtime.trust_kernel_mut().reinforce(&agent_id, 0.1);

// Check if guard should intervene
if let Some(action) = guard.check_trust(&agent_id, runtime.trust_kernel()) {
    match action {
        GuardAction::Quarantine => {
            runtime.quarantine(&agent_id).await?;
        }
        GuardAction::Terminate => {
            runtime.terminate(&agent_id).await?;
        }
        _ => {}
    }
}
```

## Agent Status Values

| Status | Meaning |
|--------|---------|
| `Active` | Agent is running and can perform actions |
| `Suspended` | Agent is paused, cannot act |
| `Quarantined` | Agent isolated due to trust violations |
| `Terminated` | Agent permanently stopped |

## Invariants

- **INV-006**: All agent actions are recorded in the audit ledger
- **INV-010**: Runtime uses validated config for trust thresholds

## Edge Cases

- Cannot spawn two agents with the same ID
- Suspend/resume only works on Active/Suspended agents
- Quarantine is irreversible — agent must be terminated after
