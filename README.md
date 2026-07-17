<p align="center">
  <img src="assets/aegis-logo.svg" alt="Ferris Aegis" width="200"/>
</p>

<h1 align="center">Ferris Aegis</h1>

<p align="center">
  <strong>The Rust Guardian for Autonomous Intelligence</strong>
</p>

<p align="center">
  An operating system for trustworthy agents — where safety is not a feature, it's the foundation.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/rust-1.75%2B-orange" alt="Rust 1.75+"/>
  <img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue" alt="License"/>
  <img src="https://img.shields.io/badge/status-active-brightgreen" alt="Status"/>
</p>

---

## 🛡️ What is Ferris Aegis?

Ferris Aegis is a Rust-powered operating system framework for building, running, and monitoring **autonomous AI agents** with strong guarantees around:

- **Safety** — Agents can only do what they're explicitly permitted to do
- **Auditability** — Every action is recorded in a tamper-evident cryptographic ledger
- **Trust** — Agents earn trust through good behavior and lose it through violations
- **Isolation** — Capability-based sandboxes enforce the principle of least privilege
- **Oversight** — Real-time monitoring detects and intervenes when agents go rogue

> *"With great autonomy comes great responsibility. Ferris Aegis ensures that responsibility is enforced."*

## 🏗️ Architecture

Ferris Aegis is built around **six core pillars**:

```
┌─────────────────────────────────────────────────────────┐
│                     FERRIS AEGIS                        │
├─────────────┬───────────┬───────────┬──────────────────┤
│             │           │           │                  │
│  ┌─────┐   │  ┌─────┐  │  ┌─────┐  │  ┌──────────┐   │
│  │Trust│   │  │Agent│  │  │Policy│  │  │  Audit   │   │
│  │Kernel│  │  │Run- │  │  │Engine│  │  │  Ledger  │   │
│  │     │   │  │time │  │  │      │  │  │          │   │
│  └─────┘   │  └─────┘  │  └─────┘  │  └──────────┘   │
│             │           │           │                  │
│  ┌─────┐   │  ┌─────┐  │           │                  │
│  │Sand │   │  │Guard│  │           │                  │
│  │box  │   │  │     │  │           │                  │
│  └─────┘   │  └─────┘  │           │                  │
│             │           │           │                  │
├─────────────┴───────────┴───────────┴──────────────────┤
│                    Configuration                        │
└─────────────────────────────────────────────────────────┘
```

### 1. 🏛️ Trust Kernel
The foundational layer that establishes and maintains trust relationships. Every agent receives a **trust score** (0.0–1.0) that determines its **trust level** (Unverified → Probationary → Standard → Elevated → Sovereign). Trust is earned through positive reinforcement and lost through penalties, with built-in time-based decay.

### 2. 🤖 Agent Runtime
Manages the complete lifecycle of autonomous agents: spawning, execution, suspension, resumption, and termination. Every state transition is tracked, and agents can be **quarantined** by the Guard when they pose a threat.

### 3. 📜 Policy Engine
Declarative policy definition and enforcement. Policies are written in TOML and govern what agents can see, do, and communicate. Higher-priority policies override lower ones, and the default is **deny** — agents start with nothing and must be granted access explicitly.

### 4. 📋 Audit Ledger
An append-only, **cryptographically chained** ledger recording every significant agent action. Each entry includes a SHA-256 hash linked to the previous entry, creating a tamper-evident chain. The entire ledger can be verified for integrity at any time.

### 5. 🏖️ Sandbox
Capability-based isolation boundaries that constrain agent execution environments. There is **no ambient authority** — every capability must be explicitly granted. Sandboxes are automatically configured based on trust level.

### 6. 🛡️ Guard
Real-time monitoring, anomaly detection, and intervention system. The Guard watches for:
- Excessive action rates
- Trust score deterioration
- Policy violation spikes
- Resource overconsumption
- Idle agents (possible deadlocks)

When anomalies are detected, the Guard can **alert**, **throttle**, **quarantine**, or **terminate** agents.

## 🚀 Quick Start

### Prerequisites
- Rust 1.75 or later

### Install

```bash
git clone https://github.com/Abdus2023/Ferris-Aegis-The-operating-system-for-trustworthy-agents-.git
cd Ferris-Aegis-The-operating-system-for-trustworthy-agents-
cargo build --release
```

### Initialize

```bash
# Create default configuration and policies
./target/release/aegis init

# View the generated config
cat aegis.toml

# View the default safety policy
cat policies/default-safety.toml
```

### Start the Daemon

```bash
# Run in foreground mode
./target/release/aegis start --foreground
```

### Manage Agents

```bash
# Spawn a new agent
./target/release/aegis agent spawn my-agent

# List all agents
./target/release/aegis agent list

# Suspend an agent
./target/release/aegis agent suspend <agent-id>

# Resume an agent
./target/release/aegis agent resume <agent-id>

# Terminate an agent
./target/release/aegis agent terminate <agent-id>
```

