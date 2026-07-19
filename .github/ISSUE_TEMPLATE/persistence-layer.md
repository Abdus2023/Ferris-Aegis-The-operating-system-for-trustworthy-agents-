---
name: Persistence Layer Implementation
about: Replace in-memory storage with production-grade SQLite persistence
title: '[Persistence] Implement production storage for '
labels: enhancement, persistence, production
assignees: ''
---

## Description

Multiple components currently use in-memory or stub storage.

## Affected Components

- [ ] Episodic Memory (`crates/memory`)
- [ ] Audit Ledger (`crates/kernel/src/audit.rs`)
- [ ] Semantic Memory (`crates/semantic-memory`)
- [ ] Session state (`crates/session`)

## Requirements

- [ ] Use `sqlx` (already in workspace dependencies)
- [ ] Proper migrations
- [ ] Encryption-at-rest for sensitive data (where applicable)
- [ ] Backward-compatible API
- [ ] Tests for persistence and recovery

## References

- `CONTRIBUTING.md` → "Persistence Layer"
- Workspace dependency: `sqlx = { version = "0.9", features = ["runtime-tokio", "sqlite"] }`

## Priority

High (required for production use)