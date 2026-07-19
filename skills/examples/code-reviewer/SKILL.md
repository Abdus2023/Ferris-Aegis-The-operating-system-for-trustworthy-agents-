---
spec_version: "1.0.0"
id: "skill:analysis:code-reviewer"
name: "code-reviewer"
version: "1.0.0"
description: >
  Performs comprehensive code review combining security analysis,
  quality assessment, and best-practice checking. Use when the user wants
  code reviewed, asks for security issues, requests a code audit, or wants
  to check code quality. Do NOT use for general text review or research.
author: "ferris-aegis"
license: "MIT OR Apache-2.0"
runtime: "mcp"
platforms:
  - name: "ferris-aegis"
    min_version: "0.4.0"
permissions:
  - "filesystem.read.tmp"
  - "filesystem.read.src"
  - "crypto.hash"
inputs:
  - name: "code_path"
    type: "path"
    required: true
    description: "Path to the file or directory to review"
  - name: "review_type"
    type: "enum:security,quality,full"
    required: false
    default: "full"
    description: "Type of review: security-only, quality-only, or comprehensive"
  - name: "language"
    type: "string"
    required: false
    description: "Programming language hint (auto-detected if omitted)"
  - name: "severity_threshold"
    type: "enum:info,warning,error,critical"
    required: false
    default: "warning"
    description: "Minimum severity level to report"
outputs:
  - name: "review"
    type: "markdown"
    description: "Structured review report with findings and recommendations"
  - name: "findings"
    type: "json[]"
    description: "Array of finding objects with severity, category, location, description"
  - name: "metrics"
    type: "json"
    description: "Code quality metrics (complexity, coverage estimate, etc.)"
entrypoint: "mcp://code-reviewer/execute"
timeout: 60
sandbox:
  network:
    allowed_domains: []
  filesystem:
    allowed_paths: ["/tmp/review-*", "/src/**"]
required_context: []
optional_context:
  - "web-search"
  - "security-scanner"
dependencies:
  skills: []
  tools:
    - name: "file_read"
      version: ">=1.0.0"
validation:
  tests:
    - name: "security_review_sql_injection"
      input: { "code_path": "/test/sql_injection.py", "review_type": "security" }
      expected_output: { "has_critical_findings": true, "category_includes": "injection" }
    - name: "quality_review_complexity"
      input: { "code_path": "/test/complex.rs", "review_type": "quality" }
      expected_output: { "has_complexity_metrics": true }
    - name: "full_review"
      input: { "code_path": "/test/moderate.ts", "review_type": "full" }
      expected_output: { "has_security_section": true, "has_quality_section": true }
  examples:
    - "Review the security of this authentication module"
    - "Check the code quality of src/main.rs"
    - "Do a full code review of the changes in this PR"
    - "Find potential vulnerabilities in the API handlers"
ferris_aegis:
  trust_level: "Standard"
  policies:
    - "deny-network-access"
    - "deny-filesystem-write"
    - "allow-filesystem-read-src"
  audit: true
  sandbox:
    fuel: 8000
    memory_limit: "256MB"
---

# Code Reviewer Skill

## Overview

Performs comprehensive code review combining security analysis, quality assessment, and best-practice checking.

## When to Use

- User asks for a code review
- User wants to find security vulnerabilities
- User needs a code quality assessment
- User wants to check for best practices or anti-patterns

## When NOT to Use

- Reviewing non-code documents
- Running test suites (use a test runner)
- Formatting code (use a formatter)
- Searching code across a repository (use code-search)

## Instructions

### Step 1: Identify Scope

Determine what to review:
- **Single file**: Read and analyze the specified file
- **Directory**: Scan all source files in the directory
- **Diff/PR**: If git context is available, review only changed lines

Auto-detect the language if not specified based on file extension and content.

### Step 2: Security Analysis (if review_type is "security" or "full")

Check for common vulnerability categories:

#### Injection Vulnerabilities
- SQL injection: string concatenation in queries, unsanitized input
- Command injection: `system()`, `exec()`, shell metacharacters
- XSS: unescaped output in HTML templates
- Path traversal: unsanitized file paths, `../` sequences

#### Authentication & Authorization
- Hardcoded credentials
- Weak password hashing
- Missing authentication checks
- Privilege escalation vectors

