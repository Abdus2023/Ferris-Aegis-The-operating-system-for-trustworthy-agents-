---
name: Daemon Mode Implementation
about: Implement proper background daemon support for `aegis start`
title: '[Daemon] Implement background daemon mode'
labels: enhancement, production, daemon
assignees: ''
---

## Description

`aegis start` currently only supports `--foreground`. Production deployments require proper daemonization.

## Requirements

- [ ] Background daemon mode (`aegis start`)
- [ ] PID file support
- [ ] Graceful shutdown on SIGTERM/SIGINT
- [ ] Log file output with rotation support
- [ ] Health check endpoint or signal handling

## References

- `src/main.rs` → `start_daemon()`
- `CONTRIBUTING.md` → "Daemonization & Production Hardening"

## Priority

Medium–High (production deployment blocker)