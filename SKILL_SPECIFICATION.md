# SKILL.md Specification — Agent Skills Library Standard

**Version:** 1.0.0  
**Status:** Active  
**Last Updated:** 2026-07-19

---

## Table of Contents

1. [Overview](#overview)
2. [SKILL.md Format & Frontmatter Schema](#skillmd-format--frontmatter-schema)
3. [Capability Classification](#capability-classification)
4. [Dependency Resolution](#dependency-resolution)
5. [Validation & Verification](#validation--verification)
6. [Execution Protocol](#execution-protocol)
7. [Integration with Ferris Aegis](#integration-with-ferris-aegis)
8. [Reference Implementation](#reference-implementation)

---

## Overview

### Purpose

The **SKILL.md** format is an open, portable, portable standard for packaging agent capabilities as modular, reusable, auditable units. A SKILL:

- Encapsulates a **domain workflow** or **tool integration** with exact steps, corrections, and error handling
- Declares **dependencies, trust boundaries, and capability requirements** upfront (zero surprise capability escalation)
- Includes **frontmatter metadata, trigger conditions, and fallback behavior**
- Carries **validation rules, security constraints, and execution protocols**
- Is **portable** across Claude, OpenAI Codex, Cursor, Gemini CLI, GitHub Copilot, and Ferris Aegis agents

### Design Principles

1. **Modular Context** — Load only the domain knowledge needed for a task
2. **Capability Explicitness** — All capabilities declared in frontmatter; no runtime surprises
3. **Auditability** — Every skill execution produces a trace; every action is logged
4. **Portability** — Works with any skill-compatible agent (verified by canonical test suite)
5. **Security by Default** — Policies, sandboxing, and trust gates are embedded, not bolted on
6. **Composability** — Skills can orchestrate other skills with explicit delegation

---

## SKILL.md Format & Frontmatter Schema

### Basic Structure

```yaml
---
# SKILL.md FRONTMATTER (YAML)
skill_version: "1.0.0"
skill_id: "skill:filesystem:file-processor"
name: "File Processor"
category: "filesystem"
description: "Safely read, validate, and process files with injection guards"
version: "1.2.0"
author: "Ferris Aegis Contributors"
license: "MIT OR Apache-2.0"

# Metadata & Discovery
tags: [filesystem, parsing, validation, safe-io]
keywords: [file-read, json-parse, csv-import, injection-guard]
maintainer: "security@example.com"

# Capability Declarations (EXPLICIT)
capabilities_required:
  - filesystem:read
  - filesystem:canonicalize
  - validation:injection-scan
trust_level_minimum: "probationary"  # 0.20–0.49
sandbox_boundary: "restricted"       # restricted | standard | elevated | sovereign

# Dependency Resolution
dependencies:
  - skill: "skill:validation:injection-scanner"
    version: ">=1.0.0"
    fallback: true
  - skill: "skill:filesystem:path-utils"
    version: ">=2.0.0"
    optional: false
  - system:
      - rust: ">=1.82"
      - jq: ">=1.7"

# Triggers & Activation
triggers:
  - event: "agent:action"
    action: "filesystem:read"
    weight: 100
  - event: "user:command"
    pattern: "^(cat|less|read) .+"
    weight: 80

# Performance & Resource Bounds
resource_limits:
  max_file_size: "100MB"
  max_execution_time: "30s"
  max_memory: "256MB"
  max_concurrent_calls: 5

# Policy Constraints
policies:
  - id: "deny-absolute-paths-outside-workspace"
    rule: "path must start with $WORKSPACE"
    effect: "deny"
  - id: "scan-for-injection-patterns"
    rule: "content matches injection_patterns"
    effect: "alert-then-allow"

# Execution Protocol & Versioning
execution_protocol: "aegis:rpc/1.0"
protocol_version: "V_2025_11_25"
export_format: "mcp-tool | http-rpc | stdio | wasm"

# Metadata for Multi-Agent Compatibility
compatible_agents:
  - name: "Ferris Aegis"
    min_version: "0.3.0"
    features: [otel, prometheus, audit]
  - name: "Claude Code"
    min_version: "1.0"
  - name: "Cursor"
    min_version: "0.36"
  - name: "GitHub Copilot"
    min_version: "2.0"

# Attestation & Signing
signature:
  algorithm: "ed25519"
  public_key: "abcd1234..."
  signed_at: "2026-07-19T11:11:51Z"

---
```

### Frontmatter Schema (YAML)

| Field | Type | Required | Example | Notes |
|-------|------|----------|---------|-------|
| `skill_version` | string | ✓ | `"1.0.0"` | Specification version (semver) |
| `skill_id` | string | ✓ | `"skill:filesystem:file-processor"` | Unique ID: `skill:<category>:<name>` |
| `name` | string | ✓ | `"File Processor"` | Human-readable name |
| `category` | string | ✓ | `"filesystem"` | Taxonomy category |
| `description` | string | ✓ | `"Safely read files..."` | One-sentence purpose |
| `version` | string | ✓ | `"1.2.0"` | Skill semantic version |
| `author` | string | ✓ | `"Ferris Aegis"` | Creator/maintainer |
| `license` | string | ✓ | `"MIT OR Apache-2.0"` | SPDX license expression |
| `tags` | list | ✓ | `[filesystem, parsing]` | Search/discovery tags |
| `capabilities_required` | list | ✓ | `[filesystem:read, validation:*]` | Capabilities this skill needs (wildcard supported) |
| `trust_level_minimum` | string | ✓ | `"probationary"` | `unverified\|probationary\|standard\|elevated\|sovereign` |
| `sandbox_boundary` | string | ✓ | `"restricted"` | Isolation level |
| `dependencies` | list | ✗ | See below | Skills/tools/system deps |
| `triggers` | list | ✗ | See below | Activation conditions |
| `resource_limits` | object | ✗ | See below | Execution constraints |
| `policies` | list | ✗ | See below | Declarative rules |
| `execution_protocol` | string | ✓ | `"aegis:rpc/1.0"` | RPC/transport protocol |
| `protocol_version` | string | ✓ | `"V_2025_11_25"` | MCP version if applicable |
| `export_format` | string | ✓ | `"mcp-tool"` | How skill is exposed |
| `compatible_agents` | list | ✗ | See below | Agent compatibility matrix |
| `signature` | object | ✗ | See below | Ed25519 attestation |

---

## Capability Classification

### Capability Namespace

All capabilities follow the taxonomy: `<domain>:<operation>[:<scope>]`

| Domain | Operations | Scopes |
|--------|-----------|--------|
| `filesystem` | read, write, delete, stat, list | global, workspace, temp |
| `network` | connect, bind, listen, fetch | public, internal, restricted |
| `validation` | injection-scan, schema-check, signature-verify | — |
| `crypto` | sign, encrypt, decrypt, hash | — |
| `memory` | read, write, search | episodic, semantic, session |
| `agent` | spawn, suspend, resume, terminate, query | local, remote |
| `audit` | record, verify, export | — |
| `execution` | compile, execute, sandbox | wasm, subprocess, native |
| `observability` | trace, metric, log | otel, prometheus, json |

### Capability Attestation

Each capability in a skill's `capabilities_required` list is verified against the executing agent's **trust level** and **policy engine**:

```yaml
capabilities_required:
  - filesystem:read
  - filesystem:write
  - filesystem:canonicalize
  - validation:injection-scan
  - crypto:sign
```

**At execution time:**
1. Agent checks executing agent's trust score against `trust_level_minimum`
2. Policy engine evaluates each capability against policies (e.g., deny path traversal)
3. If denied, execution halts with audit entry
4. If allowed, skill executes within sandbox boundaries

---

## Dependency Resolution

### Skill Dependencies

Skills can depend on other skills, system tools, or runtime features:

```yaml
dependencies:
  # Skill-to-skill dependency
  - skill: "skill:validation:injection-scanner"
    version: ">=1.0.0"
    optional: false
    fallback: true          # Use alternative implementation if unavailable

  # System tool dependency
  - system:
      jq: ">=1.7"
      curl: ">=7.68"
      python: ">=3.9"

  # Rust crate (for WASM skills)
  - crate: "serde_json"
    version: ">=1.0"
```

### Dependency Resolution Algorithm

```rust
// Pseudocode: Dependency Resolution
fn resolve_dependencies(skill: &Skill) -> Result<DepGraph> {
    let mut resolved = HashMap::new();
    let mut pending: Queue = skill.dependencies.into();

    while let Some(dep) = pending.pop_front() {
        match dep {
            Dependency::Skill { id, version, .. } => {
                if let Ok(resolved_skill) = fetch_skill(id, version) {
                    resolved.insert(id.clone(), resolved_skill);
                    pending.extend(resolved_skill.dependencies);
                } else if !dep.optional {
                    return Err(DependencyError::NotFound { id, version });
                }
            }
            Dependency::System { tools } => {
                for (tool, version) in tools {
                    if !check_system_tool(&tool, &version) {
                        if !dep.optional {
                            return Err(DependencyError::ToolMissing { tool, version });
                        }
                    }
                }
            }
        }
    }

    // Detect circular dependencies
    if has_cycle(&resolved) {
        return Err(DependencyError::CircularDependency);
    }

    Ok(resolved)
}
```

---

## Validation & Verification

### Static Validation (Before Execution)

```yaml
validation:
  schema:
    - skill_id must match pattern: skill:<category>:<name>
    - version must be semver
    - capabilities_required must be non-empty
    - all referenced triggers must be defined
  
  security:
    - all paths must be canonicalized
    - no hardcoded secrets (vault keys only)
    - signature must be valid (ed25519 for production)
  
  dependency:
    - all dependencies must be resolvable
    - no circular dependencies
    - version constraints must be compatible
```

### Runtime Validation (During Execution)

```rust
pub struct SkillValidator {
    policy_engine: PolicyEngine,
    audit_ledger: AuditLedger,
}

impl SkillValidator {
    /// Validate skill can execute in current context
    pub fn validate_execution(&self, skill: &Skill, context: &ExecutionContext) -> Result<()> {
        // 1. Trust level check
        if context.agent_trust_score < skill.trust_level_minimum {
            self.audit_ledger.record(AuditEntry {
                action: "skill:validation:failed",
                reason: "trust_level_insufficient",
                agent_id: context.agent_id.clone(),
            });
            return Err(ValidationError::TrustLevelInsufficient);
        }

        // 2. Capability check
        for cap in &skill.capabilities_required {
            if !context.has_capability(cap) {
                return Err(ValidationError::CapabilityDenied(cap.clone()));
            }
        }

        // 3. Policy evaluation
        for policy in &skill.policies {
            match self.policy_engine.evaluate(policy, context)? {
                PolicyVerdict::Deny => {
                    return Err(ValidationError::PolicyViolation(policy.id.clone()));
                }
                PolicyVerdict::Allow => {}
                PolicyVerdict::Alert => {
                    self.audit_ledger.record(AuditEntry {
                        action: "skill:policy:alert",
                        policy_id: policy.id.clone(),
                        agent_id: context.agent_id.clone(),
                    });
                }
            }
        }

        // 4. Resource limit check
        if context.memory_usage > skill.resource_limits.max_memory {
            return Err(ValidationError::ResourceLimitExceeded);
        }

        Ok(())
    }
}
```

### Signature Verification

```rust
pub fn verify_skill_signature(skill: &Skill, public_key: &[u8]) -> Result<()> {
    let signature_data = skill.get_signable_bytes();
    let sig = ed25519_dalek::Signature::from_bytes(&skill.signature.signature)?;
    
    let verifying_key = ed25519_dalek::VerifyingKey::from_bytes(public_key)?;
    verifying_key.verify(&signature_data, &sig)?;
    
    Ok(())
}
```

---

## Execution Protocol

### MCP Tool Export

A SKILL.md is exported as an MCP tool if `export_format: "mcp-tool"`:

```json
{
  "name": "file_processor",
  "description": "Safely read, validate, and process files with injection guards",
  "inputSchema": {
    "type": "object",
    "properties": {
      "file_path": {
        "type": "string",
        "description": "Absolute path to file (canonicalized)"
      },
      "validation": {
        "type": "object",
        "properties": {
          "scan_injection": { "type": "boolean" },
          "max_size": { "type": "string" }
        }
      }
    },
    "required": ["file_path"]
  }
}
```

### RPC Execution Flow

```
┌─────────────────┐
│  Agent Request  │
│ (MCP/HTTP/RPC)  │
└────────┬────────┘
         │
         ▼
┌──────────────────────────────┐
│  Load SKILL.md + Frontmatter │
│  Parse dependencies          │
└────────┬─────────────────────┘
         │
         ▼
┌──────────────────────────────┐
│ Validate Execution Context   │
│ • Trust level               │
│ • Capabilities              │
│ • Policies                  │
│ • Resources                 │
└────────┬─────────────────────┘
         │
         ▼
┌──────────────────────────────┐
│ Create Sandbox Boundary      │
│ • Set resource limits        │
│ • Bind filesystem scope      │
│ • Attach audit ledger        │
└────────┬─────────────────────┘
         │
         ▼
┌──────────────────────────────┐
│ Execute Skill                │
│ • Run workflow steps         │
│ • Emit OTel spans            │
│ • Increment Prometheus vars  │
└────────┬─────────────────────┘
         │
         ▼
┌──────────────────────────────┐
│ Record Audit Entry           │
│ • Action taken               │
│ • Outcome                    │
│ • Resources used             │
└────────┬─────────────────────┘
         │
         ▼
┌──────────────────────────────┐
│ Return Result to Caller      │
│ (with trace/metric context)  │
└──────────────────────────────┘
```

### Execution Context

```rust
pub struct ExecutionContext {
    pub agent_id: AgentId,
    pub agent_trust_score: TrustScore,
    pub session_id: SessionId,
    
    pub capabilities: HashSet<Capability>,
    pub sandbox_boundary: SandboxBoundary,
    
    pub workspace_root: PathBuf,
    pub temp_dir: PathBuf,
    
    pub memory_usage: usize,
    pub execution_deadline: Instant,
    
    pub audit_ledger: Arc<AuditLedger>,
    pub metrics: Arc<CoreMetrics>,
}

impl ExecutionContext {
    pub fn has_capability(&self, cap: &str) -> bool {
        self.capabilities.iter().any(|c| {
            c.matches(cap)  // Supports wildcards: filesystem:* matches filesystem:read
        })
    }

    pub fn record_action(&self, action: &str, outcome: &str) -> Result<()> {
        self.audit_ledger.append(AuditEntry {
            timestamp: Utc::now(),
            agent_id: self.agent_id.clone(),
            action: action.to_string(),
            outcome: outcome.to_string(),
            context: format!("skill:{}", action),
        })
    }
}
```

### Error Handling & Fallback

```yaml
fallback_behavior:
  on_dependency_missing: "use_bundled_alternative"
  on_capability_denied: "halt_with_audit_entry"
  on_policy_violation: "alert_then_continue"
  on_timeout: "kill_and_restore_state"
  on_resource_limit: "graceful_degradation"
```

---

## Integration with Ferris Aegis

### Skill as Aegis Guard Agent

A skill can be registered as a **Guard** within Ferris Aegis, granting it real-time monitoring privileges:

```rust
/// In crates/kernel/src/guard.rs
pub struct SkillGuard {
    skill: Skill,
    execution_context: ExecutionContext,
}

impl SkillGuard {
    pub async fn monitor(&self, agent_action: &AgentAction) -> GuardVerdict {
        // Load skill, validate, execute with monitoring capability
        let result = self.execute_skill(&agent_action).await?;
        
        match result {
            SkillResult::Allow => GuardVerdict::Allow,
            SkillResult::Alert { reason } => {
                GuardVerdict::Alert(reason)
            }
            SkillResult::Deny => GuardVerdict::Quarantine,
        }
    }
}
```

### Skill Registry in Aegis

```rust
// In main CLI
pub struct SkillRegistry {
    skills: HashMap<String, Arc<Skill>>,
    cache: Arc<Mutex<SkillCache>>,
}

impl SkillRegistry {
    pub async fn load_from_directory(&mut self, path: &Path) -> Result<()> {
        for entry in std::fs::read_dir(path)? {
            let path = entry?.path();
            if path.extension().map_or(false, |ext| ext == "md") {
                let skill = Skill::from_file(&path).await?;
                self.register(skill)?;
            }
        }
        Ok(())
    }

    pub fn list_by_category(&self, category: &str) -> Vec<&Skill> {
        self.skills
            .values()
            .filter(|s| s.category == category)
            .map(|s| s.as_ref())
            .collect()
    }
}
```

---

## Reference Implementation

### Complete Skill Example: File Processor

**File:** `skills/filesystem/file-processor.md`

````markdown
---
skill_version: "1.0.0"
skill_id: "skill:filesystem:file-processor"
name: "File Processor"
category: "filesystem"
description: "Safely read, validate, and process files with injection guards"
version: "1.2.0"
author: "Ferris Aegis Contributors"
license: "MIT OR Apache-2.0"

tags: [filesystem, parsing, validation, safe-io]
keywords: [file-read, json-parse, csv-import, injection-guard]

capabilities_required:
  - filesystem:read
  - filesystem:canonicalize
  - validation:injection-scan

trust_level_minimum: "probationary"
sandbox_boundary: "restricted"

dependencies:
  - skill: "skill:validation:injection-scanner"
    version: ">=1.0.0"
    optional: false
  - system:
      jq: ">=1.7"

triggers:
  - event: "agent:action"
    action: "filesystem:read"
    weight: 100

resource_limits:
  max_file_size: "100MB"
  max_execution_time: "30s"
  max_memory: "256MB"
  max_concurrent_calls: 5

policies:
  - id: "workspace-only"
    rule: "path must be within $WORKSPACE"
    effect: "deny"
  - id: "no-symlink-escape"
    rule: "canonicalized_path must start with canonical_workspace"
    effect: "deny"

execution_protocol: "aegis:rpc/1.0"
export_format: "mcp-tool"

compatible_agents:
  - name: "Ferris Aegis"
    min_version: "0.3.0"
  - name: "Claude Code"
    min_version: "1.0"
  - name: "Cursor"
    min_version: "0.36"

---

## Overview

This skill safely reads and processes files with automatic injection guards, size validation, and canonicalized path resolution. Perfect for agents that need file access without capability escalation.

## Inputs

- **`file_path`** (string, required): Path to file (can be relative; will be canonicalized)
- **`max_size`** (string, optional): Max file size (e.g., "10MB", default "100MB")
- **`scan_injection`** (boolean, optional): Run injection scanner before processing (default: true)

## Outputs

- **`content`** (string): File content
- **`metadata`** (object): File metadata (size, perms, modified time)
- **`scan_result`** (object): Injection scan verdict if enabled
- **`trace_id`** (string): OTel trace ID for audit

## Step 1: Validate Input

```rust
fn validate_input(file_path: &str, max_size: &str) -> Result<ValidatedInput> {
    // 1a. Parse and canonicalize path
    let path = std::fs::canonicalize(file_path)
        .map_err(|e| SkillError::PathResolution(e))?;

    // 1b. Verify path is within workspace
    let workspace = std::env::var("WORKSPACE")
        .unwrap_or_else(|_| "/workspace".to_string());
    let workspace_canonical = std::fs::canonicalize(&workspace)?;
    
    if !path.starts_with(&workspace_canonical) {
        return Err(SkillError::PathTraversal);
    }

    // 1c. Parse size limit
    let size_bytes = parse_size(max_size)?;
    
    Ok(ValidatedInput { path, size_bytes })
}
```

## Step 2: Check File Metadata

```rust
fn check_metadata(path: &Path, size_limit: u64) -> Result<FileMetadata> {
    let metadata = std::fs::metadata(path)?;
    
    if metadata.len() > size_limit {
        return Err(SkillError::FileTooLarge {
            actual: metadata.len(),
            limit: size_limit,
        });
    }

    if !metadata.is_file() {
        return Err(SkillError::NotAFile);
    }

    Ok(FileMetadata {
        size: metadata.len(),
        modified: metadata.modified()?,
        permissions: format!("{:o}", metadata.permissions().mode()),
    })
}
```

## Step 3: Run Injection Scanner (if enabled)

This step calls the **`skill:validation:injection-scanner`** skill as a dependency.

```rust
async fn scan_for_injection(content: &str) -> Result<ScanResult> {
    let scanner = load_skill("skill:validation:injection-scanner")?;
    let result = scanner.execute(InjectionScanInput {
        text: content.to_string(),
        patterns: vec![
            r#"(?i)(union.*select|drop\s+table|exec\(|eval\()"#,
            r#"<script.*</script>"#,
            r#"\$\{.*\}"#,  // Template injection
        ],
    }).await?;

    Ok(result)
}
```

## Step 4: Read and Return

```rust
async fn read_file(path: &Path) -> Result<String> {
    let content = tokio::fs::read_to_string(path).await?;
    Ok(content)
}
```

## Error Cases

- **Path traversal**: Deny access, record audit entry, alert
- **File too large**: Return partial content + warning
- **Injection detected**: Alert-only (process content for inspection)
- **Permission denied**: Return error with hint to adjust policy

## Example Usage (MCP)

```json
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "file_processor",
    "arguments": {
      "file_path": "./config/database.json",
      "max_size": "50MB",
      "scan_injection": true
    }
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "result": {
    "content": "{\"host\":\"localhost\",\"port\":5432}",
    "metadata": {
      "size": 42,
      "modified": "2026-07-19T10:00:00Z",
      "permissions": "644"
    },
    "scan_result": {
      "verdict": "clean",
      "patterns_checked": 3
    },
    "trace_id": "abc123def456"
  }
}
```
````

---

## Multi-Agent Compatibility Matrix

| Agent | SKILL.md Support | Export Format | Audit Integration |
|-------|------------------|---------------|--------------------|
| **Ferris Aegis** | ✅ Native | MCP, HTTP-RPC, stdio | ✅ Full audit ledger |
| **Claude Code** | ✅ via MCP | MCP tool definition | ⚠️ External logging |
| **Cursor** | ✅ via MCP | MCP tool definition | ⚠️ Cursor telemetry |
| **GitHub Copilot** | ✅ via MCP | MCP tool definition | ⚠️ GitHub logs |
| **Gemini CLI** | ✅ via HTTP-RPC | HTTP endpoint | ⚠️ Google Cloud Logging |
| **OpenAI Codex** | ✅ via HTTP-RPC | HTTP endpoint | ❌ No native audit |

---

## Specification Versioning

This specification follows **semantic versioning**. Future versions may introduce:

- **1.1.0**: Skill composition & orchestration primitives
- **1.2.0**: Episodic memory bindings for skill state
- **2.0.0**: Agent-to-agent skill delegation with trust gates

---

## See Also

- [Ferris Aegis Architecture](./README.md)
- [Policy Engine Specification](./POLICY_SPEC.md)
- [Audit Ledger Format](./AUDIT_FORMAT.md)
- [Capability Classification](./CAPABILITIES.md)
