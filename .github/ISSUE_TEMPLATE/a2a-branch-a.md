---
name: A2A Branch A (Standalone Server)
about: Implement or wire the standalone AgentCard discovery server
title: '[A2A] Implement Branch A standalone server'
labels: enhancement, a2a, high-priority
assignees: ''
---

## Description

Branch A of the A2A protocol (`crates/a2a/src/branch_a.rs`) implements a standalone AgentCard server at `/.well-known/agent-card.json`. It is currently not exposed via the CLI or main binary.

## Goals

- Expose `aegis a2a serve` command
- Serve `AgentCard` at the well-known path
- Support task submission and routing

## Requirements

- [ ] Wire existing `branch_a` module into the CLI
- [ ] Add new subcommand under `Commands::A2a`
- [ ] Ensure trust-gated routing via `A2aRouter`
- [ ] Add integration tests
- [ ] Update `SPECIFICATION.md` Section 20

## References

- `crates/a2a/src/branch_a.rs`
- `crates/a2a/src/lib.rs`
- `CONTRIBUTING.md` → "A2A Protocol (Open Decision)"

## Priority

High (unblocks external agent discoverability)