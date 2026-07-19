---
spec_version: "1.0.0"
id: "skill:research:research-planner"
name: "research-planner"
version: "1.0.0"
description: >
  Orchestrates multi-step research workflows combining web search,
  source evaluation, and synthesis into structured reports. Use when the user
  needs to research a topic, compare sources, produce a research brief,
  or do a literature review. Do NOT use for simple single-query lookups.
author: "ferris-aegis"
license: "MIT OR Apache-2.0"
runtime: "mcp"
platforms:
  - name: "ferris-aegis"
    min_version: "0.4.0"
permissions:
  - "network.http.get"
  - "filesystem.read.tmp"
  - "filesystem.write.tmp"
inputs:
  - name: "topic"
    type: "string"
    required: true
    description: "The research topic or question to investigate"
  - name: "depth"
    type: "enum:shallow,medium,deep"
    required: false
    default: "medium"
    description: "Research depth: shallow (3 sources), medium (5-8), deep (10+)"
  - name: "focus"
    type: "string"
    required: false
    description: "Specific aspect to focus the research on"
outputs:
  - name: "report"
    type: "markdown"
    description: "Synthesized research report with structured sections"
  - name: "sources"
    type: "json[]"
    description: "Array of source objects with url, title, credibility_score, relevance"
entrypoint: "mcp://research-planner/execute"
timeout: 120
sandbox:
  network:
    allowed_domains: ["*.wikipedia.org", "arxiv.org", "*.github.com", "*.doi.org", "scholar.google.com"]
  filesystem:
    allowed_paths: ["/tmp/research-*"]
required_context:
  - "web-search"
  - "source-evaluator"
optional_context:
  - "code-analyzer"
  - "citation-formatter"
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
      input: { "topic": "RLHF alignment techniques", "depth": "deep" }
      expected_output: { "has_report": true, "source_count_min": 10 }
  examples:
    - "Research the latest advances in sparse attention mechanisms"
    - "Compare WebGPU vs WebAssembly for browser-based ML inference"
    - "Do a literature review on constitutional AI approaches"
ferris_aegis:
  trust_level: "Standard"
  policies:
    - "deny-network-unless-elevated"
    - "allow-filesystem-read-tmp"
  audit: true
  sandbox:
    fuel: 10000
    memory_limit: "512MB"
---

# Research Planner Skill

## Overview

Orchestrates multi-step research workflows combining web search, source evaluation, and synthesis into structured reports.

## When to Use

- User needs comprehensive research on a topic
- User wants to compare multiple sources or perspectives
- User needs a literature review or research brief
- User asks "what does the research say about X?"

## When NOT to Use

- Simple factual lookups (use `web-search` directly)
- Code-specific questions (use `code-reviewer`)
- Single-source verification

## Instructions

### Step 1: Parse Research Request

Analyze the user's request to extract:
- **Topic**: The core subject area
- **Depth**: shallow (3 sources), medium (5-8), or deep (10+)
- **Focus**: Specific aspect to emphasize (if specified)
- **Constraints**: Any time, source type, or domain restrictions

### Step 2: Generate Search Queries

Based on the depth level, generate multiple search queries:

| Depth | Queries | Sources Target |
|-------|---------|---------------|
| Shallow | 2-3 queries | 3 sources minimum |
| Medium | 4-6 queries | 5-8 sources |
| Deep | 6-10 queries | 10+ sources, include academic |

### Step 3: Execute Searches

Delegate each search query to the `web-search` skill:

```
For each query:
  1. Call skill:search:web-search with the query
  2. Collect top results (titles, URLs, snippets)
  3. Deduplicate across queries
```

### Step 4: Evaluate Sources

For each candidate source, use `source-evaluator` to assess:

- **Credibility**: Is this a reliable source? (score 0-1)
- **Relevance**: Does it directly address the topic? (score 0-1)
- **Recency**: Is it current enough for the topic? (score 0-1)
- **Diversity**: Does it add a new perspective? (boolean)

Filter to sources meeting minimum thresholds:
- Credibility ≥ 0.5
- Relevance ≥ 0.6

### Step 5: Synthesize Report

Structure the research report:

```markdown
# Research Report: {Topic}

## Summary
{2-3 paragraph executive summary}

## Key Findings
1. {Finding 1} — [Source: {citation}]
2. {Finding 2} — [Source: {citation}]
...

## Detailed Analysis
### {Subtopic 1}
{Analysis with inline citations}

### {Subtopic 2}
{Analysis with inline citations}

## Contradictions & Gaps
{Where sources disagree or evidence is lacking}

## Methodology
- Sources consulted: {N}
- Search depth: {depth}
- Date of research: {date}

## Sources
| # | Title | URL | Credibility | Relevance |
|---|-------|-----|-------------|-----------|
| 1 | ...   | ... | 0.9         | 0.8       |
```

### Step 6: Return Results

Output both the report and structured source list:

```json
{
  "sources": [
    {
      "url": "https://...",
      "title": "...",
      "credibility_score": 0.9,
      "relevance": 0.8,
      "type": "academic|blog|documentation|news"
    }
  ]
}
```

## Quality Checklist

- [ ] Minimum source count met for chosen depth
- [ ] Sources include diverse perspectives (not all from one domain)
- [ ] Every claim in the report is attributed to a source
- [ ] Contradictions between sources are acknowledged
- [ ] Report follows the structured template
- [ ] No fabricated citations — every URL was actually visited

## Error Handling

- **No results found**: Broaden search queries, try alternative phrasing
- **Low-credibility sources**: Increase query specificity, add academic sources
- **Timeout**: Return partial results with a note about incompleteness
- **Conflicting information**: Present both viewpoints in "Contradictions" section

## Edge Cases

- Highly niche topics may need deeper search even at "shallow" depth
- Time-sensitive topics should include date constraints in queries
- Multilingual research requires specifying source language preferences
