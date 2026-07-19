# SKILL.md — Ecosystem Summary

> **Version:** 1.0.0  
> **Date:** 2026-07-19  
> **Project:** Ferris Aegis — The Operating System for Trustworthy Agents  

---

## 1. What is the SKILL.md Ecosystem?

The SKILL.md ecosystem is a **vendor-neutral, declarative format** for packaging AI agent capabilities as self-describing Markdown files. It enables:

- **Portability** — Skills work across any compliant runtime (Ferris Aegis, LangChain, AutoGen, etc.)
- **Discoverability** — Skills can be found via structured manifests and well-known URLs
- **Safety** — Skills declare permissions, trust levels, and sandbox constraints
- **Composability** — Skills can depend on and invoke other skills
- **Verifiability** — Skills can be cryptographically signed and integrity-checked

---

## 2. Specification Stack (10 Layers)

| # | Layer | Purpose | Status |
|---|-------|---------|--------|
| 1 | **Specification** | Core SKILL.md format, frontmatter schema, 15 validation rules | ✅ Defined |
| 2 | **Runtime Extension** | Platform-specific blocks (`ferris_aegis:`, `langchain:`, etc.) | ✅ Defined |
| 3 | **Runtime Manifest** | Discovery index (`index.json`), registry layout | ✅ Implemented |
| 4 | **Trust** | Trust scoring, trust-gated activation | ✅ Implemented (Aegis) |
| 5 | **Cryptographic Identity** | Ed25519 signing, SHA-256 digests | ✅ Defined |
| 6 | **Lifecycle** | Versioning, deprecation, migration | ✅ Defined |
| 7 | **Capability Model** | Permission declarations, least-privilege | ✅ Defined |
| 8 | **Skill Composition** | Dependencies, orchestration, chaining | ✅ Defined |
| 9 | **Observability** | OTel spans, Prometheus metrics, JSON audit | ✅ Implemented (Aegis) |
| 10 | **Repository Layout** | Directory conventions, packaging | ✅ Defined |

---

## 3. Key Documents

| Document | Location | Purpose |
|----------|----------|---------|
| **SKILL_STANDARD_SPECIFICATION.md** | Root | Vendor-neutral Layer 1-10 specification |
| **SKILL_AEGIS_IMPLEMENTATION.md** | Root | Ferris Aegis reference implementation guide |
| **SKILL-SPEC.md** | Root | Original Aegis-specific spec (agentskills.io v0.2.0) |
| **SKILL_ECOSYSTEM_SUMMARY.md** | Root | This document |
| **CLAUDE.md** | Root | AI agent project context |

---

## 4. Skill Inventory

### 4.1 Ferris Aegis Skills (agentskills.io v0.2.0 format)

Located in `.agents/skills/`, these 10 skills are specific to the Ferris Aegis operating system:

| # | Skill | Crate | Trust Level | Invariants |
|---|-------|-------|-------------|------------|
| 1 | `aegis-trust-kernel` | kernel | Standard | INV-006, INV-010 |
| 2 | `aegis-security-pipeline` | security, sandbox-wasm | Elevated | INV-001–009 |
| 3 | `aegis-durable-workflow` | durable | Standard | INV-013, INV-014 |
| 4 | `aegis-policy-authoring` | kernel::policy | Standard | INV-010 |
| 5 | `aegis-agent-lifecycle` | kernel::agent, guard | Elevated | INV-006 |
| 6 | `aegis-resilience-ops` | resilience | Standard | INV-011, INV-012 |
| 7 | `aegis-mcp-server` | mcp, observability | Probationary | INV-004 |
| 8 | `aegis-crash-recovery` | durable | Standard | INV-013–015 |
| 9 | `aegis-session-supervisor` | session, supervisor, a2a | Standard | INV-006 |
| 10 | `aegis-skill-creator` | meta | Probationary | — |

### 4.2 Vendor-Neutral Example Skills (v1.0.0 format)

