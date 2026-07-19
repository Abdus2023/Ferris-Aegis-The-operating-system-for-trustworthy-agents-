# Ferris Aegis SKILL.md Implementation Guide

**Version:** 1.0.0  
**Status:** Reference Implementation  
**Language:** Rust  

---

## Overview

This guide explains how Ferris Aegis implements the vendor-neutral **SKILL.md** specification with full cryptographic verification, capability-based sandboxing, policy enforcement, and immutable audit trails.

Ferris Aegis adds optional security layers that don't break portability:

```
SKILL.md (vendor-neutral) ← loads successfully in Claude, Cursor, Copilot
    ↓
ferris_aegis: { trust_level, policies, audit, signature }
    ↓
Optional extensions ← ignored by other runtimes
    ↓
Ferris Aegis validates + sandboxes + audits
```

---

## Architecture: 7-Layer Execution Model

```
┌─────────────────────────────────────────────────┐
│ 1. Discovery & Registry Lookup                 │
│    Load from file or registry.dev               │
└──────────────┬──────────────────────────────────┘
               ↓
┌─────────────────────────────────────────────────┐
│ 2. Static Validation                            │
│    • Schema validation (YAML → types)           │
│    • Capability format check                    │
│    • Trigger pattern validation                 │
└──────────────┬──────────────────────────────────┘
               ↓
┌─────────────────────────────────────────────────┐
│ 3. Cryptographic Verification                  │
│    • Load Ed25519 signature + public key        │
│    • Verify manifest.json hash                  │
│    • Trust established (or rejected)            │
└──────────────┬──────────────────────────────────┘
               ↓
┌─────────────────────────────────────────────────┐
│ 4. Dependency Resolution                        │
│    • Recursively resolve skills, tools, models  │
│    • Detect circular dependencies               │
│    • Verify all dependencies available          │
└──────────────┬──────────────────────────────────┘
               ↓
┌─────────────────────────────────────────────────┐
│ 5. Policy & Trust Evaluation                   │
│    • Check agent trust score ≥ minimum          │
│    • Evaluate policy rules (allow/deny/alert)   │
│    • Capability vs. trust level alignment       │
└──────────────┬──────────────────────────────────┘
               ↓
┌─────────────────────────────────────────────────┐
│ 6. Sandbox Preparation                          │
│    • Bind capabilities to execution context     │
│    • Set resource limits (memory, time, files)  │
│    • Configure filesystem & network boundaries  │
│    • Prepare audit ledger handle                │
└──────────────┬──────────────────────────────────┘
               ↓
┌─────────────────────────────────────────────────┐
│ 7. Execution & Audit                            │
│    • Run entrypoint with inputs                 │
│    • Intercept capability usage                 │
│    • Record to SHA-256 chained audit ledger     │
│    • Emit OTel/Prometheus observability events  │
└─────────────────────────────────────────────────┘
```

---

## Integration Points

### 1. Skill Registry & Loading

```rust
use ferris_aegis_skills::{SkillRegistry, SkillLoader};
use std::path::Path;

// Load all skills from a directory
let registry = SkillRegistry::new();
registry.load_from_directory(Path::new("skills/"))?;

// Lookup by ID
let skill = registry.get("research-planner")?;

// Search by category
let filesystem_skills = registry.by_category("filesystem");

// Search by capability
let web_search_skills = registry.by_capability("network.http");
```

### 2. Static Validation

```rust
use ferris_aegis_skills::validator::SkillValidator;

// Validate schema, formats, policies
SkillValidator::validate_static(&skill)?;
// Returns: Ok(()) or Err(ValidationError)
```

### 3. Signature Verification

```rust
use ferris_aegis_skills::validator::SkillValidator;

// Load author's public key from trust store
let pub_key = load_public_key("acme-corp.pub")?;

// Verify signature
SkillValidator::verify_signature(&skill, &pub_key)?;
// Returns: Ok(()) if valid, Err(SignatureVerificationFailed)
```

### 4. Dependency Resolution

```rust
use ferris_aegis_skills::validator::DependencyResolver;

// Recursively resolve all dependencies
let resolved_skills = DependencyResolver::resolve(&skill, &registry)?;
// Returns: Vec<Skill> (topologically sorted, no cycles)
```

