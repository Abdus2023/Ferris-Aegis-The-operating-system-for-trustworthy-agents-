---
spec_version: "1.0.0"
id: "skill:search:web-search"
name: "web-search"
version: "1.0.0"
description: >
  Performs multi-source web search aggregation with result ranking,
  deduplication, and credibility scoring. Use when the user needs to search
  the web, find information online, look up a topic, or gather sources.
  Do NOT use for code-specific searches (use code-search instead).
author: "ferris-aegis"
license: "MIT OR Apache-2.0"
runtime: "mcp"
platforms:
  - name: "ferris-aegis"
    min_version: "0.4.0"
permissions:
  - "network.http.get"
inputs:
  - name: "query"
    type: "string"
    required: true
    description: "The search query string"
    validation: "^.{1,500}$"
  - name: "max_results"
    type: "integer"
    required: false
    default: 10
    description: "Maximum number of results to return"
  - name: "source_types"
    type: "string"
    required: false
    default: "all"
    description: "Comma-separated: academic,blog,news,documentation,all"
  - name: "freshness"
    type: "enum:day,week,month,year,any"
    required: false
    default: "any"
    description: "How recent the results must be"
outputs:
  - name: "results"
    type: "json[]"
    description: "Array of search result objects with title, url, snippet, source_type, credibility"
  - name: "total_found"
    type: "integer"
    description: "Total number of results available (may exceed max_results)"
entrypoint: "mcp://web-search/execute"
timeout: 30
sandbox:
  network:
    allowed_domains: ["*"]
  filesystem:
    allowed_paths: ["/tmp/search-*"]
required_context: []
optional_context:
  - "source-evaluator"
dependencies:
  skills: []
  tools:
    - name: "web_search"
      version: ">=1.0.0"
validation:
  tests:
    - name: "basic_search"
      input: { "query": "Rust programming language" }
      expected_output: { "result_count_min": 1 }
    - name: "academic_only"
      input: { "query": "transformer attention mechanism", "source_types": "academic" }
      expected_output: { "has_academic_results": true }
    - name: "fresh_results"
      input: { "query": "AI news today", "freshness": "day" }
      expected_output: { "all_results_from_today": true }
  examples:
    - "Search for recent papers on mixture of experts"
    - "Find documentation for the Tokio async runtime"
    - "Look up the latest Rust release notes"
ferris_aegis:
  trust_level: "Probationary"
  policies:
    - "deny-filesystem-write"
  audit: true
  sandbox:
    fuel: 5000
    memory_limit: "256MB"
---

# Web Search Skill

## Overview

Performs multi-source web search aggregation with result ranking, deduplication, and credibility scoring.

## When to Use

- User needs to search the web for information
- User asks to look something up online
- User needs sources for a claim or topic
- User wants to find documentation or tutorials

## When NOT to Use

- Searching code within a repository (use `code-search`)
- Simple factual questions you already know
- Academic paper searches requiring specialized databases (use `research-planner`)

## Instructions

### Step 1: Parse Search Request

Extract the search parameters:
- **Query**: The search terms
- **Max results**: How many results to return (default: 10)
- **Source types**: Filter by source category (default: all)
- **Freshness**: Time constraint on results (default: any)

### Step 2: Execute Search

Send the query to the configured search provider:

```
results = web_search(query, {
    max_results: max_results * 2,  // Over-fetch for dedup
    freshness: freshness,
})
```

### Step 3: Rank & Score Results

For each result, compute a composite score:

```
composite_score = (
    relevance_score * 0.4 +
    credibility_score * 0.3 +
    recency_score * 0.2 +
    diversity_bonus * 0.1
)
```

**Credibility scoring:**

| Source Type | Base Score | Examples |
|-------------|-----------|----------|
| Academic (arxiv, .edu) | 0.9 | Papers, theses |
| Documentation (official docs) | 0.85 | RFCs, API docs |
| News (major outlets) | 0.7 | Reuters, Nature News |
| Blog (established) | 0.6 | Personal blogs, Medium |
| Forum (Stack Overflow, etc.) | 0.5 | Community answers |
| Unknown | 0.3 | Unrecognized sources |

### Step 4: Deduplicate

Remove duplicate or near-duplicate results:
- Exact URL match → keep higher-scored
- Same title + same domain → merge, keep canonical
- Same content, different URL → keep higher-credibility source

### Step 5: Filter & Format

Apply source type filters and truncate to `max_results`:

```json
{
  "results": [
    {
      "title": "...",
      "url": "https://...",
      "snippet": "...",
      "source_type": "academic",
      "credibility": 0.9,
      "composite_score": 0.85
    }
  ],
  "total_found": 42
}
```

## Quality Checklist

- [ ] Results are relevant to the query
- [ ] No duplicate results
- [ ] Credibility scores assigned correctly
- [ ] Source type filters applied when specified
- [ ] Freshness constraints respected

## Error Handling

- **No results**: Try alternative query formulations, suggest broadening
- **Rate limited**: Back off and retry with exponential delay
- **Network error**: Return cached results if available, otherwise fail gracefully

## Security Considerations

- Never expose internal API keys in results
- Sanitize URLs before returning (no XSS vectors)
- Respect robots.txt and rate limits
- All network requests logged for audit
