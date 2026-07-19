# Ferris Aegis Roadmap

**Current Version**: 0.3.0  
**Status**: All 5 phases complete — entering **Hardening & Production Readiness** phase

---

## Vision

Ferris Aegis aims to become the reference implementation of a **trustworthy, auditable, and resilient operating system for autonomous AI agents** in Rust.

---

## Phase Status

| Phase | Name                              | Status     | Focus |
|-------|-----------------------------------|------------|-------|
| 1     | Core Kernel                       | ✅ Complete | Trust, Policy, Audit, Sandbox, Guard |
| 2     | Observability + MCP               | ✅ Complete | OTel, Prometheus, MCP stdio |
| 3     | Security + Memory + Plugin        | ✅ Complete | Vault, WASM, Ed25519 plugins |
| 4     | Session + Supervisor + Semantic + A2A | ✅ Complete | Budgets, anomaly detection, AgentCard |
| 5     | Resilience + Health + Config      | ✅ Complete | Circuit breaker, retry, health checks |

---

## Current Focus: Hardening & Production Readiness (v0.4.0)

### Q3–Q4 2026 Milestones

| Milestone | Target | Key Deliverables | Status |
|-----------|--------|------------------|--------|
| **v0.4.0-alpha** | Aug 2026 | CLI command completion, A2A Branch A server | In progress |
| **v0.4.0-beta** | Sep 2026 | WASM plugin integration, Persistent storage | Planned |
| **v0.4.0** | Oct 2026 | Daemon mode, Production test suite, Documentation polish | Planned |

---

## Detailed Roadmap

### 1. CLI & Usability (v0.4.0)

- [ ] Complete all stub commands (`agent list`, `vault-store`, `audit`, `memory`)
- [ ] Add `aegis a2a serve` (Branch A)
- [ ] Improve output formatting and error messages
- [ ] Add shell completions

### 2. A2A Protocol Maturation (v0.4.0)

- [ ] Decide on / support both Branch A and Branch B
- [ ] Full task lifecycle implementation
- [ ] A2A conformance tests
- [ ] Documentation + examples for external agents

### 3. Execution & Plugin System (v0.4.0)

- [ ] Integrate `WasmSandbox` into `SkillExecutor`
- [ ] Plugin manifest resolution and loading
- [ ] Resource limit enforcement from skill definitions

### 4. Persistence & Production (v0.5.0)

- [ ] SQLite-backed storage for:
  - Audit Ledger
  - Episodic Memory
  - Semantic Memory
  - Sessions
- [ ] Encryption-at-rest for credentials and audit data
- [ ] Migration system

### 5. Operations & Hardening (v0.5.0)

- [ ] Background daemon mode with PID files and signal handling
- [ ] Log rotation and structured logging improvements
- [ ] Health check HTTP endpoint (optional)
- [ ] Prometheus exporter for `aegis` binary

### 6. Testing & Quality (Ongoing)

- [ ] Increase integration test coverage (target: 80%+)
- [ ] Cross-phase end-to-end tests (Guard + A2A + Resilience)
- [ ] Fuzzing for policy engine and audit ledger
- [ ] Property-based testing for trust decay and guard thresholds

### 7. Documentation & Ecosystem (v0.4.0+)

- [ ] More usage examples and integration guides
- [ ] Video walkthroughs / architecture deep dives
- [ ] SKILL.md ecosystem examples and registry
- [ ] External contributor onboarding improvements

---

## Long-Term Vision (2027+)

- **Phase 6**: Multi-agent orchestration layer
- **Phase 7**: Federated trust and cross-organization A2A
- **Phase 8**: Formal verification of core invariants (using tools like Kani or Prusti)
- **Phase 9**: WebAssembly System Interface (WASI) support for broader plugin ecosystem

---

## How to Track Progress

- GitHub Projects board (to be created)
- Milestone tracking in GitHub
- Quarterly status updates in `docs/PHASE-DELIVERY-RECORD.md`

---

*This roadmap is a living document and will be updated as the project evolves.*