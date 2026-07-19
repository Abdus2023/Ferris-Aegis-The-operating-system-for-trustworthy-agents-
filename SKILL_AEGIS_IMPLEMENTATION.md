# SKILL.md — Ferris Aegis Reference Implementation Guide

> **Version:** 1.0.0  
> **Spec:** SKILL_STANDARD_SPECIFICATION.md v1.0.0  
> **Crate:** `ferris-aegis-skills` (v0.4.0)  
> **Runtime:** Ferris Aegis OS for Trustworthy Agents  

---

## 1. Overview

This document describes how the **Ferris Aegis** operating system implements the vendor-neutral SKILL.md specification. It covers:

- The 7-layer execution model (Discovery → Validation → Verify → Resolve → Policy → Sandbox → Execute)
- How SKILL.md declarations map to Aegis kernel capabilities
- Trust level alignment
- Observability integration (OTel, Prometheus, JSON)
- Audit trail format
- CLI commands

---

## 2. 7-Layer Execution Model

When a skill is activated in Ferris Aegis, it passes through 7 distinct layers. Each layer can short-circuit execution if its checks fail.

```
┌─────────────────────────────────────┐
│  1. Discovery                       │  Scan .agents/skills/ for SKILL.md
│     ↓                               │  Parse frontmatter, build registry
│  2. Validation                      │  Check Layer 1 schema rules (15 rules)
│     ↓                               │  Check extension block schema
│  3. Verify                          │  Signature verification (Layer 5)
│     ↓                               │  Digest integrity check
│  4. Resolve                         │  Resolve dependencies (skills, tools, models)
│     ↓                               │  Detect dependency cycles
│  5. Policy                          │  Check trust level requirements
│     ↓                               │  Evaluate policy rules
│     ↓                               │  Check permission grants
│  6. Sandbox                         │  Apply sandbox constraints
│     ↓                               │  Enforce network/filesystem limits
│  7. Execute                         │  Run skill instructions
│                                     │  Emit observability events
│                                     │  Record audit trail
└─────────────────────────────────────┘
```

### 2.1 Layer 1: Discovery

The `SkillRegistry` scans the configured skill directory (default: `.agents/skills/`) and parses each `SKILL.md` file's frontmatter. It builds an in-memory map of `SkillMetadata` objects.

```rust
let mut registry = SkillRegistry::new(SkillRegistryConfig::default());
let count = registry.discover(".agents/skills").await?;
```

**Short-circuit:** If no SKILL.md is found, discovery returns 0 skills.

### 2.2 Layer 2: Validation

The `SkillValidator` applies 15 Layer 1 validation rules plus any Layer 2 extension rules:

```rust
let results = registry.validate_all();
for result in &results {
    if !result.is_valid() {
        // Log errors and skip this skill
    }
}
```

**Short-circuit:** Invalid skills are flagged but not removed from the registry. Activation will fail at this layer.

### 2.3 Layer 3: Verify (Cryptographic)

If the `ferris_aegis.signature` block is present:

1. Compute SHA-256 digest of SKILL.md content (excluding `signature.value`)
2. Verify Ed25519 signature against `signature.public_key`
3. Check `signature.signed_at` is within validity window

```rust
if let Some(sig) = &frontmatter.ferris_aegis.signature {
    let verified = verify_ed25519_signature(
        &sig.public_key,
        &sig.value,
        skill_content_hash,
    )?;
    if !verified {
        return Err(SkillError::SignatureVerificationFailed);
    }
}
```

**Short-circuit:** Failed signature verification prevents skill activation.

### 2.4 Layer 4: Resolve (Dependencies)

Resolve all declared dependencies:

1. **Skills** — Check that each `dependencies.skills[].id` is available in the registry
2. **Tools** — Check that each `dependencies.tools[].name` is registered with the MCP server
3. **Models** — Check that each `dependencies.models[].name` is accessible
4. **Cycles** — Run topological sort to detect dependency cycles

```rust
let resolver = DependencyResolver::new(&registry);
let resolved = resolver.resolve("skill:research:research-planner")?;
// Returns ordered list of skills to activate before this one
```

