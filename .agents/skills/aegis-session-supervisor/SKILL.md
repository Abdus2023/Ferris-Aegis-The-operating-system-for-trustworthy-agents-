---
name: aegis-session-supervisor
description: >
  Manages Ferris Aegis conversation sessions, supervisor anomaly detection, and
  Agent-to-Agent (A2A) protocol routing. Use when the user says "session management",
  "session budget", "supervisor anomaly", "A2A routing", "AgentCard", "trust-gated
  routing", or "semantic memory". Do NOT use for trust scoring or security pipeline.
license: "MIT OR Apache-2.0"
compatibility: Requires Rust 1.82+, ferris-aegis-session, ferris-aegis-supervisor, ferris-aegis-a2a crates
metadata:
  aegis-crate: "ferris-aegis-session ferris-aegis-supervisor ferris-aegis-a2a"
  aegis-phase: "4"
  aegis-depends: "aegis-trust-kernel"
  aegis-invariants: "ADR-005 ADR-008 ADR-009"
  version: "0.4.0"
  author: "ferris-aegis"
  tags: "session supervisor a2a agentcard routing anomaly"
allowed-tools: Bash(cargo:*) Read Write
---

# Ferris Aegis — Session, Supervisor & A2A

Manage conversation sessions, detect anomalies, and route Agent-to-Agent messages.

## When to Use

- Creating and managing multi-turn conversation sessions
- Detecting rate anomalies, trust decay, or context drift
- Building AgentCard for A2A protocol
- Routing messages between agents with trust-level gating

## Architecture

```
Session (4-field budget: tokens, cost, rounds, wall-clock)
   │
   ├── SessionManager ─── tracks sessions per agent
   │
Supervisor (anomaly detection)
   │
   ├── Rate anomaly:    turns/minute exceeds threshold
   ├── Trust decay:     score drops below threshold
   ├── Context drift:   semantic deviation (placeholder)
   │
   ├── Recommendations: Log → Notify → Suspend → Quarantine → Terminate
   │
A2A Protocol
   │
   ├── AgentCard:       /.well-known/agent-card.json (ADR-005)
   ├── Task lifecycle:  Submitted → Working → Completed/Failed
   ├── Branch A:        Standalone AgentCard server
   ├── Branch B:        MCP-integrated tool params
   └── A2aRouter:       Trust-gated, skill-based discovery
```

## Workflow — Session

1. Create session: `Session::new(agent_id, context)`
2. Advance turns: `session.advance_turn()`
3. Check budget: session automatically deactivates when exhausted
4. Serialize: `serde_json::to_string(&session)` for persistence/A2A

## Workflow — Supervisor

1. Create: `Supervisor::with_defaults()` or `Supervisor::new(config)`
2. Feed session data: `supervisor.check_session(&session)`
3. Read findings: `supervisor.findings()`
4. Take action based on severity and recommendations

## Workflow — A2A

1. Create AgentCard: `default_aegis_card()` or `AgentCardBuilder`
2. Set up router: `A2aRouter::new()` with registry
3. Register agents with trust levels and skills
4. Route messages: `router.route_message(sender, target, message)`

## Code Pattern — Session + Supervisor

```rust
use ferris_aegis_session::{Session, SessionManager};
use ferris_aegis_supervisor::{Supervisor, SupervisorConfig, Severity};

let mut session = Session::new("agent-1", "research");
session.advance_turn();

let supervisor = Supervisor::with_defaults();
// Check session for anomalies...
let findings = supervisor.findings();
```

## Code Pattern — A2A Routing

```rust
use ferris_aegis_a2a::{A2aRouter, AgentCard, default_aegis_card};

let card = default_aegis_card();
let router = A2aRouter::new();
// Register agent with card and trust level
// Route messages with trust-gated filtering
```

## Invariants

- **ADR-005**: AgentCard served at `/.well-known/agent-card.json` (RFC 8615), NOT `/.well-known/agent.json`
- **ADR-008**: A2A has two branches (A: standalone, B: MCP). Both implemented.
- **ADR-009**: Supervisor uses anomaly detection, NOT ractor DAG

## Edge Cases

- Session budget exhausted → session auto-deactivates
- Supervisor with no sessions → empty findings
- A2A router with no matching agents → `RouteError`