#### Data Protection
- Sensitive data in logs
- Insecure deserialization
- Missing encryption for sensitive data
- Improper key management

#### Configuration
- Debug mode enabled in production
- Default credentials
- Insecure CORS headers
- Missing security headers

**Severity Classification:**

| Severity | Criteria | Example |
|----------|---------|---------|
| Critical | Directly exploitable, data breach risk | SQL injection with user input |
| Error | Security weakness, needs immediate fix | Hardcoded API key |
| Warning | Potential issue, should be addressed | Missing input validation |
| Info | Best practice recommendation | Add rate limiting |

### Step 3: Quality Analysis (if review_type is "quality" or "full")

#### Complexity Metrics
- Cyclomatic complexity per function
- Lines of code per function (LOC)
- Nesting depth
- Parameter count

**Thresholds:**

| Metric | Good | Warning | Error |
|--------|------|---------|-------|
| Cyclomatic Complexity | ≤ 10 | 11-20 | > 20 |
| Lines per Function | ≤ 40 | 41-80 | > 80 |
| Nesting Depth | ≤ 3 | 4-5 | > 5 |
| Parameters | ≤ 4 | 5-6 | > 6 |

#### Code Smells
- Duplicated code blocks
- Dead code (unreachable, unused)
- Magic numbers / strings
- Long parameter lists
- Deep nesting
- God functions / classes

#### Best Practices
- Error handling: proper error types, no swallowed errors
- Naming: clear, consistent naming conventions
- Documentation: public APIs have doc comments
- Testing: test coverage indicators
- Type safety: no unsafe type casts, proper generics usage

### Step 4: Synthesize Report

Generate a structured markdown report:

```markdown
# Code Review: {filename}

## Summary
{1-2 paragraph overview of the code and key findings}

## Findings

### 🔴 Critical
| # | Category | Location | Description |
|---|----------|----------|-------------|
| 1 | injection | L42 | SQL query built via string concatenation |

### 🟠 Error
| # | Category | Location | Description |
|---|----------|----------|-------------|
| 2 | credentials | L15 | Hardcoded API key |

### 🟡 Warning
| # | Category | Location | Description |
|---|----------|----------|-------------|
| 3 | validation | L78 | Missing input sanitization |

### 🔵 Info
| # | Category | Location | Description |
|---|----------|----------|-------------|
| 4 | best-practice | L100 | Consider using Result type instead of unwrap() |

## Quality Metrics

| Metric | Value | Status |
|--------|-------|--------|
| Cyclomatic Complexity (avg) | 8.3 | ✅ Good |
| Lines per Function (avg) | 35 | ✅ Good |
| Nesting Depth (max) | 4 | ⚠️ Warning |
| Error Handling Coverage | 72% | ⚠️ Warning |

## Recommendations

1. {Top priority fix}
2. {Second priority}
3. {Third priority}

## Positive Observations

- {Good pattern found in the code}
- {Well-structured module}
```

### Step 5: Return Structured Findings

```json
{
  "findings": [
    {
      "severity": "critical",
      "category": "injection",
      "file": "src/db/queries.rs",
      "line": 42,
      "description": "SQL query built via string concatenation with user input",
      "recommendation": "Use parameterized queries or an ORM",
      "cwe": "CWE-89"
    }
  ],
  "metrics": {
    "avg_cyclomatic_complexity": 8.3,
    "max_nesting_depth": 4,
    "avg_loc_per_function": 35,
    "error_handling_coverage": 0.72,
    "total_files_analyzed": 5,
    "total_lines_reviewed": 1247
  }
}
```

## Quality Checklist

- [ ] All security categories checked (injection, auth, data, config)
- [ ] Severity levels correctly assigned
- [ ] False positives minimized
- [ ] Each finding has a clear recommendation
- [ ] Code quality metrics computed accurately
- [ ] Report follows the structured template
- [ ] Positive observations included (not just criticism)

## Security Considerations

- This skill runs with **no network access** — all analysis is local
- Source code is only read, never modified
- No code is executed during review
- Findings are reported but not automatically fixed

## Error Handling

- **File not found**: Report clearly, suggest checking the path
- **Unreadable file**: Skip and note in the report
- **Unsupported language**: Provide basic analysis, note limitations
- **Binary file detected**: Skip and note in the report