**Short-circuit:** Missing dependencies or cycles prevent activation.

### 2.5 Layer 5: Policy

Check whether the requesting agent is authorized:

1. **Trust level** — Compare agent's current trust level against `ferris_aegis.trust_level`
2. **Policy rules** — Evaluate each rule in `ferris_aegis.policies[]`
3. **Permission grants** — Check that declared `permissions[]` are granted to the agent

```rust
let agent_trust = kernel.get_record(&agent_id)?.level;
let required_trust = parse_trust_level(&frontmatter.ferris_aegis.trust_level)?;
if agent_trust < required_trust {
    return Err(SkillError::InsufficientTrustLevel {
        required: required_trust,
        actual: agent_trust,
    });
}
```

**Short-circuit:** Insufficient trust or failed policy check prevents activation.

### 2.6 Layer 6: Sandbox

Apply sandbox constraints from the `sandbox` and `ferris_aegis.sandbox` blocks:

1. **Network** — Restrict outbound connections to `allowed_domains`
2. **Filesystem** — Restrict file access to `allowed_paths`
3. **WASM** — If `wasm_module` is specified, execute in WASM sandbox with fuel/memory limits
4. **Compute** — Enforce memory and CPU limits

```rust
let sandbox_config = SandboxConfig {
    network_allowlist: frontmatter.sandbox.network.allowed_domains,
    fs_allowlist: frontmatter.sandbox.filesystem.allowed_paths,
    wasm_module: frontmatter.ferris_aegis.sandbox.wasm_module,
    fuel: frontmatter.ferris_aegis.sandbox.fuel,
    memory_limit: frontmatter.ferris_aegis.sandbox.memory_limit,
};
let sandbox = Sandbox::new(sandbox_config);
```

**Short-circuit:** Sandbox configuration failure prevents activation.

### 2.7 Layer 7: Execute

Run the skill instructions within the sandbox:

1. Load Tier 2 instructions from SKILL.md body
2. Load Tier 3 resources on demand
3. Emit OTel spans for each step
4. Record audit trail entries
5. Enforce timeout

```rust
let skill = registry.load_skill("research-planner").await?;
let result = sandbox.execute(async {
    skill.instructions.execute(inputs).await
}).timeout(frontmatter.timeout).await?;
```

---

## 3. Capability Mapping: SKILL.md → Aegis Kernel

The SKILL.md `permissions` field maps directly to Ferris Aegis `Capability` variants:

| SKILL.md Permission | Aegis Capability | Trust Level Required |
|---------------------|-----------------|---------------------|
| `network.http.get` | `Network` | Standard (0.50+) |
| `network.http.post` | `Network` | Elevated (0.75+) |
| `filesystem.read.*` | `FilesystemRead` | Probationary (0.20+) |
| `filesystem.write.*` | `FilesystemWrite` | Elevated (0.75+) |
| `process.spawn` | `ProcessSpawn` | Elevated (0.75+) |
| `crypto.hash` | `Crypto` | Elevated (0.75+) |
| `crypto.sign` | `Crypto` | Sovereign (0.95+) |
| `memory.read` | `FilesystemRead` | Probationary (0.20+) |
| `memory.write` | `FilesystemWrite` | Standard (0.50+) |
| `agent.spawn` | `ProcessSpawn` | Sovereign (0.95+) |
| `agent.communicate` | `InterAgentComm` | Unverified (0.00+) |
| `compute.wasm.execute` | `WasmExecute` | Standard (0.50+) |

**Resolution algorithm:**

1. For each permission in the skill's `permissions[]`, look up the corresponding Aegis Capability
2. Compute the maximum trust level required across all permissions
3. Compare against `ferris_aegis.trust_level` (if set, use the higher of the two)
4. Verify the agent's trust level meets or exceeds the required level

---

## 4. Trust Level Alignment

