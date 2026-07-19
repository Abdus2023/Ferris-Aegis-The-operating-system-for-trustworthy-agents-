# Ferris Aegis — Skill Specification Quick Reference

> Loaded on demand by the aegis-skill-creator skill.

## agentskills.io Specification v0.2.0

### Frontmatter Schema

| Field | Required | Type | Constraints |
|-------|----------|------|-------------|
| `name` | Yes | string | 1-64 chars, `^[a-z0-9]+(-[a-z0-9]+)*$`, matches directory |
| `description` | Yes | string | 1-1024 chars, no `<` or `>`, include "Use when..." |
| `license` | No | string | SPDX identifier |
| `compatibility` | No | string | Max 500 chars, environment requirements |
| `metadata` | No | map | String key-value pairs |
| `allowed-tools` | No | string/list | Space-delimited tool names |

### Directory Structure

```
skill-name/
├── SKILL.md          # Required (metadata + instructions)
├── scripts/          # Optional (executable code)
├── references/       # Optional (docs on demand)
└── assets/           # Optional (templates, data)
```

### Progressive Disclosure (3 Tiers)

1. **Metadata** (~100 tokens): `name` + `description` at startup
2. **Instructions** (<5,000 tokens): SKILL.md body on activation
3. **Resources** (on demand): scripts/, references/, assets/ when needed

### Validation Rules

- `name` must match parent directory name (Unicode NFKC normalization)
- `name` cannot start or end with hyphen, no consecutive hyphens
- `description` should include positive triggers ("Use when...") and negative triggers ("Do NOT use for...")
- SKILL.md body should be under 500 lines
- Avoid angle brackets in frontmatter (injection risk)

### Cross-Platform Paths

| Agent | Path |
|-------|------|
| Claude Code | `.claude/skills/` or `.agents/skills/` |
| OpenAI Codex | `.agents/skills/` |
| Cursor | `.cursor/skills/` |
| Gemini CLI | `.agents/skills/` |
| Cross-platform canonical | `.agents/skills/` |

### Discovery Endpoint (HTTP)

```
GET /.well-known/agent-skills/index.json
GET /.well-known/agent-skills/{name}/SKILL.md
```
