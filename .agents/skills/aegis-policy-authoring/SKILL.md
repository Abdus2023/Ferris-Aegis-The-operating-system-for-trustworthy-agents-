---
name: aegis-policy-authoring
description: >
  Authors and validates Ferris Aegis policy rules in TOML format. Use when the user
  says "policy rule", "policy engine", "safety policy", "TOML policy", "policy
  authoring", or "deny-by-default". Do NOT use for trust scoring or agent lifecycle.
license: "MIT OR Apache-2.0"
compatibility: Requires Rust 1.82+ and ferris-aegis-kernel crate
metadata:
  aegis-crate: "ferris-aegis-kernel"
  aegis-phase: "1"
  aegis-depends: "aegis-trust-kernel"
  aegis-invariants: "INV-010"
  version: "0.4.0"
  author: "ferris-aegis"
  tags: "policy toml rules safety deny-by-default"
allowed-tools: Bash(cargo:*) Read Write
---

# Ferris Aegis — Policy Authoring

Author and validate TOML policy rules for the Ferris Aegis Policy Engine.

## When to Use

- Creating new safety policies in TOML format
- Modifying existing policy rules
- Understanding the deny-by-default policy model
- Validating policy files before deployment

## Policy Model

Ferris Aegis policies are **deny-by-default**: any action not explicitly allowed is denied.

```
Policy = name + version + priority + default_effect + rules[]
Rule   = action + effect + targets[] + description
Effect = "allow" | "deny"
```

## Workflow

1. Create a new TOML file in `policies/` directory
2. Define the policy header (name, version, priority, default_effect)
3. Add rules with action pattern, effect, targets, and description
4. Validate with `PolicyEngine::from_file(path)`
5. Test with `engine.evaluate(action, target)`

## Code Pattern — Policy File

```toml
[policy]
name = "data-access"
version = "1.0.0"
priority = 100
enabled = true
default_effect = "deny"

[[rules]]
action = "file:read"
effect = "allow"
targets = ["/workspace/data/*"]
description = "Allow reads from data directory"

[[rules]]
action = "file:write"
effect = "deny"
targets = ["/etc/*", "/var/*"]
description = "Deny writes to system directories"

[[rules]]
action = "network:connect"
effect = "allow"
targets = ["api.openai.com:443"]
description = "Allow connections to approved AI API endpoints"
```

## Code Pattern — Evaluate

```rust
use ferris_aegis_kernel::policy::PolicyEngine;

let engine = PolicyEngine::with_defaults();
// Or load custom: PolicyEngine::from_file("policies/data-access.toml")?;

assert!(engine.evaluate("file:read", "/workspace/data/info.txt").is_allowed());
assert!(!engine.evaluate("file:write", "/etc/passwd").is_allowed());
```

## Action Patterns

| Action Pattern | Meaning |
|---------------|---------|
| `file:read` | File read operations |
| `file:write` | File write operations |
| `network:connect` | Network connections |
| `exec:*` | Any code execution |
| `comm:broadcast` | Broadcast to all agents |
| `comm:direct` | Direct agent-to-agent communication |
| `policy:modify` | Policy modification (sovereign only) |

## Invariant

- **INV-010**: Config must validate before use. `AegisConfig::default_config().validate()` must return `Ok(())`.

## Templates

See `assets/policy-template.toml` for a starter policy template.
See `references/policy-design.md` for detailed policy design guidelines.