| Ferris Aegis Level | Score Range | SKILL.md Trust Tag | Example Skills |
|--------------------|------------|-------------------|----------------|
| Unverified | 0.00–0.19 | `"Unverified"` | `agent.communicate` only |
| Probationary | 0.20–0.49 | `"Probationary"` | Simple read-only skills |
| Standard | 0.50–0.74 | `"Standard"` | Research, search, analysis |
| Elevated | 0.75–0.94 | `"Elevated"` | Code modification, process spawn |
| Sovereign | 0.95–1.00 | `"Sovereign"` | Agent spawning, crypto signing |

### Trust Gating Flow

```
Skill Activation Request
         │
         ▼
  ferris_aegis.trust_level specified?
         │
    Yes ─┤── No
         │      │
         ▼      ▼
  Check against   Compute from
  explicit level  permissions[]
         │      │
         └──┬───┘
            ▼
  Agent trust ≥ required?
         │
    Yes ─┤── No
         │      │
         ▼      ▼
    Allow     Deny + Audit
```

---

## 5. Observability Integration

### 5.1 OpenTelemetry Spans

Each skill execution generates OTel spans:

```
skill.activate/{skill_name}          — Layer 1: Activation begins
  skill.validate/{skill_name}        — Layer 2: Validation
  skill.verify/{skill_name}          — Layer 3: Signature verify
  skill.resolve/{skill_name}         — Layer 4: Dependency resolution
  skill.policy/{skill_name}          — Layer 5: Policy check
  skill.sandbox/{skill_name}         — Layer 6: Sandbox setup
  skill.execute/{skill_name}         — Layer 7: Execution
    skill.step/{step_name}           — Individual step within execution
```

**Attributes on each span:**

| Attribute | Type | Description |
|-----------|------|-------------|
| `skill.id` | string | Full skill ID (e.g. `skill:research:research-planner`) |
| `skill.version` | string | Skill version |
| `skill.name` | string | Short skill name |
| `skill.trust_level` | string | Required trust level |
| `skill.agent_id` | string | Agent invoking the skill |
| `skill.layer` | int | Execution layer (1-7) |

### 5.2 Prometheus Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `aegis_skill_activations_total` | Counter | `skill`, `trust_level` | Total skill activations |
| `aegis_skill_duration_seconds` | Histogram | `skill`, `layer` | Duration per execution layer |
| `aegis_skill_errors_total` | Counter | `skill`, `layer`, `error_type` | Errors per layer |
| `aegis_skill_sandbox_violations_total` | Counter | `skill`, `violation_type` | Sandbox constraint violations |
| `aegis_skill_dependencies_resolved` | Gauge | `skill` | Number of resolved dependencies |

### 5.3 JSON Audit Events

Each skill execution emits a structured JSON audit event:

```json
{
  "timestamp": "2026-07-19T12:34:56.789Z",
  "event_type": "skill.execution",
  "skill_id": "skill:research:research-planner",
  "skill_version": "1.2.0",
  "agent_id": "agent:researcher-01",
  "trust_level": "Standard",
  "layers": {
    "discovery": { "status": "ok", "duration_ms": 1 },
    "validation": { "status": "ok", "duration_ms": 2, "errors": 0, "warnings": 0 },
    "verify": { "status": "ok", "duration_ms": 5, "signature_valid": true },
    "resolve": { "status": "ok", "duration_ms": 3, "dependencies": 2 },
    "policy": { "status": "ok", "duration_ms": 1, "trust_required": "Standard" },
    "sandbox": { "status": "ok", "duration_ms": 8, "constraints_applied": 3 },
    "execute": { "status": "ok", "duration_ms": 2340, "steps_completed": 5 }
  },
  "digest": "sha256:a1b2c3...",
  "audit_chain_hash": "sha256:d4e5f6..."
}
```

---

## 6. Audit Trail Format

Skill executions are recorded in the `AuditLedger` with SHA-256 chain integrity (INV-006):

```
AuditEntry {
    timestamp:   2026-07-19T12:34:56Z,
    agent_id:    "agent:researcher-01",
    action:      "skill.execute",
    target:      "skill:research:research-planner",
    success:     true,
    severity:    AuditSeverity::Info,
    metadata: {
        "skill_version": "1.2.0",
        "trust_level": "Standard",
        "layers_completed": "7",
        "duration_ms": "2360",
    },
    prev_hash:   "sha256:previous_entry_hash...",
    entry_hash:  "sha256:this_entry_hash...",
}
```

