---
spec_version: "1.0.0"
id: "web-search"
name: "Web Search"
version: "2.1.0"
description: "Searches the web via multiple APIs with result aggregation"
author: "Search Research Team"
license: "Apache-2.0"

runtime: "mcp"
platforms:
  - name: "claude-code"
    min_version: "1.0"
  - name: "cursor"
    min_version: "0.36"
  - name: "ferris-aegis"
    min_version: "0.3.0"

permissions:
  - network.http
  - memory.store

inputs:
  query:
    type: string
    required: true
  num_results:
    type: integer
    default: 10
    maximum: 100
  safe_search:
    type: boolean
    default: true

outputs:
  results:
    type: array
    items:
      type: object
      properties:
        title: { type: string }
        url: { type: string }
        snippet: { type: string }
        source: { type: string }

entrypoint: "search"
timeout: "30s"
sandbox: "required"
network:
  allowed_domains:
    - "*.google.com"
    - "*.bing.com"
    - "*.duckduckgo.com"
    - "api.search.io"

required_context:
  - "search_api_key"

dependencies:
  tools:
    - name: "curl"
      version: ">=7.68"
    - name: "jq"
      version: ">=1.6"

ferris_aegis:
  trust_level:
    minimum: "probationary"
  policies:
    - id: "api-rate-limit"
      rule: "max 100 searches per hour"
      effect: "alert"
  audit:
    required: true
  signature:
    algorithm: "ed25519"
    required: true

---

# Web Search Skill

## Overview

Provides unified web search across multiple backends (Google, Bing, DuckDuckGo) with result aggregation and deduplication.

## Features

- **Multi-source**: Aggregates results from multiple search engines
- **Deduplication**: Removes duplicate URLs from results
- **Filtering**: Optional safe search mode
- **Caching**: Caches frequent searches in memory

## Inputs

- `query` (string): Search query
- `num_results` (integer): Number of results (1-100)
- `safe_search` (boolean): Enable safe search filtering

## Outputs

Array of search results with title, URL, snippet, and source API.

## Example

```json
{
  "results": [
    {
      "title": "Quantum Computing Basics - Wikipedia",
      "url": "https://en.wikipedia.org/wiki/Quantum_computing",
      "snippet": "Quantum computing is the use of quantum phenomena...",
      "source": "google"
    },
    {
      "title": "Quantum Computing Guide | IBM",
      "url": "https://www.ibm.com/quantum/",
      "snippet": "IBM's quantum computing solutions...",
      "source": "bing"
    }
  ]
}
```

## Error Handling

- **API key invalid**: Fails immediately with clear error
- **Rate limit exceeded**: Falls back to cached results
- **Network timeout**: Retries with exponential backoff

## Performance

Average response time: 2-5 seconds. Results cached for 24 hours.
