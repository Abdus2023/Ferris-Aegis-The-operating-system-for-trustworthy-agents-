---
name: WASM Plugin Integration
about: Integrate the existing WASM sandbox and plugin system into skill execution
title: '[WASM] Integrate WasmSandbox into SkillExecutor'
labels: enhancement, wasm, plugin, high-priority
assignees: ''
---

## Description

`crates/sandbox-wasm` and `crates/plugin` provide fuel/memory/epoch limits and Ed25519 verification, but are not yet used in the main execution path.

## Goals

- Allow skills to declare WASM execution
- Verify plugins at load time
- Execute WASM modules inside `SkillExecutor`

## Requirements

- [ ] Extend `Skill` manifest with optional `wasm` field
- [ ] Integrate `WasmSandbox` into `SkillExecutor`
- [ ] Use `PluginKeyring` for verification
- [ ] Add resource limit enforcement from skill manifest
- [ ] Tests for fuel exhaustion and signature verification

## References

- `crates/sandbox-wasm/src/lib.rs`
- `crates/plugin/src/lib.rs`
- `crates/skills/src/executor.rs`
- `CONTRIBUTING.md` → "WASM Plugin System"

## Priority

High (completes Phase 3 vision)