Located in `skills/examples/`, these demonstrate the vendor-neutral specification:

| # | Skill | Category | Trust Level | Permissions |
|---|-------|----------|-------------|-------------|
| 1 | `research-planner` | research | Standard | network.http.get, filesystem.read.tmp |
| 2 | `web-search` | search | Probationary | network.http.get |
| 3 | `code-reviewer` | analysis | Standard | filesystem.read.tmp, crypto.hash |

### 4.3 Dependency Graph

```
Ferris Aegis Skills:
  aegis-trust-kernel ←── aegis-agent-lifecycle
                     ←── aegis-policy-authoring
                     ←── aegis-security-pipeline
  aegis-durable-workflow ←── aegis-crash-recovery
  aegis-session-supervisor ←── aegis-trust-kernel
                            ←── aegis-resilience-ops
  aegis-mcp-server ←── aegis-security-pipeline
                    ←── aegis-trust-kernel
  aegis-resilience-ops ←── aegis-durable-workflow
  aegis-skill-creator  (standalone)

Vendor-Neutral Examples:
  research-planner ←── web-search
                   ←── source-evaluator (not yet implemented)
  web-search  (standalone)
  code-reviewer  (standalone)
```

---

## 5. Crate Implementation

### 5.1 `ferris-aegis-skills` (v0.4.0)

The Rust crate provides programmatic access to the SKILL.md ecosystem:

| Component | Purpose |
|-----------|---------|
| `SkillFrontmatter` | YAML frontmatter parsing (agentskills.io v0.2.0) |
| `SkillMetadata` | Tier 1 metadata (name, description, digest) |
| `Skill` | Tier 2 full skill (metadata + instructions + resources) |
| `SkillResources` | Tier 3 resource discovery (scripts, references, assets) |
| `SkillValidator` | 9-rule validation engine |
| `SkillRegistry` | Discovery, loading, and lifecycle management |
| `SkillIndex` | Discovery manifest generation (`.well-known/agent-skills/index.json`) |

### 5.2 Planned Extensions (v1.0.0 vendor-neutral)

| Component | Purpose | Status |
|-----------|---------|--------|
| `SkillId` | Namespaced ID (`skill:<category>:<name>`) | Defined |
| `Capability` | Permission model (`<domain>.<operation>`) | Defined |
| `TrustLevelRequired` | Trust-gated activation | Defined |
| `Dependency` | Versioned skill/tool/model dependencies | Defined |
| `DependencyResolver` | Topological sort, cycle detection | Defined |
| `PolicyRule` | Policy enforcement rules | Defined |
| `ResourceLimits` | Sandbox resource constraints | Defined |
| `Signature` | Ed25519 cryptographic signing | Defined |
| `SkillError` | Comprehensive error types | Defined |

---

## 6. 7-Layer Execution Model

When a skill is activated in Ferris Aegis, it passes through 7 layers:

```
1. Discovery     — Scan .agents/skills/ for SKILL.md files
2. Validation    — Check 15 Layer 1 + Layer 2 extension rules
3. Verify        — Cryptographic signature + digest verification
4. Resolve       — Dependency resolution + cycle detection
5. Policy        — Trust level + policy rules + permission grants
6. Sandbox       — Network, filesystem, compute constraints
7. Execute       — Run instructions, emit telemetry, record audit
```

Each layer can short-circuit execution. Invalid, untrusted, or unauthorized skills are rejected before execution.

---

## 7. Progressive Disclosure

The 3-tier model minimizes token overhead for agents with many skills:

| Tier | Content | Token Budget | When Loaded |
|------|---------|-------------|-------------|
| 1 — Metadata | name, description, id, version | ~100 tokens/skill | Startup |
| 2 — Instructions | SKILL.md body | <5,000 tokens | Activation |
| 3 — Resources | scripts/, references/, assets/ | On demand | Execution |

