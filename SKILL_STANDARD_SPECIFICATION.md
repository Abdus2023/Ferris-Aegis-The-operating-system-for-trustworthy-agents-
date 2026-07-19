# SKILL.md Open Standard Specification

**Version:** 1.0.0  
**Status:** Portable Standard  
**Maintainers:** Open Specification Community  
**License:** CC-BY-4.0 (specification), Apache 2.0 (reference implementation)

---

## Executive Summary

**SKILL.md** is a vendor-neutral, portable format for packaging AI agent capabilities as composable, verifiable units. Like OCI for containers or OpenAPI for REST APIs, SKILL.md defines the standard contract that allows skills to be:

- **Discovered** across repositories and registries
- **Verified** cryptographically before execution
- **Composed** into workflows and pipelines
- **Executed** safely across heterogeneous runtimes (Claude, Copilot, Cursor, Ferris Aegis, any MCP-compatible system)
- **Audited** through standardized observability events

---

## Table of Contents

1. [Specification Layers](#specification-layers)
2. [Core Metadata](#core-metadata)
3. [Capability Model](#capability-model)
4. [Dependency Resolution](#dependency-resolution)
5. [Execution Contract](#execution-contract)
6. [Verification & Signatures](#verification--signatures)
7. [Observable Events](#observable-events)
8. [Runtime Implementations](#runtime-implementations)
9. [Repository Layout](#repository-layout)

---

## Specification Layers

```
┌──────────────────────────────────────────────────────────────────┐
│ Layer 1: SKILL.md Specification (Vendor-Neutral)                │
│ • Portable contract: metadata, capabilities, dependencies, I/O   │
└──────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌──────────────────────────────────────────────────────────────────┐
│ Layer 2: Runtime Extensions (Optional)                          │
│ • Ferris Aegis: trust-level, policies, audit, signatures        │
│ • Claude Code: execution context, telemetry                     │
│ • Cursor: language-specific bindings                            │
│ • (Other runtimes ignore unknown keys)                          │
└──────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌──────────────────────────────────────────────────────────────────┐
│ Layer 3: Executable Manifest (Runtime-Specific)                 │
│ • Parsed metadata, resolved dependencies, permission graph      │
│ • Capability graph, execution plan, sandbox configuration       │
└──────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌──────────────────────────────────────────────────────────────────┐
│ Layer 4: Execution (Sandboxed, Audited)                        │
│ • Verify signature, check policies, bind capabilities           │
│ • Execute within declared sandbox, emit observability events    │
│ • Record audit trail (Ferris Aegis) or telemetry (others)      │
└──────────────────────────────────────────────────────────────────┘
```

---

## Core Metadata

Every SKILL.md starts with portable frontmatter:

```yaml
---
# Specification Compliance
spec_version: "1.0.0"          # SKILL.md spec version

# Identity & Discovery
id: "research-planner"         # Unique identifier
name: "Research Planner"       # Human-readable name
version: "1.2.0"               # Semantic version
description: "Plans research tasks with web search"
author: "Acme Corp"
license: "MIT"
repository: "https://github.com/example/research-planner"

# Compatibility Matrix
runtime: "mcp"                 # Base runtime (mcp | rpc | http | wasm)
platforms:                     # Target platforms
  - name: "claude-code"
    min_version: "1.0"
  - name: "cursor"
    min_version: "0.36"
  - name: "github-copilot"
    min_version: "2.0"
  - name: "ferris-aegis"
    min_version: "0.3.0"

# Capability Declaration (Portable)
permissions:
  - filesystem.read             # Read files
  - filesystem.write            # Write files (if needed)
  - network.http                # HTTP requests
  - mcp.call                     # Call MCP tools
  - memory.query                 # Query episodic memory (optional)

# Input/Output Contract
inputs:
  query:
    type: string
    description: "Research topic to plan"
    required: true
  depth:
    type: integer
    description: "Search depth (1-5)"
    default: 2

outputs:
  research_plan:
    type: object
    properties:
      steps:
        type: array
        items: { type: string }
      estimated_time:
        type: string
        example: "45 minutes"

# Execution Model
entrypoint: "run"              # Entry function/command
timeout: "300s"                # Max execution time
sandbox: "required"            # none | optional | required
network:
  allowed_domains:
    - "api.openai.com"
    - "wikipedia.org"
    - "scholar.google.com"
filesystem:
  allowed_paths:
    - "/tmp/*"
    - "$HOME/.cache/research/*"

# Required Context (Runtime-Dependent)
required_context:
  - "user_api_key"             # Must be provided by runtime
  - "web_search_tool"          # Must be available

optional_context:
  - "memory_store"             # Nice to have
  - "embedding_service"

# Dependencies
dependencies:
  skills:
    - id: "web-search"
      version: ">=1.0.0"
    - id: "markdown-parser"
      version: ">=2.0.0"
      optional: true
  tools:
    - name: "curl"
      version: ">=7.68"
  models:
    - name: "claude-opus"
      version: "latest"

# Validation & Testing
validation:
  tests:
    - name: "basic_planning"
      input: { query: "AI safety" }
      expected_output: { steps: ["?" ] }
  examples:
    - input: { query: "quantum computing", depth: 3 }
      description: "Deep research on quantum computing"

---
```

---

## Capability Model

SKILL.md defines a portable capability namespace:

| Category | Operations | Scope |
|----------|-----------|-------|
| `filesystem` | read, write, delete, list | paths, patterns |
| `network` | http, websocket, dns | domains, ports |
| `memory` | query, store, search | episodic, semantic, session |
| `mcp` | call | tool, prompt, resource |
| `terminal` | exec | commands (restricted) |
| `git` | read, write, clone | repos, branches |
| `wasm` | execute | modules, memory |
| `plugin` | load, call | verified plugins |
| `model` | invoke | model names/versions |

**Syntax:** `<category>.<operation>[.<scope>]`

Examples:
- `filesystem.read` — read any file
- `filesystem.read./tmp/*` — read only in /tmp
- `network.http.wikipedia.org` — HTTP calls to Wikipedia only
- `memory.query.episodic` — query episodic memory
- `mcp.call.web-search` — call the web-search tool

---

## Dependency Resolution

Skills can depend on other skills, tools, or models:

```yaml
dependencies:
  skills:
    - id: "web-search"
      version: ">=1.0.0"
      fallback: true           # Use alternative if unavailable
  tools:
    - name: "curl"
      version: ">=7.68"
  models:
    - name: "claude-3-opus"
    - name: "gpt-4"            # Multiple models supported
```

**Resolution Algorithm:**

1. **Discovery** — Check local registry, then published registries (npm, PyPI, Hugging Face, etc.)
2. **Version Constraint** — Validate version matches semver constraint
3. **Cycle Detection** — Ensure no circular dependencies
4. **Signature Verification** — For each dependency, verify Ed25519 signature
5. **Capability Check** — Ensure runtime supports required capabilities
6. **Download/Cache** — Fetch and cache resolved skills

---

## Execution Contract

### Lifecycle

```
Discover
  ↓ Load SKILL.md from file/registry
  ↓
Validate
  ↓ Check schema, fields, formats
  ↓
Verify Signature
  ↓ Ed25519 verify against public key
  ↓
Resolve Dependencies
  ↓ Recursively resolve, detect cycles
  ↓
Check Compatibility
  ↓ Runtime supports all required capabilities?
  ↓
Load (Cached)
  ↓ Parse, compile, cache compiled form
  ↓
Prepare Sandbox
  ↓ Bind capabilities, set resource limits, configure filesystem/network
  ↓
Execute
  ↓ Call entrypoint with inputs
  ↓
Record Events
  ↓ Emit SkillStarted, ToolInvoked, CapabilityGranted, Completed events
  ↓
Unload
  ↓ Clean up resources
```

### Execution Context (Runtime-Provided)

Every skill receives a context object:

```rust
{
  execution_id: UUID,
  skill_id: "research-planner",
  skill_version: "1.2.0",
  
  // Capabilities granted by runtime
  capabilities: [
    "filesystem.read",
    "network.http",
    "mcp.call.web-search"
  ],
  
  // Provided context
  context: {
    user_api_key: "sk-...",
    web_search_tool: { /* MCP tool def */ }
  },
  
  // Sandbox constraints
  sandbox: {
    timeout: 300s,
    memory_limit: 256MB,
    filesystem_root: "/tmp/skill-xyz",
    allowed_domains: ["wikipedia.org", "openai.com"]
  },
  
  // Observable outputs
  events: EventEmitter
}
```

---

## Verification & Signatures

### Signing Workflow

```
1. Author creates SKILL.md
   │
   ▼
2. Generate manifest.json (deterministic JSON of metadata)
   │
   ▼
3. Hash with SHA-256
   │
   ▼
4. Sign with Ed25519 private key
   │
   ▼
5. Distribute: SKILL.md + manifest.json + SKILL.sig
```

### File Structure

```
research-planner/
  SKILL.md                      # Portable specification
  manifest.json                 # Deterministic JSON representation
  SKILL.sig                      # Ed25519 signature (hex)
  example.py                    # Implementation
  tests/
    test_basic.py
  README.md
```

### Verification Algorithm

```rust
// 1. Load SKILL.md, parse frontmatter
let skill = parse_frontmatter("SKILL.md")?;

// 2. Load manifest.json, verify it matches SKILL.md metadata
let manifest = load_manifest("manifest.json")?;
verify_manifest_matches_skill(&skill, &manifest)?;

// 3. Compute SHA-256 of manifest
let manifest_hash = sha256(read_file("manifest.json"));

// 4. Load signature and public key
let signature = load_signature("SKILL.sig")?;
let public_key = get_public_key_from_registry(&skill.author)?;

// 5. Verify Ed25519 signature
ed25519_verify(manifest_hash, signature, public_key)?;

// 6. Trust established ✓
```

---

## Observable Events

Every skill execution emits standardized events (compatible with OpenTelemetry):

```json
{
  "timestamp": "2026-07-19T11:23:45Z",
  "skill_id": "research-planner",
  "skill_version": "1.2.0",
  "execution_id": "abc-123-xyz",
  "agent_id": "agent-001",
  "event_type": "skill.started",
  "attributes": {
    "entrypoint": "run",
    "timeout_seconds": 300,
    "input_size_bytes": 156
  }
}
```

### Event Types

| Event | When | Attributes |
|-------|------|-----------|
| `skill.discovered` | Found in registry | skill_id, version |
| `skill.loaded` | Parsed & compiled | duration_ms |
| `skill.dependency_resolved` | Dependency satisfied | dep_id, dep_version |
| `skill.sandbox_prepared` | Capabilities bound | capabilities, limits |
| `skill.started` | Execution begins | inputs |
| `tool.invoked` | MCP/other tool called | tool_name, args |
| `capability.granted` | Capability used | capability_name |
| `capability.denied` | Capability rejected | capability_name, reason |
| `policy.violated` | Policy check failed | policy_id, reason |
| `checkpoint` | Progress marker | message |
| `skill.completed` | Success | output, duration_ms |
| `skill.failed` | Exception/timeout | error_code, message |

### Example Event Stream

```json
[
  { "event": "skill.started", "skill_id": "research-planner" },
  { "event": "tool.invoked", "tool": "web-search", "query": "AI safety" },
  { "event": "tool.completed", "tool": "web-search", "results": 42 },
  { "event": "checkpoint", "message": "Phase 1: Information gathering complete" },
  { "event": "capability.granted", "capability": "filesystem.write" },
  { "event": "skill.completed", "output": { "steps": [...] }, "duration_ms": 2340 }
]
```

---

## Runtime Implementations

### Base Runtime (All Implementations Must Support)

```yaml
spec_version: "1.0.0"
id: string
name: string
version: semver
description: string
permissions: [capability]
inputs: { name: { type, required } }
outputs: { name: { type } }
entrypoint: string | function
timeout: duration
```

### Ferris Aegis Extension

Optional keys that Ferris Aegis recognizes:

```yaml
ferris_aegis:
  trust_level:
    minimum: "probationary"   # Unverified | Probationary | Standard | Elevated | Sovereign
    preferred: "standard"
  
  policies:
    - id: "workspace-only"
      rule: "filesystem.read paths must be within $WORKSPACE"
      effect: "deny"
    - id: "no-symlink-escape"
      effect: "deny"
  
  audit:
    required: true            # Must record to audit ledger
    chain_type: "sha256"      # Cryptographic chain
  
  signature:
    algorithm: "ed25519"
    required: true
  
  sandbox:
    capability_boundary: "restricted"  # restricted | standard | elevated | sovereign
    resource_limits:
      max_memory: "256MB"
      max_file_size: "100MB"
      max_concurrent: 5
```

### Claude Code Extension

```yaml
claude_code:
  execution_context: "browser"  # browser | terminal | embedded
  language: "python"
  framework: "claude-sdk@2.0"
```

### Cursor Extension

```yaml
cursor:
  language_bindings:
    - typescript
    - python
  vscode_plugin: true
```

---

## Repository Layout

### Publisher Structure

```
skills-registry/
  ├── skills/
  │   ├── research-planner/
  │   │   ├── SKILL.md
  │   │   ├── manifest.json
  │   │   ├── SKILL.sig
  │   │   ├── README.md
  │   │   ├── example.py
  │   │   ├── tests/
  │   │   │   ├── test_basic.py
  │   │   │   └── test_fixtures.json
  │   │   └── .meta/
  │   │       ├── published_at.txt
  │   │       └── checksums.sha256
  │   │
  │   ├── web-search/
  │   │   ├── SKILL.md
  │   │   ├── manifest.json
  │   │   ├── SKILL.sig
  │   │   └── ...
  │   │
  │   └── markdown-parser/
  │       ├── SKILL.md
  │       ├── manifest.json
  │       ├── SKILL.sig
  │       └── ...
  │
  ├── registry.json              # Index of all skills
  └── .registry/
      ├── public_keys/
      │   ├── acme-corp.pub      # Author public keys
      │   └── research-team.pub
      └── checksums.json         # Signed checksums
```

### Central Registry Format (e.g., skill.dev, npm, PyPI)

```json
{
  "skills": [
    {
      "id": "research-planner",
      "name": "Research Planner",
      "version": "1.2.0",
      "author": "Acme Corp",
      "license": "MIT",
      "description": "Plans research tasks with web search",
      "download_url": "https://skill-registry.dev/research-planner/1.2.0/SKILL.md",
      "signature_url": "https://skill-registry.dev/research-planner/1.2.0/SKILL.sig",
      "manifest_url": "https://skill-registry.dev/research-planner/1.2.0/manifest.json",
      "published_at": "2026-07-19T11:00:00Z",
      "tags": ["research", "web-search", "planning"],
      "popularity": 512,
      "dependencies": {
        "skills": ["web-search@>=1.0.0"],
        "tools": ["curl@>=7.68"],
        "models": ["claude-3-opus"]
      },
      "supported_runtimes": [
        "claude-code",
        "cursor",
        "github-copilot",
        "ferris-aegis"
      ]
    },
    { /* more skills */ }
  ]
}
```

---

## Long-Term Vision

SKILL.md can evolve into an ecosystem comparable to:

- **OCI Image Spec** for containers
- **OpenAPI** for REST APIs
- **WASI Component Model** for WebAssembly
- **MCP** for tool interoperability

### Composability

```
Research Skill
    ↓ (outputs: research_plan)
    ↓
Planning Skill
    ↓ (outputs: implementation_plan)
    ↓
Coding Skill
    ↓ (outputs: code)
    ↓
Verification Skill
    ↓ (outputs: test_results)
    ↓
Deployment Skill
    ↓ (outputs: deployment_report)
```

### Portability Matrix

| Runtime | SKILL.md Support | Execution Model | Audit |
|---------|------------------|-----------------|-------|
| Claude Code | ✅ Native | Browser/Terminal | ⚠️ Optional |
| Cursor | ✅ Native | VSCode Plugin | ⚠️ Optional |
| GitHub Copilot | ✅ via MCP | GitHub Codespaces | ⚠️ GitHub logs |
| Gemini CLI | ✅ via HTTP-RPC | CLI Commands | ⚠️ Cloud Logging |
| OpenAI Codex | ✅ via HTTP-RPC | HTTP API | ❌ None |
| Ferris Aegis | ✅ Native | Sandbox + Audit | ✅ Full Audit Ledger |

---

## Governance & Evolution

- **Specification Maintainer:** Open community (CC-BY-4.0)
- **Version Management:** Semantic versioning
- **RFC Process:** Propose changes via GitHub issues
- **Reference Implementations:**
  - Ferris Aegis (Rust, high-assurance)
  - Claude SDK (Python, pragmatic)
  - Node.js SDK (JavaScript/TypeScript)

---

## See Also

- [Ferris Aegis Implementation](../README.md)
- [MCP Specification](https://spec.modelcontextprotocol.io)
- [OCI Image Spec](https://opencontainers.org)
- [WASI Component Model](https://github.com/WebAssembly/component-model)
