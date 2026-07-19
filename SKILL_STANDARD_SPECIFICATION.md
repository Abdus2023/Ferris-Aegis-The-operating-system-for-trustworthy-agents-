# SKILL.md — Vendor-Neutral Agent Skill Specification

> **Version:** 1.0.0  
> **Status:** Stable  
> **License:** MIT OR Apache-2.0  
> **Compatibility:** Extends [agentskills.io v0.2.0](https://agentskills.io/specification)  

---

## 1. Purpose

The SKILL.md specification defines a **portable, declarative format** for packaging AI agent capabilities as self-describing Markdown files. A SKILL.md file contains:

- **Frontmatter** — Structured YAML metadata (identity, I/O contract, permissions, dependencies)
- **Body** — Natural-language instructions the agent follows when the skill is activated

The spec is **vendor-neutral** at Layer 1: any compliant runtime can discover, validate, and execute skills. Layer 2 defines optional runtime extension blocks that specific platforms (Ferris Aegis, LangChain, AutoGen, etc.) can use for advanced features without breaking Layer 1 compatibility.

---

## 2. 10-Layer Architecture

This specification is organized as a progressive stack of 10 layers. Implementations MAY support any subset of layers; Layer 1 is the only mandatory one.

| # | Layer | Purpose | Required |
|---|-------|---------|----------|
| 1 | **Specification** | Core SKILL.md format, frontmatter schema, validation rules | ✅ Yes |
| 2 | **Runtime Extension** | Platform-specific extension blocks (`ferris_aegis:`, `langchain:`, etc.) | No |
| 3 | **Runtime Manifest** | Discovery index (`index.json`), skill registry layout | No |
| 4 | **Trust** | Trust scoring, trust level alignment, trust-gated activation | No |
| 5 | **Cryptographic Identity** | Skill signing, signature verification, key management | No |
| 6 | **Lifecycle** | Skill versioning, deprecation, migration, upgrade paths | No |
| 7 | **Capability Model** | Permission declarations, capability resolution, least-privilege enforcement | No |
| 8 | **Skill Composition** | Skill dependencies, orchestration, fan-out/fan-in, chaining | No |
| 9 | **Observability** | Tracing spans, metrics, audit trails for skill execution | No |
| 10 | **Repository Layout** | Directory conventions, registry structure, packaging format | No |

---

## 3. File Format

A SKILL.md file is a Markdown document with YAML frontmatter delimited by `---`.

```markdown
---
spec_version: "1.0.0"
id: "skill:research:research-planner"
name: "research-planner"
version: "1.0.0"
description: >
  Orchestrates multi-step research workflows combining web search,
  source evaluation, and synthesis. Use when the user needs to
  research a topic, compare sources, or produce a research brief.
author: "example-org"
license: "MIT OR Apache-2.0"
runtime: "mcp"
platforms:
  - name: "ferris-aegis"
    min_version: "0.4.0"
  - name: "langchain"
    min_version: "0.1.0"
permissions:
  - "network.http.get"
  - "filesystem.read.tmp"
inputs:
  - name: "topic"
    type: "string"
    required: true
    description: "The research topic or question"
  - name: "depth"
    type: "enum:shallow,medium,deep"
    required: false
    default: "medium"
outputs:
  - name: "report"
    type: "markdown"
    description: "Synthesized research report"
  - name: "sources"
    type: "json[]"
    description: "Array of source objects with url, title, credibility"
entrypoint: "mcp://research-planner/execute"
timeout: 120
sandbox:
  network:
    allowed_domains: ["*.wikipedia.org", "arxiv.org", "*.github.com"]
  filesystem:
    allowed_paths: ["/tmp/research-*"]
required_context:
  - "web-search"
  - "source-evaluator"
optional_context:
  - "code-analyzer"
dependencies:
  skills:
    - id: "skill:search:web-search"
      version: ">=1.0.0"
    - id: "skill:analysis:source-evaluator"
      version: ">=0.5.0"
  tools:
    - name: "web_search"
      version: ">=1.0.0"
  models:
    - name: "gpt-4o"
      version: ">=2024-05-01"
      purpose: "synthesis"
validation:
  tests:
    - name: "basic_research"
      input: { "topic": "quantum computing", "depth": "shallow" }
      expected_output: { "has_report": true, "source_count_min": 3 }
    - name: "deep_research"
      input: { "topic": "RLHF alignment", "depth": "deep" }
      expected_output: { "has_report": true, "source_count_min": 10 }
  examples:
    - "Research the latest advances in sparse attention mechanisms"
    - "Compare WebGPU vs WebAssembly for browser-based ML inference"

# Research Planner Skill

## Overview

This skill orchestrates multi-step research workflows...

## Instructions

1. Parse the research topic and depth parameter
2. Delegate web searches to the `web-search` skill
3. Evaluate source credibility with `source-evaluator` skill
4. Synthesize findings into a structured markdown report
5. Return report and source list
```

---

## 4. Frontmatter Schema (Layer 1 — Required)

All Layer 1 fields are vendor-neutral. Compliant runtimes MUST parse these fields and MAY ignore any they don't implement.

### 4.1 Identity Fields

| Field | Type | Required | Constraints |
|-------|------|----------|-------------|
| `spec_version` | string | ✅ | Must be `"1.0.0"` |
| `id` | string | ✅ | Format: `skill:<category>:<name>` (lowercase, hyphens allowed) |
| `name` | string | ✅ | 1-64 chars, `^[a-z0-9]+(-[a-z0-9]+)*$`, must match directory name |
| `version` | string | ✅ | Semantic versioning (`MAJOR.MINOR.PATCH`) |
| `description` | string | ✅ | 1-1024 chars, no `<` or `>`, MUST include "Use when..." trigger |
| `author` | string | ✅ | Author or organization identifier |
| `license` | string | ✅ | SPDX identifier (e.g. `MIT OR Apache-2.0`) |

### 4.2 Runtime & Compatibility

| Field | Type | Required | Constraints |
|-------|------|----------|-------------|
| `runtime` | string | No | Execution model: `"mcp"`, `"cli"`, `"http"`, `"wasm"`, `"native"` |
| `platforms` | array | No | Compatibility matrix (see below) |
| `entrypoint` | string | No | Runtime-specific invocation target |
| `timeout` | integer | No | Max execution time in seconds (default: 60) |

**Platform entry:**

```yaml
platforms:
  - name: "ferris-aegis"
    min_version: "0.4.0"
    max_version: "1.0.0"  # optional
  - name: "langchain"
    min_version: "0.1.0"
```

### 4.3 Permission Model

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `permissions` | array of strings | No | Capability declarations in `<domain>.<operation>` format |

**Permission domains:**

| Domain | Operations | Example |
|--------|-----------|---------|
| `network` | `http.get`, `http.post`, `dns.resolve`, `websocket.connect` | `network.http.get` |
| `filesystem` | `read.<path>`, `write.<path>`, `exec.<path>` | `filesystem.read.tmp` |
| `process` | `spawn`, `signal`, `inspect` | `process.spawn` |
| `crypto` | `sign`, `verify`, `encrypt`, `decrypt`, `hash` | `crypto.hash` |
| `memory` | `read`, `write`, `delete` | `memory.read` |
| `agent` | `spawn`, `communicate`, `supervise` | `agent.communicate` |
| `compute` | `wasm.execute`, `gpu.access` | `compute.wasm.execute` |

### 4.4 I/O Contract

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `inputs` | array of Input | No | Typed input parameters |
| `outputs` | array of Output | No | Typed output declarations |

**Input entry:**

```yaml
inputs:
  - name: "topic"
    type: "string"
    required: true
    description: "The research topic"
    default: null        # optional, used when required=false
    validation: "^[\\w\\s-]+$"  # optional regex constraint
```

**Output entry:**

```yaml
outputs:
  - name: "report"
    type: "markdown"
    description: "The synthesized research report"
```

**Supported types:** `string`, `integer`, `float`, `boolean`, `json`, `json[]`, `markdown`, `enum:A,B,C`, `path`, `url`

### 4.5 Sandbox Constraints

```yaml
sandbox:
  network:
    allowed_domains: ["*.wikipedia.org", "arxiv.org"]
    max_requests: 100
  filesystem:
    allowed_paths: ["/tmp/research-*", "/data/cached/*"]
    max_file_size: "10MB"
  compute:
    max_memory: "512MB"
    max_cpu_seconds: 60
```

### 4.6 Context & Dependencies

```yaml
required_context:     # Skills/tools that MUST be available
  - "web-search"
  - "source-evaluator"

optional_context:     # Skills/tools that ENHANCE the skill
  - "code-analyzer"
  - "citation-formatter"

dependencies:
  skills:             # Other skills this skill invokes
    - id: "skill:search:web-search"
      version: ">=1.0.0"
      optional: false
  tools:              # External tools required
    - name: "web_search"
      version: ">=1.0.0"
  models:             # AI models required
    - name: "gpt-4o"
      version: ">=2024-05-01"
      purpose: "synthesis"
```

### 4.7 Validation

```yaml
validation:
  tests:
    - name: "basic_test"
      input: { "topic": "quantum computing" }
      expected_output: { "has_report": true }
      timeout: 30
  examples:
    - "Research the latest advances in sparse attention"
    - "Compare WebGPU vs WebAssembly for browser-based ML"
```

---

## 5. Runtime Extension Blocks (Layer 2)

Runtime-specific extensions are namespaced under the runtime name. This ensures Layer 1 compatibility: unknown blocks are ignored by other runtimes.

### 5.1 Ferris Aegis Extension

```yaml
ferris_aegis:
  trust_level: "Standard"              # Required trust level to activate
  policies:                            # Policy rules enforced during execution
    - "deny-network-unless-elevated"
    "allow-filesystem-read-tmp"
  audit: true                          # Enable audit trail for this skill
  signature:                           # Cryptographic signature
    algorithm: "ed25519"
    public_key: "sha256:abcdef..."
    value: "ed25519:123456..."
  sandbox:                             # Aegis-specific sandbox overrides
    wasm_module: "research-planner.wasm"
    fuel: 10000
    memory_limit: "64MB"
```

### 5.2 Other Runtime Extensions (Examples)

```yaml
langchain:
  chain_type: "sequential"
  memory: "buffer"
  callbacks: ["tracing", "streaming"]

autogen:
  agent_type: "assistant"
  max_rounds: 5
  human_input: false
```

### 5.3 Extension Rules

1. Extension blocks MUST be namespaced under a lowercase identifier matching the runtime/platform name
2. Extension blocks MUST NOT override Layer 1 fields
3. Runtimes MUST ignore unknown extension blocks
4. Extension block keys SHOULD follow `<vendor>_<feature>` snake_case convention

---

## 6. Backward Compatibility with agentskills.io v0.2.0

The vendor-neutral spec is a **strict superset** of agentskills.io v0.2.0. Skills written for v0.2.0 continue to work:

| agentskills.io v0.2.0 | Vendor-Neutral v1.0.0 | Notes |
|----------------------|----------------------|-------|
| `name` | `name` + `id` | `id` adds category namespace |
| `description` | `description` | Same semantics |
| `license` | `license` | Same, now required |
| `compatibility` | `platforms` | More structured |
| `metadata` | Extension blocks | `metadata.aegis-*` → `ferris_aegis:` block |
| `allowed-tools` | `permissions` | More granular capability model |

**Migration path:**

1. Add `spec_version: "1.0.0"`, `id`, `version`, `author` fields
2. Replace `metadata.aegis-*` keys with `ferris_aegis:` extension block
3. Replace `compatibility` with `platforms` array
4. Replace `allowed-tools` with `permissions` array
5. Add `inputs`/`outputs` for I/O contract
6. Add `dependencies` for skill composition

---

## 7. Directory Layout Convention

```
skills/
├── research-planner/
│   ├── SKILL.md                 # Required: frontmatter + instructions
│   ├── scripts/                 # Optional: executable helpers
│   │   └── fetch-sources.sh
│   ├── references/              # Optional: detailed docs loaded on demand
│   │   ├── ARCHITECTURE.md
│   │   └── API.md
│   └── assets/                  # Optional: templates, schemas, configs
│       └── report-template.md
├── web-search/
│   ├── SKILL.md
│   └── ...
└── manifest.json                # Registry index
```

---

## 8. Validation Rules

### 8.1 Mandatory Checks (Layer 1)

| # | Rule | Severity |
|---|------|----------|
| 1 | SKILL.md file exists | Error |
| 2 | Frontmatter starts with `---` and ends with `---` | Error |
| 3 | `spec_version` is `"1.0.0"` | Error |
| 4 | `id` matches `^skill:[a-z0-9-]+:[a-z0-9-]+$` | Error |
| 5 | `name` matches `^[a-z0-9]+(-[a-z0-9]+)*$` | Error |
| 6 | `name` matches directory name | Error |
| 7 | `version` matches semver `^\d+\.\d+\.\d+$` | Error |
| 8 | `description` is 1-1024 chars, no `<>` | Error |
| 9 | `description` includes "Use when" trigger phrase | Warning |
| 10 | `license` is a valid SPDX identifier | Error |
| 11 | All declared `permissions` follow `<domain>.<operation>` format | Error |
| 12 | All `inputs` have `name`, `type`, `required` fields | Error |
| 13 | All `outputs` have `name`, `type` fields | Error |
| 14 | Dependency `id` fields follow `skill:<category>:<name>` format | Error |
| 15 | No angle brackets in any frontmatter value | Error |

### 8.2 Extended Checks (Layer 2+)

| # | Rule | Layer | Severity |
|---|------|-------|----------|
| 16 | `ferris_aegis.trust_level` is a valid TrustLevel | 4 | Error |
| 17 | `ferris_aegis.signature` has required fields | 5 | Error |
| 18 | Dependency cycle detection | 8 | Error |
| 19 | Entry count in `validation.tests` ≥ 1 | 1 | Warning |
| 20 | `sandbox.network.allowed_domains` is non-empty if `network.*` in permissions | 7 | Warning |

---

## 9. Skill ID Namespaces

The `id` field follows the format `skill:<category>:<name>` to enable federated skill registries:

| Category | Purpose | Examples |
|----------|---------|---------|
| `search` | Search and retrieval | `skill:search:web-search`, `skill:search:code-search` |
| `research` | Research and synthesis | `skill:research:research-planner` |
| `analysis` | Code/data analysis | `skill:analysis:code-reviewer`, `skill:analysis:source-evaluator` |
| `security` | Security operations | `skill:security:injection-scan`, `skill:security:ssrf-guard` |
| `workflow` | Orchestration | `skill:workflow:durable-executor`, `skill:workflow:crash-recovery` |
| `trust` | Trust management | `skill:trust:trust-kernel`, `skill:trust:audit-ledger` |
| `communication` | Agent-to-agent | `skill:communication:a2a-router` |
| `meta` | Skill creation | `skill:meta:skill-creator` |

---

## 10. Progressive Disclosure Model

Compliant runtimes SHOULD implement the 3-tier progressive disclosure model:

| Tier | Content | Token Budget | When Loaded |
|------|---------|-------------|-------------|
| 1 — Metadata | `name`, `description`, `id`, `version` | ~100 tokens/skill | Startup |
| 2 — Instructions | SKILL.md body (markdown instructions) | <5,000 tokens | Activation |
| 3 — Resources | `scripts/`, `references/`, `assets/` | On demand | Execution |

This means an agent with 50 skills installed pays only ~5,000 tokens at startup for discovery — the full instruction set loads only when a specific skill is triggered.

---

## 11. Discovery Index (Layer 3)

The discovery index is a JSON manifest served at a well-known URL, enabling programmatic skill discovery.

**Location:** `/.well-known/agent-skills/index.json`

```json
{
  "$schema": "https://schemas.agentskills.io/discovery/1.0.0/schema.json",
  "spec_version": "1.0.0",
  "registry": "local",
  "updated_at": "2026-07-19T00:00:00Z",
  "skills": [
    {
      "id": "skill:research:research-planner",
      "name": "research-planner",
      "version": "1.0.0",
      "type": "skill-md",
      "description": "Orchestrates multi-step research...",
      "url": "/.well-known/agent-skills/research-planner/SKILL.md",
      "digest": "sha256:a1b2c3...",
      "runtime": "mcp",
      "permissions": ["network.http.get", "filesystem.read.tmp"],
      "author": "example-org"
    }
  ]
}
```

---

## 12. Cryptographic Signing (Layer 5)

Skills MAY be cryptographically signed to establish provenance and integrity.

```yaml
signature:
  algorithm: "ed25519"
  public_key: "sha256:abcdef1234567890..."
  value: "ed25519:SIGNATURE_BYTES_BASE64..."
  signed_at: "2026-07-19T12:00:00Z"
  signed_by: "example-org"
```

**Verification steps:**

1. Compute SHA-256 digest of the SKILL.md content (excluding the `signature.value` field)
2. Verify the Ed25519 signature against the public key and digest
3. Check `signed_at` is within the validity window
4. Optionally verify the public key against a trusted root CA

---

## 13. Lifecycle States (Layer 6)

Skills follow a defined lifecycle:

```
Draft → Stable → Deprecated → Retired
```

| State | Description |
|-------|-------------|
| `draft` | Under development, not for production use |
| `stable` | Production-ready, maintained |
| `deprecated` | Still functional but superseded; will be retired |
| `retired` | No longer supported; do not use |

Lifecycle state is communicated via the `lifecycle` field:

```yaml
lifecycle:
  state: "stable"
  since: "2026-07-19"
  deprecation_notice: null
  replacement: null
  migration_guide: null
```

---

## 14. Conformance

A runtime claims conformance at a specific layer:

- **Layer 1 Conformance**: Parse and validate Layer 1 frontmatter fields
- **Layer 2 Conformance**: Layer 1 + process extension blocks for at least one runtime
- **Layer 3 Conformance**: Layer 2 + serve/generate discovery index
- **Full Conformance**: Layers 1-10

Runtimes MUST NOT fail when encountering unknown extension blocks. They SHOULD log a warning and continue processing.

---

## Appendix A: Minimal Valid SKILL.md

```markdown
---
spec_version: "1.0.0"
id: "skill:example:hello-world"
name: "hello-world"
version: "1.0.0"
description: "A minimal example skill. Use when greeting the user."
author: "example-org"
license: "MIT"
---

# Hello World

Say "Hello, World!" to the user.
```

## Appendix B: Full SKILL.md with All Layers

```markdown
---
spec_version: "1.0.0"
id: "skill:research:research-planner"
name: "research-planner"
version: "1.2.0"
description: >
  Orchestrates multi-step research workflows combining web search,
  source evaluation, and synthesis. Use when the user needs to
  research a topic, compare sources, or produce a research brief.
author: "example-org"
license: "MIT OR Apache-2.0"
runtime: "mcp"
platforms:
  - name: "ferris-aegis"
    min_version: "0.4.0"
  - name: "langchain"
    min_version: "0.1.0"
permissions:
  - "network.http.get"
  - "filesystem.read.tmp"
inputs:
  - name: "topic"
    type: "string"
    required: true
    description: "The research topic or question"
  - name: "depth"
    type: "enum:shallow,medium,deep"
    required: false
    default: "medium"
outputs:
  - name: "report"
    type: "markdown"
    description: "Synthesized research report"
  - name: "sources"
    type: "json[]"
    description: "Array of source objects"
entrypoint: "mcp://research-planner/execute"
timeout: 120
sandbox:
  network:
    allowed_domains: ["*.wikipedia.org", "arxiv.org"]
  filesystem:
    allowed_paths: ["/tmp/research-*"]
required_context:
  - "web-search"
optional_context:
  - "code-analyzer"
dependencies:
  skills:
    - id: "skill:search:web-search"
      version: ">=1.0.0"
  tools:
    - name: "web_search"
      version: ">=1.0.0"
validation:
  tests:
    - name: "basic_research"
      input: { "topic": "quantum computing" }
      expected_output: { "has_report": true }
  examples:
    - "Research the latest advances in sparse attention"
lifecycle:
  state: "stable"
  since: "2026-07-19"
ferris_aegis:
  trust_level: "Standard"
  policies: ["deny-network-unless-elevated"]
  audit: true
  signature:
    algorithm: "ed25519"
    public_key: "sha256:abcdef..."
    value: "ed25519:signature..."
  sandbox:
    wasm_module: "research-planner.wasm"
    fuel: 10000
    memory_limit: "64MB"
---

# Research Planner Skill

## Overview

Orchestrates multi-step research workflows combining web search,
source evaluation, and synthesis into structured reports.

## Instructions

1. Parse the research topic and depth parameter
2. Delegate web searches to the `web-search` skill
3. Evaluate source credibility with the `source-evaluator` skill
4. Synthesize findings into a structured markdown report
5. Return the report and source list

## Quality Checklist

- [ ] At least 3 sources consulted for shallow depth
- [ ] Sources include both primary and secondary references
- [ ] Report follows the structured template
- [ ] All claims are attributed to sources
```

---

*Version 1.0.0 — 2026-07-19 — Ferris Aegis Project*
