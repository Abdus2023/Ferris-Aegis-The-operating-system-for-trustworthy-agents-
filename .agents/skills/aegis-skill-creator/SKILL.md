---
name: aegis-skill-creator
description: >
  Creates new Agent Skills in the SKILL.md format for the Ferris Aegis ecosystem.
  Use when the user says "create skill", "new skill", "SKILL.md template", "skill
  authoring", or "skill creation". Produces skills compatible with Claude Code,
  Codex, Cursor, and 20+ other agents.
license: "MIT OR Apache-2.0"
compatibility: Requires a text editor and basic YAML knowledge. No Rust needed for skill creation.
metadata:
  aegis-crate: ""
  aegis-phase: ""
  aegis-depends: ""
  version: "0.4.0"
  author: "ferris-aegis"
  tags: "skill creation authoring template SKILL.md"
allowed-tools: Bash(mkdir:*) Bash(chmod:*) Read Write
---

# Ferris Aegis — Skill Creator

Create new Agent Skills in the SKILL.md format for the Ferris Aegis ecosystem.

## When to Use

- Creating a new skill for a Ferris Aegis capability
- Converting domain knowledge into a portable SKILL.md
- Generating skill boilerplate with correct frontmatter
- Validating skills against the agentskills.io specification

## Skill Structure

```
.agents/skills/aegis-{name}/
├── SKILL.md          # Required: metadata + instructions
├── scripts/          # Optional: executable code
├── references/       # Optional: docs loaded on demand
└── assets/           # Optional: templates, data files
```

## Workflow

1. Identify the domain knowledge to extract
2. Choose a skill name (lowercase, hyphens, matches directory)
3. Write YAML frontmatter with name, description, and extensions
4. Write markdown instructions with step-by-step workflow
5. Add edge cases and invariants
6. Move detailed reference material to references/
7. Validate with `skills-ref validate`

## Code Pattern — SKILL.md Template

```markdown
---
name: aegis-{skill-name}
description: >
  {One-sentence what the skill does}. Use when the user says
  "{trigger phrase 1}", "{trigger phrase 2}", or "{trigger phrase 3}".
  Do NOT use for {negative triggers}.
license: "MIT OR Apache-2.0"
compatibility: Requires Rust 1.82+ and ferris-aegis-{crate} crate
metadata:
  aegis-crate: "ferris-aegis-{crate}"
  aegis-phase: "{phase}"
  aegis-depends: "aegis-{dependency}"
  aegis-invariants: "INV-{number}"
  version: "0.4.0"
  author: "ferris-aegis"
  tags: "{tag1} {tag2} {tag3}"
allowed-tools: Bash(cargo:*) Read Write
---

# Ferris Aegis — {Skill Title}

{One-paragraph overview of what this skill does.}

## When to Use

- {Use case 1}
- {Use case 2}
- {Use case 3}

## Workflow

1. {Step 1}
2. {Step 2}
3. {Step 3}
4. {Step 4}

## Code Pattern

\```rust
// Example code showing the primary usage pattern
\```

## Invariants

- **INV-{N}**: {Invariant description and how to verify}

## Edge Cases

- {Edge case 1}
- {Edge case 2}
```

## Frontmatter Rules

| Field | Required | Rules |
|-------|----------|-------|
| `name` | Yes | 1-64 chars, `^[a-z0-9]+(-[a-z0-9]+)*$`, matches directory name |
| `description` | Yes | 1-1024 chars, no `<` or `>`, include "Use when..." trigger |
| `license` | No | SPDX identifier |
| `compatibility` | No | Max 500 chars, environment requirements |
| `metadata` | No | Key-value string map for custom properties |
| `allowed-tools` | No | Space-delimited tool list |

## Ferris Aegis Extension Fields (in metadata)

| Key | Purpose | Example |
|-----|---------|---------|
| `aegis-crate` | Primary Rust crate | `ferris-aegis-durable` |
| `aegis-phase` | Development phase | `5.1` |
| `aegis-depends` | Skill dependency | `aegis-trust-kernel` |
| `aegis-invariants` | Enforced invariants | `INV-013 INV-014` |

## Validation

```bash
# Validate a single skill
uvx --from git+https://github.com/agentskills/agentskills#subdirectory=skills-ref \
  skills-ref validate .agents/skills/aegis-{name}

# Validate all Aegis skills
for skill in .agents/skills/aegis-*/; do
  skills-ref validate "$skill" || echo "FAIL: $skill"
done
```

## Token Budget Guidelines

- SKILL.md body: under 500 lines, under 5,000 tokens
- Description: 2-4 sentences, ~100 tokens for discovery
- Move long reference material to references/ directory
- Keep examples minimal — one good example beats three vague ones