### Manage Policies

```bash
# List active policies
./target/release/aegis policy list

# View the default safety policy
./target/release/aegis policy default

# Load a custom policy
./target/release/aegis policy load policies/custom.toml
```

### Inspect the Audit Ledger

```bash
# Show recent entries
./target/release/aegis audit --last 50

# Verify ledger integrity
./target/release/aegis verify
```

## 📦 Library Usage

Use Ferris Aegis as a library in your own Rust projects:

```rust
use ferris_aegis::{
    kernel::TrustKernel,
    agent::AgentRuntime,
    policy::PolicyEngine,
    audit::AuditLedger,
    sandbox::Sandbox,
    guard::Guard,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize core components
    let kernel = TrustKernel::new();
    let policy = PolicyEngine::with_defaults();
    let mut runtime = AgentRuntime::new(kernel, policy);
    let mut ledger = AuditLedger::new();
    let mut guard = Guard::new();

    // Spawn an agent
    let agent_id = runtime.spawn("my-agent", "1.0.0").await?;

    // Register with guard
    guard.register_agent(&agent_id);

    // Check policy before an action
    let verdict = runtime.policy_engine()
        .evaluate("file:read", "/workspace/data.txt");
    if verdict.is_allowed() {
        // Action is permitted — proceed
        ledger.append(
            agent_id.clone(),
            "file:read".to_string(),
            "/workspace/data.txt".to_string(),
            true,
            ferris_aegis::audit::AuditSeverity::Info,
        );
    }

    // Build trust through good behavior
    runtime.trust_kernel_mut()
        .reinforce(&agent_id, 0.1);

    Ok(())
}
```

## 📜 Policy Format

Policies are defined in TOML:

```toml
[policy]
name = "my-policy"
version = "1.0.0"
priority = 100
enabled = true
default_effect = "deny"

[[rules]]
action = "file:read"
effect = "allow"
targets = ["/workspace/*"]
description = "Allow reads from workspace"

[[rules]]
action = "file:write"
effect = "deny"
targets = ["/etc/*", "/var/*"]
description = "Deny writes to system directories"

[[rules]]
action = "network:connect"
effect = "allow"
targets = ["api.openai.com:443"]
description = "Allow connections to OpenAI API"
```

## 🔐 Trust Levels

| Level | Score Range | Capabilities | Description |
|-------|-------------|-------------|-------------|
| 🔴 Unverified | 0.00–0.19 | Timer, Inter-agent comm | No trust established |
| 🟡 Probationary | 0.20–0.49 | + Filesystem read | Under observation |
| 🟢 Standard | 0.50–0.74 | + Network, Environment, Audit | Production-ready |
| 🔵 Elevated | 0.75–0.94 | + Filesystem write, Process spawn, Crypto | Proven track record |
| 🟣 Sovereign | 0.95–1.00 | All capabilities | System-critical agents |

## 🛡️ Guard Actions

| Action | Trigger | Effect |
|--------|---------|--------|
| Alert | Action rate exceeded | Warning logged |
| Throttle | Action rate significantly exceeded | Agent slowed down |
| Quarantine | Trust score critical / violation spike | Capabilities stripped, agent suspended |
| Terminate | Severe threat | Agent immediately killed |

## 🧪 Testing

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific module tests
cargo test kernel
cargo test policy
cargo test audit
```

## 📂 Project Structure

```
├── src/
│   ├── lib.rs          # Library root & prelude
│   ├── kernel.rs       # Trust Kernel
│   ├── agent.rs        # Agent Runtime
│   ├── policy.rs       # Policy Engine
│   ├── audit.rs        # Audit Ledger
│   ├── sandbox.rs      # Sandbox Manager
│   ├── guard.rs        # Guard (monitoring)
│   ├── config.rs       # Configuration
│   └── bin/
│       └── aegis.rs    # CLI binary
├── examples/
│   └── sentinel.rs     # Example agent
├── policies/
│   ├── default-safety.toml
│   └── sovereign.toml
├── tests/
│   └── integration.rs  # Integration tests
├── assets/
│   └── aegis-logo.svg
└── Cargo.toml
```

## 🤝 Contributing

We welcome contributions! Ferris Aegis is built on the principle that trustworthy systems require trustworthy foundations — and that starts with open, collaborative development.

1. Fork the repository
2. Create a feature branch
3. Write tests for your changes
4. Ensure all tests pass (`cargo test`)
5. Submit a pull request

## 📄 License

Licensed under either of:
- MIT License
- Apache License, Version 2.0

at your option.

---

<p align="center">
  <em>🦀 Built with Rust. Guarded with Aegis. Trusted by design.</em>
</p>