### 5. Policy Evaluation

```rust
use ferris_aegis_kernel::policy::PolicyEngine;

// Create policy engine from skill's policies
let policies = skill.ferris_aegis
    .as_ref()
    .map(|ae| ae.policies.clone())
    .unwrap_or_default();

let policy_engine = PolicyEngine::from_policies(policies)?;

// Evaluate: does this action conform to policy?
let verdict = policy_engine.evaluate(
    "filesystem:read",
    "/workspace/config.json"
)?;

match verdict {
    PolicyVerdict::Allow => { /* proceed */ }
    PolicyVerdict::Deny => { /* reject */ }
    PolicyVerdict::Alert => { /* warn + continue */ }
}
```

### 6. Sandbox Creation

```rust
use ferris_aegis_kernel::sandbox::Sandbox;
use ferris_aegis_skills::executor::ExecutionContext;

// Create sandbox with capabilities & limits
let mut sandbox = Sandbox::new();

// Bind capabilities from skill
for cap in &skill.permissions {
    sandbox.grant_capability(cap)?;
}

// Set resource limits
sandbox.set_resource_limit("max_memory", &skill.ferris_aegis.?.resource_limits.max_memory)?;
sandbox.set_resource_limit("max_execution_time", &skill.timeout)?;

// Configure filesystem & network
sandbox.set_filesystem_root(Path::new("/tmp/skill-execution"));
sandbox.set_network_allowed_domains(&skill.network.allowed_domains);

// Create execution context
let exec_ctx = ExecutionContext {
    execution_id: Uuid::new_v4(),
    agent_id: agent.id.clone(),
    agent_trust_score: agent.trust_score,
    
    capabilities: sandbox.capabilities().clone(),
    sandbox_boundary: "restricted".to_string(),
    workspace_root: Path::new("/workspace").to_path_buf(),
    temp_dir: Path::new("/tmp/skill-xyz").to_path_buf(),
    start_time: Utc::now(),
    deadline: Some(Utc::now() + Duration::seconds(300)),
};
```

### 7. Execution & Audit

```rust
use ferris_aegis_skills::executor::SkillExecutor;

// Execute the skill
let executor = SkillExecutor::new();

let result = executor.execute(
    &skill,
    &exec_ctx,
    serde_json::json!({
        "query": "AI safety research",
        "depth": 3
    })
).await?;

// Result includes:
// - Output data
// - Duration
// - OTel trace ID
// - Audit entries recorded to ledger

println!("Output: {:?}", result.output);
println!("Trace ID: {}", result.trace_id.unwrap_or_default());
```

---

## Capability Mapping

Ferris Aegis maps SKILL.md capabilities to kernel capabilities:

| SKILL.md Capability | Aegis Capability | Sandbox Boundary |
|-------------------|-----------------|-----------------|
| `filesystem.read` | `filesystem:read` | Restricted path |
| `filesystem.write` | `filesystem:write` | Workspace only |
| `filesystem.delete` | `filesystem:delete` | Denied by default |
| `network.http` | `network:connect` | Domain allowlist |
| `network.websocket` | `network:connect` | Domain allowlist |
| `memory.query` | `memory:read` | Episodic only |
| `memory.store` | `memory:write` | Current session |
| `mcp.call` | `execution:call` | Tool allowlist |
| `wasm.execute` | `execution:wasm` | Fuel-metered |
| `git.read` | `filesystem:read` | Git repo paths |
| `model.invoke` | `model:invoke` | Model allowlist |

---

## Trust Level Alignment

Map SKILL.md trust requirements to Aegis trust levels:

| SKILL.md `trust_level` | Aegis TrustLevel | Score Range | Capabilities |
|----------------------|----------------|-------------|--------------|
| `Unverified` | Unverified | 0.00–0.19 | Timer, Inter-agent comm |
| `Probationary` | Probationary | 0.20–0.49 | + Filesystem read |
| `Standard` | Standard | 0.50–0.74 | + Network, Environment |
| `Elevated` | Elevated | 0.75–0.94 | + Filesystem write, Process spawn |
| `Sovereign` | Sovereign | 0.95–1.00 | All capabilities |

