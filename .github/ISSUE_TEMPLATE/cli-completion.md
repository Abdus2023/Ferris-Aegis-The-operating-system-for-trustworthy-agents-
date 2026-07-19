---
name: CLI Command Completion
about: Implement or improve a currently stubbed CLI command
title: '[CLI] Implement '
labels: enhancement, cli, good-first-issue
assignees: ''
---

## Description

One or more CLI commands in `src/main.rs` are currently stub implementations.

## Affected Commands

- [ ] `aegis agent list`
- [ ] `aegis security vault-store`
- [ ] `aegis audit`
- [ ] `aegis memory record / recent`
- [ ] `aegis start` (daemon mode)

## Requirements

- [ ] Real functionality (not just print statements)
- [ ] Proper error handling
- [ ] Integration with existing kernel components
- [ ] Tests added in `tests/integration.rs` or crate tests
- [ ] Documentation updated in `SPECIFICATION.md` if behavior changes

## Acceptance Criteria

- Command works end-to-end when run via `./target/release/aegis`
- Output is human-readable and machine-parseable where appropriate
- No violation of security invariants (especially INV-004: stderr-only logging)

## References

- See `CONTRIBUTING.md` → "CLI Completeness"
- Related code: `src/main.rs` (run_* functions)

## Priority

Medium–High (immediate usability improvement)