**Chain verification:**

```rust
// Each entry's hash covers: timestamp + agent_id + action + target + prev_hash
// verify_chain() recomputes hashes and checks they match
assert!(ledger.verify_chain());
```

---

## 7. CLI Commands

### 7.1 Existing Commands (agentskills.io v0.2.0)

```bash
# List discovered skills
aegis skills list

# Validate all skills against spec rules
aegis skills validate

# Show a specific skill's details
aegis skills show <name>

# Generate discovery index JSON
aegis skills index
```

### 7.2 New Commands (Vendor-Neutral v1.0.0)

```bash
# Run a skill with inputs
aegis skill run <id> --input topic="quantum computing" --input depth="deep"

# Sign a skill with Ed25519
aegis skill sign <id> --key ~/.aegis/signing-key.pem

# Publish a skill to a registry
aegis skill publish <id> --registry https://skills.example.com

# Verify a skill's signature
aegis skill verify <id>

# Show dependency tree
aegis skill deps <id>

# Show skill execution history
aegis skill history <id>
```

### 7.3 Output Format

All CLI commands support `--format json` for machine-readable output:

```bash
aegis skills list --format json
```

```json
{
  "skills": [
    {
      "id": "skill:research:research-planner",
      "name": "research-planner",
      "version": "1.2.0",
      "description": "Orchestrates multi-step research...",
      "trust_level": "Standard",
      "permissions": ["network.http.get", "filesystem.read.tmp"],
      "lifecycle": "stable"
    }
  ]
}
```

---

## 8. Crate Architecture

The `ferris-aegis-skills` crate provides the programmatic implementation:

```
crates/skills/src/
├── lib.rs              # Public API: SkillRegistry, SkillValidator, SkillIndex
├── types.rs            # Core types: SkillId, Capability, TrustLevelRequired,
│                       #   Dependency, Trigger, PolicyRule, ResourceLimits, Signature
├── loader.rs           # SKILL.md parsing (frontmatter + body extraction)
├── validator.rs        # Validation engine (15 Layer 1 rules + Layer 2 rules)
├── resolver.rs         # Dependency resolver (topological sort, cycle detection)
├── index.rs            # Discovery index generation
└── error.rs            # SkillError enum (comprehensive error types)
```

### Key Types

```rust
/// Skill ID in namespace format: skill:<category>:<name>
pub struct SkillId {
    pub category: String,
    pub name: String,
}

/// Capability declaration: <domain>.<operation>
pub struct Capability {
    pub domain: String,
    pub operation: String,
}

/// Required trust level for skill activation
pub enum TrustLevelRequired {
    Unverified,
    Probationary,
    Standard,
    Elevated,
    Sovereign,
}

/// Dependency with version constraint
pub struct Dependency {
    pub id: SkillId,
    pub version: VersionReq,
    pub optional: bool,
    pub dep_type: DependencyType, // Skill, Tool, or Model
}

/// Policy rule enforced during execution
pub struct PolicyRule {
    pub name: String,
    pub effect: PolicyEffect, // Allow or Deny
    pub condition: Option<String>,
}

/// Resource limits for sandboxed execution
pub struct ResourceLimits {
    pub max_memory: Option<u64>,
    pub max_cpu_seconds: Option<u64>,
    pub max_network_requests: Option<u32>,
    pub max_file_size: Option<u64>,
}

/// Cryptographic signature for skill provenance
pub struct Signature {
    pub algorithm: String,       // "ed25519"
    pub public_key: String,      // "sha256:hex..."
    pub value: String,           // "ed25519:base64..."
    pub signed_at: DateTime<Utc>,
    pub signed_by: String,
}
```

---

## 9. Migration from agentskills.io v0.2.0

### 9.1 Frontmatter Migration

**Before (v0.2.0):**