**Enforcement:**

```rust
if agent.trust_score < skill.ferris_aegis.trust_level.minimum.as_f64() {
    return Err(SkillError::TrustLevelInsufficient { ... });
}
```

---

## Observability Integration

### OTel Tracing

Each skill execution generates a span tree:

```
[ferris-aegis-skill]
  ├─ [skill.discover]
  │  └─ duration_ms: 12
  ├─ [skill.validate]
  │  ├─ [schema-check]
  │  ├─ [signature-verify]
  │  └─ duration_ms: 45
  ├─ [skill.resolve-deps]
  │  └─ duration_ms: 23
  ├─ [skill.execute]
  │  ├─ [tool.invoke:web-search]
  │  │  └─ duration_ms: 1240
  │  ├─ [tool.invoke:parse-results]
  │  │  └─ duration_ms: 320
  │  └─ duration_ms: 1680
  └─ [skill.completed]
     └─ duration_ms: 1760
```

### Prometheus Metrics

```
ferris_aegis_skill_executions_total{skill_id="research-planner",status="success"} 42
ferris_aegis_skill_duration_seconds{skill_id="research-planner"} 1.76
ferris_aegis_skill_capabilities_used{skill_id="research-planner",capability="network.http"} 8
ferris_aegis_skill_policy_violations_total{skill_id="research-planner"} 2
```

### JSON Structured Logging

```json
{
  "timestamp": "2026-07-19T11:23:45.123Z",
  "level": "info",
  "message": "Skill execution completed",
  "skill_id": "research-planner",
  "execution_id": "abc-123-xyz",
  "agent_id": "agent-001",
  "status": "success",
  "duration_ms": 1760,
  "capabilities_used": ["network.http", "filesystem.read"],
  "audit_chain_length": 15,
  "trace_id": "4bf92f3577b34da6a3ce929d0e0e4736"
}
```

---

## Audit Trail

Every skill execution produces immutable audit entries:

```rust
// Audit entry format
{
  timestamp: 2026-07-19T11:23:45Z,
  agent_id: "agent-001",
  skill_id: "research-planner",
  execution_id: "abc-123-xyz",
  action: "skill:execute",
  capability: "network.http",
  outcome: "allow",
  context: {
    url: "https://api.openai.com/...",
    response_code: 200,
    bytes_transferred: 4096
  },
  previous_hash: "sha256:abcd1234...",
  hash: "sha256:efgh5678..."
}
```

**Chain Integrity:**

```
Entry 1: hash = sha256("audit data 1")
Entry 2: hash = sha256("audit data 2" + Entry1.hash)
Entry 3: hash = sha256("audit data 3" + Entry2.hash)
...
```

Verify chain at any time:

```rust
let ledger = AuditLedger::open("audit.ledger")?;
if ledger.verify_chain() {
    println!("✓ Audit chain integrity verified");
} else {
    println!("✗ AUDIT CHAIN TAMPERED");
}
```

---

## Example: Research Planner Skill

### SKILL.md

```yaml
---
spec_version: "1.0.0"
id: "research-planner"
name: "Research Planner"
version: "1.2.0"
description: "Plans research tasks with web search"
author: "Acme Research"
license: "MIT"

runtime: "mcp"
platforms:
  - name: "claude-code"
    min_version: "1.0"
  - name: "ferris-aegis"
    min_version: "0.3.0"

permissions:
  - network.http
  - filesystem.read

inputs:
  query:
    type: string
    required: true
  depth:
    type: integer
    default: 2

outputs:
  research_plan:
    type: object
    properties:
      steps:
        type: array
        items: { type: string }

entrypoint: "run"
timeout: "300s"
sandbox: "required"
network:
  allowed_domains:
    - "api.openai.com"
    - "wikipedia.org"
    - "scholar.google.com"

dependencies:
  skills:
    - id: "web-search"
      version: ">=1.0.0"
  models:
    - name: "claude-3-opus"

ferris_aegis:
  trust_level:
    minimum: "standard"
  policies:
    - id: "api-keys-only"
      rule: "network.http.api.openai.com only via authenticated channel"
      effect: "deny"
  audit:
    required: true
  signature:
    algorithm: "ed25519"
    required: true
---

# Research Planner

Plans research tasks by breaking them into structured steps.
```