**Example**: 50 skills at startup = ~5,000 tokens. Only the activated skill's ~2,000 tokens load on demand.

---

## 8. Backward Compatibility

The v1.0.0 vendor-neutral spec is a **strict superset** of agentskills.io v0.2.0:

| v0.2.0 Field | v1.0.0 Field | Migration |
|-------------|-------------|-----------|
| `name` | `name` + `id` | Add `id` with category namespace |
| `description` | `description` | Same |
| `license` | `license` | Now required |
| `compatibility` | `platforms` | Structured array |
| `metadata.aegis-*` | `ferris_aegis:` block | Move to extension block |
| `allowed-tools` | `permissions` | Granular capability model |

Existing v0.2.0 skills continue to work. Migration is additive.

---

## 9. CLI Integration

```bash
# Existing (v0.2.0)
aegis skills list           # List all discovered skills
aegis skills validate       # Validate all skills
aegis skills show <name>    # Show skill details
aegis skills index          # Generate discovery index

# New (v1.0.0)
aegis skill run <id>        # Run a skill with inputs
aegis skill sign <id>       # Sign a skill with Ed25519
aegis skill verify <id>     # Verify a skill's signature
aegis skill publish <id>    # Publish to a registry
aegis skill deps <id>       # Show dependency tree
aegis skill history <id>    # Show execution history
aegis skill migrate <name>  # Migrate from v0.2.0 to v1.0.0
```

---

## 10. Observability

### OpenTelemetry
- Span hierarchy: `skill.activate → skill.validate → skill.verify → skill.resolve → skill.policy → skill.sandbox → skill.execute`
- Attributes: skill_id, skill_version, trust_level, agent_id, layer

### Prometheus
- `aegis_skill_activations_total` — Counter by skill, trust_level
- `aegis_skill_duration_seconds` — Histogram by skill, layer
- `aegis_skill_errors_total` — Counter by skill, layer, error_type
- `aegis_skill_sandbox_violations_total` — Counter by skill, violation_type

### Audit Trail
- Every skill execution recorded in AuditLedger
- SHA-256 chain integrity (INV-006)
- Structured JSON events with all 7 layer results

---

## 11. Security Invariant Coverage

| Invariant | Skill Enforcement |
|-----------|------------------|
| INV-001: Credential flow | Policy layer: deny-credential-leak |
| INV-002: No plaintext secrets | Policy layer: require crypto.encrypt |
| INV-003: Allowlist enforcement | Execute layer: tool allowlist check |
| INV-004: MCP protocol version | Validate layer: pinned version check |
| INV-005: Injection scan | Policy layer: input validation regex |
| INV-006: Audit chain | Execute layer: AuditLedger recording |
| INV-007: SSRF guard | Sandbox layer: network allowed_domains |
| INV-008: Rate limiting | Sandbox layer: max_requests |
| INV-009: WASM sandbox | Sandbox layer: wasm_module + fuel |
| INV-010: Config validation | Validate layer: frontmatter schema |
| INV-011: Circuit breaker | Policy layer: dependency health check |
| INV-012: Retry policy | Execute layer: timeout + retry |
| INV-013: Hash verification | Verify layer: SHA-256 digest |
| INV-014: Checkpoint every step | Execute layer: durable execution |
| INV-015: Resume from checkpoint | Execute layer: crash recovery |

---

## 12. Project Metrics

| Metric | Value |
|--------|-------|
| Crates | 14 (including ferris-aegis-skills) |
| Lines of Code | ~13,100+ |
| Integration Tests | 47+ |
| Aegis Skills (v0.2.0) | 10 |
| Vendor-Neutral Examples (v1.0.0) | 3 |
| Spec Layers | 10 |
| Execution Layers | 7 |
| Validation Rules (Layer 1) | 15 |
| Security Invariants Covered | 15 (INV-001 through INV-015) |
| Version | 0.4.0 |

---

*Version 1.0.0 — 2026-07-19 — Ferris Aegis Project*
