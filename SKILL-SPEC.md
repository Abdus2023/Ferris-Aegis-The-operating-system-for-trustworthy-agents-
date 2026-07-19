# Ferris Aegis — SKILL.md Specification & Architecture

> The complete Agent Skill Library for the Ferris Aegis Operating System for Trustworthy Agents.
> Compliant with the [Agent Skills Open Specification](https://agentskills.io) v0.2.0.
> Version: 0.4.0 | 13 crates | 10 skills | 47 integration tests

---

## 1. Architecture Overview

### 1.1 Design Philosophy

Ferris Aegis skills follow the **3-Tier Progressive Disclosure** model from the agentskills.io specification:

```
Tier 1 — Metadata (~100 tokens/skill):  name + description loaded at startup
Tier 2 — Instructions (<5,000 tokens):  SKILL.md body loaded on activation
Tier 3 — Resources (on demand):         scripts/, references/, assets/ loaded lazily
```

This means an agent with all 10 Aegis skills installed pays only ~1,000 tokens at startup for discovery metadata — the full instruction set (~40,000 tokens) loads only when a specific skill is triggered.

### 1.2 Skill-OS Mapping

Each skill maps to one or more Ferris Aegis crates, creating a 1:many relationship between skills and implementation modules:

```
┌──────────────────────────────────────────────────────────────┐
│                    Agent (Claude Code / Codex / Cursor)       │
│                                                              │
│  .agents/skills/                                             │
│  ├── aegis-trust-kernel/     ← kernel                       │
│  ├── aegis-security-pipeline/ ← security + sandbox-wasm      │
│  ├── aegis-durable-workflow/  ← durable                      │
│  ├── aegis-policy-authoring/  ← kernel::policy               │
│  ├── aegis-agent-lifecycle/   ← kernel::agent + guard        │
│  ├── aegis-resilience-ops/    ← resilience + kernel::health  │
│  ├── aegis-mcp-server/        ← mcp + observability          │
│  ├── aegis-crash-recovery/    ← durable::CrashRecovery       │
│  ├── aegis-session-supervisor/ ← session + supervisor + a2a  │
│  └── aegis-skill-creator/     ← meta: creates new skills     │
│                                                              │
└──────────────────────────────────────────────────────────────┘
         │
         ▼
┌──────────────────────────────────────────────────────────────┐
│              Ferris Aegis Crates (Rust)                       │
│  kernel | security | durable | resilience | mcp | session    │
│  supervisor | a2a | semantic-memory | memory | plugin | ...  │
└──────────────────────────────────────────────────────────────┘
```

### 1.3 Skill Dependency Graph

```
aegis-trust-kernel  ←── aegis-agent-lifecycle
                   ←── aegis-policy-authoring
                   ←── aegis-security-pipeline

aegis-durable-workflow ←── aegis-crash-recovery

aegis-session-supervisor ←── aegis-trust-kernel
                          ←── aegis-resilience-ops

aegis-mcp-server ←── aegis-security-pipeline
                  ←── aegis-trust-kernel

aegis-resilience-ops ←── aegis-durable-workflow

aegis-skill-creator  (standalone — no crate dependencies)
```

---

## 2. SKILL.md Specification — Ferris Aegis Extension

### 2.1 Base Spec Compliance

All Ferris Aegis skills comply with the [agentskills.io specification](https://agentskills.io/specification):

| Field | Required | Constraints |
|-------|----------|-------------|
| `name` | ✅ Yes | 1-64 chars, `^[a-z0-9]+(-[a-z0-9]+)*$`, must match directory name |
| `description` | ✅ Yes | 1-1024 chars, no `<` or `>`, must include "Use when..." trigger |
| `license` | No | SPDX identifier (e.g. `MIT OR Apache-2.0`) |
| `compatibility` | No | Max 500 chars, environment requirements |
| `metadata` | No | Key-value string map |
| `allowed-tools` | No | Space-delimited tool list (experimental) |

### 2.2 Ferris Aegis Extension Fields

These are stored in `metadata:` to maintain spec compliance. Unknown frontmatter keys are ignored by spec-compliant runtimes.

```yaml
metadata:
  # Aegis-specific extensions
  aegis-crate: "ferris-aegis-durable"        # Primary crate this skill wraps
  aegis-phase: "5.1"                          # Development phase
  aegis-depends: "aegis-trust-kernel"         # Skill dependency (space-separated)
  aegis-invariants: "INV-013 INV-014 INV-015" # Security invariants enforced
  version: "0.4.0"                            # Skill version
  author: "ferris-aegis"
  tags: "durable checkpoint recovery workflow"
```

### 2.3 Directory Layout Convention

```
.agents/skills/aegis-{skill-name}/
├── SKILL.md                 # Required: frontmatter + instructions
├── scripts/                 # Optional: executable helpers
│   └── validate-checkpoint.sh
├── references/              # Optional: detailed docs loaded on demand
│   ├── ARCHITECTURE.md      # Component architecture
│   └── API.md               # Type/function reference
└── assets/                  # Optional: templates, schemas
    └── workflow-template.toml
```

### 2.4 Token Budgets

| Skill Type | SKILL.md Body | Target Tokens |
|------------|---------------|---------------|
| Workflow/Process | < 500 lines | 400–800 |
| Tool Wrapper | < 200 lines | 150–400 |
| Reference/Quick-ref | < 150 lines | 80–250 |
| Emergency/Recovery | < 300 lines | Split into sub-skills < 800 each |

### 2.5 Validation

```bash
# Validate all skills against the agentskills.io spec
uvx --from git+https://github.com/agentskills/agentskills#subdirectory=skills-ref \
  skills-ref validate .agents/skills/aegis-trust-kernel

# Validate all Aegis skills at once
for skill in .agents/skills/aegis-*/; do
  skills-ref validate "$skill"
done
```

---

## 3. Security Invariant Enforcement via Skills

Each skill enforces the security invariants defined in `docs/ARCHITECTURE-QUICK-REF.md`. The skill instructions tell the agent what invariants apply and how to verify them.

| Skill | Enforced Invariants |
|-------|-------------------|
| aegis-trust-kernel | INV-006 (audit chain), INV-010 (config validation) |
| aegis-security-pipeline | INV-001–003 (credential flow), INV-005–009 |
| aegis-durable-workflow | INV-013 (hash verify), INV-014 (checkpoint every step) |
| aegis-crash-recovery | INV-013, INV-014, INV-015 (resume from checkpoint) |
| aegis-policy-authoring | INV-010 (config validation) |
| aegis-resilience-ops | INV-011 (circuit breaker), INV-012 (rate limiter) |

---

## 4. Skill Catalog

| # | Skill Name | Crate(s) | Phase | Triggers |
|---|-----------|----------|-------|----------|
| 1 | `aegis-trust-kernel` | kernel | 1 | "trust score", "trust level", "audit ledger", "trust lifecycle" |
| 2 | `aegis-security-pipeline` | security, sandbox-wasm | 3 | "injection scan", "SSRF", "credential", "allowlist", "WASM sandbox" |
| 3 | `aegis-durable-workflow` | durable | 5.1 | "durable execution", "checkpoint", "workflow", "step outcome" |
| 4 | `aegis-policy-authoring` | kernel::policy | 1 | "policy rule", "policy engine", "safety policy", "TOML policy" |
| 5 | `aegis-agent-lifecycle` | kernel::agent, guard | 1 | "spawn agent", "quarantine", "agent lifecycle", "guard action" |
| 6 | `aegis-resilience-ops` | resilience, kernel::health | 5 | "circuit breaker", "rate limit", "retry", "health check" |
| 7 | `aegis-mcp-server` | mcp, observability | 2 | "MCP server", "file_read", "tool handler", "MCP protocol" |
| 8 | `aegis-crash-recovery` | durable | 5.1 | "crash recovery", "resume workflow", "incomplete workflow", "recovery scan" |
| 9 | `aegis-session-supervisor` | session, supervisor, a2a | 4 | "session management", "supervisor anomaly", "A2A routing", "AgentCard" |
| 10 | `aegis-skill-creator` | meta | — | "create skill", "SKILL.md", "new skill", "skill template" |

---

## 5. Agent Execution Protocol

When a compatible agent (Claude Code, Codex, Cursor, etc.) works in a Ferris Aegis project:

1. **Discovery**: Agent scans `.agents/skills/` and loads `name` + `description` for each skill (~1,000 tokens total)
2. **Triggering**: When user's request semantically matches a skill's `description`, agent activates that skill
3. **Loading**: Agent reads the full `SKILL.md` body (under 5,000 tokens)
4. **Execution**: Agent follows the skill's step-by-step instructions
5. **Resource Loading**: Agent reads `references/`, runs `scripts/`, or loads `assets/` only when needed
6. **Verification**: Agent validates its work against the invariants listed in the skill
7. **Completion**: Agent marks the skill's checklist complete

### Cross-Platform Paths

| Agent | Skill Path |
|-------|-----------|
| Claude Code | `.claude/skills/` or `.agents/skills/` |
| OpenAI Codex | `.agents/skills/` |
| Cursor | `.cursor/skills/` |
| Gemini CLI | `.agents/skills/` |
| VS Code Copilot | `.agents/skills/` (symlink) |
| Roo Code | `.agents/skills/` |
| Cross-platform | `.agents/skills/` (canonical) |

---

## 6. Implementation Traceability

| Skill | Spec Ref | Crate | Test Criteria | ADRs |
|-------|----------|-------|---------------|------|
| aegis-trust-kernel | INV-006, INV-010 | kernel | 1–4 | ADR-001 |
| aegis-security-pipeline | INV-001–009 | security, sandbox-wasm | 1–8 | ADR-002, ADR-003 |
| aegis-durable-workflow | INV-013, INV-014 | durable | 30–39 | ADR-010–012 |
| aegis-crash-recovery | INV-013–015 | durable | 31, 35 | ADR-010 |
| aegis-resilience-ops | INV-011, INV-012 | resilience | 19–29 | ADR-001 |
| aegis-mcp-server | INV-004, ADR-007 | mcp, observability | observability, MCP | ADR-004, ADR-007 |

*Updated: 2026-07-19. Version 0.4.0. 10 skills, 13 crates, 47 integration tests.*