### Rust Implementation

```rust
use serde_json::{json, Value};
use ferris_aegis_skills::executor::ExecutionContext;

/// Main entrypoint for the research-planner skill
pub async fn run(
    input: Value,
    ctx: &ExecutionContext,
) -> Result<Value, Box<dyn std::error::Error>> {
    let query = input["query"].as_str().ok_or("Missing query")?;
    let depth = input["depth"].as_i64().unwrap_or(2) as usize;

    // Record execution start
    ctx.record_action("research:start", &format!("query={}", query))?;

    // Step 1: Call web-search skill
    ctx.emit_capability_used("network.http")?;
    
    let search_results = call_web_search_skill(query, ctx).await?;
    
    ctx.record_checkpoint(&format!("Retrieved {} results", search_results.len()))?;

    // Step 2: Parse and organize
    let mut steps = vec![];
    for (i, result) in search_results.iter().enumerate() {
        steps.push(format!("Step {}: Investigate \"{}\"", i + 1, result["title"]));
    }

    // Step 3: Generate plan
    let plan = json!({
        "query": query,
        "depth": depth,
        "steps": steps,
        "estimated_time_minutes": steps.len() * 5
    });

    ctx.record_action("research:complete", "success")?;

    Ok(plan)
}

async fn call_web_search_skill(
    query: &str,
    ctx: &ExecutionContext,
) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    // In a real implementation, call the web-search skill
    // via MCP or dependency resolution
    
    // Mock implementation:
    Ok(vec![
        json!({"title": "AI Safety Overview", "url": "https://wikipedia.org/..."}),
        json!({"title": "Recent Advances in AI", "url": "https://scholar.google.com/..."}),
    ])
}
```

---

## CLI Integration

Ferris Aegis CLI can execute skills directly:

```bash
# Discover available skills
$ aegis skill list
research-planner          v1.2.0  Standard  6 months ago
web-search                v2.1.0  Probationary  2 weeks ago
markdown-parser           v1.0.0  Standard  1 year ago

# View skill details
$ aegis skill show research-planner
Name: Research Planner
Version: 1.2.0
Trust Level: Standard
Capabilities: network.http, filesystem.read
Dependencies: web-search@>=1.0.0

# Execute a skill
$ aegis skill run research-planner \
  --input '{"query":"quantum computing","depth":3}' \
  --agent my-agent

Executing research-planner v1.2.0...
✓ Signature verified
✓ Dependencies resolved (web-search@2.1.0)
✓ Trust level: 0.62 (Standard) ✓
✓ Sandbox prepared
✓ Policy: api-keys-only [ALERT] → continuing

Research Plan:
  1. Search for "quantum computing fundamentals"
  2. Review latest papers on quantum error correction
  3. Investigate quantum computing applications

Execution time: 2.3s
Trace ID: 4bf92f3577b34da6a3ce929d0e0e4736

# Check audit trail
$ aegis audit show --skill research-planner --execution abc-123-xyz
Entry 1: skill:execute     [allow]   network.http → https://api.openai.com
Entry 2: tool:invoke      [allow]   web-search
Entry 3: skill:complete   [allow]   output 2.1KB
Entry 4: [SHA-256 chain verified ✓]
```

---

## Best Practices

1. **Always Verify Signatures** — Never execute an unsigned skill in production
2. **Minimize Capabilities** — Only request what you truly need
3. **Set Timeouts** — Always include a timeout; default to 5 minutes
4. **Test in Sandbox** — Run skills in restricted sandbox first
5. **Monitor Audit Trail** — Regularly check for unexpected actions
6. **Version Dependencies** — Pin exact versions, not `*`
7. **Document Context** — Clearly state required environment variables
8. **Compose Safely** — Skill composition should preserve isolation boundaries

---

## See Also

- [SKILL.md Standard Specification](../SKILL_STANDARD_SPECIFICATION.md)
- [Ferris Aegis Architecture](../README.md)
- [Capability-Based Security](../CAPABILITIES.md)
- [Audit Ledger Format](../AUDIT_FORMAT.md)
