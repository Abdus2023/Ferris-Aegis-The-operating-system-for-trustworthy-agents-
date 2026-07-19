---
spec_version: "1.0.0"
id: "research-planner"
name: "Research Planner"
version: "1.2.0"
description: "Plans research tasks with web search and information synthesis"
author: "Acme Research Labs"
license: "MIT"
repository: "https://github.com/example/research-planner"

runtime: "mcp"
platforms:
  - name: "claude-code"
    min_version: "1.0"
  - name: "cursor"
    min_version: "0.36"
  - name: "github-copilot"
    min_version: "2.0"
  - name: "ferris-aegis"
    min_version: "0.3.0"

permissions:
  - network.http
  - filesystem.read
  - memory.query

inputs:
  query:
    type: string
    description: "Research topic to investigate"
    required: true
  depth:
    type: integer
    description: "Search depth (1-5, default 2)"
    default: 2
    minimum: 1
    maximum: 5
  max_results:
    type: integer
    description: "Maximum results per search"
    default: 10

outputs:
  research_plan:
    type: object
    properties:
      topic: { type: string }
      steps:
        type: array
        items: { type: string }
        description: "Ordered research steps"
      sources:
        type: array
        items: { type: string }
        description: "Recommended information sources"
      estimated_time_minutes: { type: integer }

entrypoint: "run"
timeout: "300s"
sandbox: "required"
network:
  allowed_domains:
    - "api.openai.com"
    - "wikipedia.org"
    - "scholar.google.com"
    - "arxiv.org"

required_context:
  - "web_search_tool"
  - "openai_api_key"

optional_context:
  - "memory_store"
  - "embedding_service"

dependencies:
  skills:
    - id: "web-search"
      version: ">=1.0.0"
    - id: "text-summarizer"
      version: ">=1.5.0"
      optional: true
  tools:
    - name: "curl"
      version: ">=7.68"
  models:
    - name: "claude-3-opus"

validation:
  tests:
    - name: "basic_planning"
      input: { query: "quantum computing" }
      expected_output: { steps: ["?"] }
  examples:
    - input: { query: "AI safety", depth: 3, max_results: 20 }
      description: "Deep research on AI safety with multiple sources"

ferris_aegis:
  trust_level:
    minimum: "standard"
    preferred: "elevated"
  policies:
    - id: "api-keys-secure"
      rule: "network.http calls to api.openai.com must use authenticated channels only"
      effect: "deny"
    - id: "memory-isolation"
      rule: "memory.query can only access current session"
      effect: "deny"
  audit:
    required: true
    chain_type: "sha256"
  signature:
    algorithm: "ed25519"
    required: true
  sandbox:
    capability_boundary: "standard"
    resource_limits:
      max_memory: "512MB"
      max_file_size: "50MB"
      max_concurrent: 3

---

# Research Planner Skill

## Overview

This skill automates research planning by:
1. Breaking down a research topic into focused questions
2. Identifying relevant information sources
3. Creating a structured research workflow
4. Estimating time requirements

## How It Works

### Phase 1: Topic Analysis
Understand the research query, identify key concepts, and determine depth requirements.

### Phase 2: Source Discovery
Search for authoritative sources using the web-search skill.

### Phase 3: Planning
Synthesizes research steps based on discovered sources.

## Example Usage

```bash
aegisskill run research-planner \
  --input '{"query":"advances in quantum error correction","depth":4}'
```

## Requirements

- **web-search** skill (v1.0.0+): Discovers relevant sources
- **OpenAI API Key**: For LLM-based planning (if enabled)
- **curl**: For direct HTTP requests (fallback)

## Permissions

This skill requires:
- `network.http` — Query search APIs
- `filesystem.read` — Read cached research data
- `memory.query` — Retrieve previous research sessions

## Performance

Typical execution: 30-120 seconds depending on depth.

## Error Handling

- **No sources found**: Suggests broader search terms
- **API rate limit**: Reduces search depth gracefully
- **Memory unavailable**: Operates without session context

## Audit & Logging

All searches are logged with timestamps and result counts. Search queries are not stored (privacy-preserving).