```yaml
name: aegis-trust-kernel
description: Manages trust scores. Use when the user mentions trust.
license: "MIT OR Apache-2.0"
compatibility: Requires Rust 1.82+
metadata:
  aegis-crate: "ferris-aegis-kernel"
  aegis-phase: "1"
  aegis-invariants: "INV-006 INV-010"
  version: "0.4.0"
  author: "ferris-aegis"
  tags: "trust kernel audit"
allowed-tools: Bash(cargo:*) Read Write
```

**After (v1.0.0 vendor-neutral):**

```yaml
spec_version: "1.0.0"
id: "skill:trust:trust-kernel"
name: "aegis-trust-kernel"
version: "0.4.0"
description: Manages trust scores. Use when the user mentions trust.
author: "ferris-aegis"
license: "MIT OR Apache-2.0"
runtime: "native"
platforms:
  - name: "ferris-aegis"
    min_version: "0.4.0"
permissions:
  - "filesystem.read./aegis"
  - "filesystem.write./aegis/trust"
  - "crypto.hash"
  - "agent.communicate"
inputs:
  - name: "agent_id"
    type: "string"
    required: true
  - name: "action"
    type: "enum:reinforce,penalize,query"
    required: true
outputs:
  - name: "trust_record"
    type: "json"
dependencies:
  skills: []
  tools:
    - name: "aegis-kernel"
      version: ">=0.4.0"
ferris_aegis:
  trust_level: "Standard"
  policies: ["deny-filesystem-write-etc"]
  audit: true
  crate: "ferris-aegis-kernel"
  phase: "1"
  invariants: "INV-006 INV-010"
```

### 9.2 Migration Tool

A migration helper is planned:

```bash
aegis skill migrate <name> --from v0.2.0 --to v1.0.0
```

This will:
1. Add `spec_version`, `id`, `version`, `author` fields
2. Convert `metadata.aegis-*` to `ferris_aegis:` block
3. Convert `compatibility` to `platforms` array
4. Convert `allowed-tools` to `permissions` array
5. Infer `inputs`/`outputs` from the skill instructions (heuristic)
6. Add `dependencies` from `metadata.aegis-depends`

---

## 10. Security Invariant Enforcement

Skills enforce the security invariants from `docs/ARCHITECTURE-QUICK-REF.md`:

| Invariant | Enforcement Layer | Skill Mechanism |
|-----------|------------------|-----------------|
| INV-001: Credential flow | Layer 5 (Policy) | `ferris_aegis.policies: deny-credential-leak` |
| INV-002: No plaintext secrets | Layer 5 (Policy) | `permissions: -crypto.encrypt` required |
| INV-003: Allowlist enforcement | Layer 7 (Execute) | Tool allowlist checked before each call |
| INV-004: MCP protocol version | Layer 2 (Validate) | `runtime: "mcp"` + pinned version |
| INV-005: Injection scan | Layer 5 (Policy) | Input validation via `inputs[].validation` regex |
| INV-006: Audit chain | Layer 7 (Execute) | Every execution recorded in `AuditLedger` |
| INV-007: SSRF guard | Layer 6 (Sandbox) | `sandbox.network.allowed_domains` |
| INV-008: Rate limiting | Layer 6 (Sandbox) | `sandbox.network.max_requests` |
| INV-009: WASM sandbox | Layer 6 (Sandbox) | `ferris_aegis.sandbox.wasm_module` |
| INV-010: Config validation | Layer 2 (Validate) | Frontmatter schema validation |
| INV-011: Circuit breaker | Layer 5 (Policy) | Dependency health check before activation |
| INV-012: Retry policy | Layer 7 (Execute) | `timeout` + retry config in dependencies |
| INV-013: Hash verification | Layer 3 (Verify) | SHA-256 digest of SKILL.md content |
| INV-014: Checkpoint every step | Layer 7 (Execute) | Durable execution integration |
| INV-015: Resume from checkpoint | Layer 7 (Execute) | Crash recovery integration |

---

*Version 1.0.0 — 2026-07-19 — Ferris Aegis Project*
