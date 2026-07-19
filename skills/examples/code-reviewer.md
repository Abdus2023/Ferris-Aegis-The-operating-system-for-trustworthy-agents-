---
spec_version: "1.0.0"
id: "code-reviewer"
name: "Code Reviewer"
version: "1.0.0"
description: "Reviews code for bugs, security issues, and style violations"
author: "DevTools Team"
license: "MIT"

runtime: "mcp"
platforms:
  - name: "cursor"
    min_version: "0.36"
  - name: "vscode"
    min_version: "1.75"
  - name: "ferris-aegis"
    min_version: "0.3.0"

permissions:
  - filesystem.read
  - memory.query
  - model.invoke

inputs:
  code:
    type: string
    description: "Code snippet to review"
    required: true
  language:
    type: string
    enum: ["python", "rust", "javascript", "typescript", "go"]
    required: true
  focus:
    type: array
    items:
      enum: ["security", "performance", "readability", "best-practices"]
    default: ["security", "best-practices"]

outputs:
  issues:
    type: array
    items:
      type: object
      properties:
        severity:
          enum: ["critical", "warning", "info"]
        category:
          type: string
        line:
          type: integer
        message:
          type: string
        suggestion:
          type: string
  summary:
    type: string
  score:
    type: number
    minimum: 0
    maximum: 100

entrypoint: "review"
timeout: "60s"
sandbox: "required"

required_context:
  - "claude_model"

optional_context:
  - "linter_configs"
  - "code_style_guide"

dependencies:
  models:
    - name: "claude-3-opus"
    - name: "claude-3-sonnet"
  tools:
    - name: "pylint"
      version: ">=2.16"
    - name: "eslint"
      version: ">=8.0"

ferris_aegis:
  trust_level:
    minimum: "standard"
  policies:
    - id: "no-secrets-in-analysis"
      rule: "Never output or log detected API keys, passwords, tokens"
      effect: "deny"
  audit:
    required: true
  signature:
    algorithm: "ed25519"
    required: true

---

# Code Reviewer Skill

## Overview

Performs intelligent code review using LLM analysis combined with static analysis tools. Detects:

- **Security issues**: SQL injection, XSS, credential leaks
- **Performance problems**: N+1 queries, memory leaks, inefficient algorithms
- **Code quality**: Readability, maintainability, style violations
- **Best practices**: Error handling, testing, documentation

## Supported Languages

- Python
- Rust
- JavaScript/TypeScript
- Go

## Example Review

```
Code Review Summary
====================

Severity: ⚠️ WARNING (Score: 72/100)

Critical Issues:
1. [security] SQL injection vulnerability on line 42
   - Database query concatenates user input directly
   - Suggestion: Use parameterized queries

Warnings:
2. [performance] Inefficient loop on line 58
   - O(n²) complexity for simple operation
   - Suggestion: Use HashSet for lookup

3. [readability] Variable name unclear on line 15
   - 'x' should be 'user_count'

Info:
4. [best-practices] Consider adding type hints on line 28

---

Detailed fixes can be shown with --verbose flag.
```

## Performance

Typical review time: 10-30 seconds depending on code size.

## Limitations

- Maximum code size: 10,000 lines
- Some language-specific checks require linters to be installed
- Does not perform runtime analysis
