//! # Integration Tests for Ferris Aegis
//!
//! Full end-to-end tests that exercise the complete system.

use ferris_aegis_kernel::{
    agent::{AgentId, AgentRuntime, AgentStatus},
    audit::{AuditLedger, AuditSeverity},
    guard::{Guard, GuardAction, GuardConfig},
    kernel::{TrustKernel, TrustLevel, TrustScore},
    policy::{Effect, Policy, PolicyEngine, PolicyRule, PolicyVerdict},
    sandbox::{Capability, Sandbox, SandboxBoundary},
};

/// Helper to create a fully wired runtime for testing
fn test_runtime() -> AgentRuntime {
    let kernel = TrustKernel::new();
    let policy = PolicyEngine::with_defaults();
    AgentRuntime::new(kernel, policy)
}

// ── Trust Kernel Integration ──────────────────────────────────────

#[tokio::test]
async fn test_full_trust_lifecycle() {
    let mut kernel = TrustKernel::new();
    let agent_id = AgentId::new("lifecycle-test");

    // Register
    let record = kernel.register(&agent_id);
    assert_eq!(record.level, TrustLevel::Unverified);

    // Build trust to Standard
    for _ in 0..10 {
        kernel.reinforce(&agent_id, 0.05);
    }
    let record = kernel.get_record(&agent_id).unwrap();
    assert!(record.score.value() >= 0.5);
    assert_eq!(record.level, TrustLevel::Standard);

    // Attest
    let attestation = kernel.attest(
        &agent_id,
        "sha256:abc123".to_string(),
        vec![Capability::FileSystemRead, Capability::NetworkAccess],
        chrono::Duration::hours(24),
    );
    assert!(attestation.is_some());
    assert!(!attestation.unwrap().is_expired());

    // Degrade trust
    for _ in 0..10 {
        kernel.penalize(&agent_id, 0.05);
    }
    let record = kernel.get_record(&agent_id).unwrap();
    assert!(record.score.value() < 0.5);
}

#[test]
fn test_trust_score_boundaries() {
    assert_eq!(TrustScore::new(0.20).level(), TrustLevel::Probationary);
    assert_eq!(TrustScore::new(0.19).level(), TrustLevel::Unverified);
    assert_eq!(TrustScore::new(0.50).level(), TrustLevel::Standard);
    assert_eq!(TrustScore::new(0.49).level(), TrustLevel::Probationary);
    assert_eq!(TrustScore::new(0.75).level(), TrustLevel::Elevated);
    assert_eq!(TrustScore::new(0.95).level(), TrustLevel::Sovereign);
}

// ── Agent Runtime Integration ─────────────────────────────────────

#[tokio::test]
async fn test_agent_spawn_and_lifecycle() {
    let mut runtime = test_runtime();

    let id = runtime.spawn("test-agent", "1.0.0").await.unwrap();
    let agent = runtime.get_agent(&id).unwrap();
    assert_eq!(agent.status, AgentStatus::Running);
    assert_eq!(agent.name, "test-agent");

    // Suspend
    runtime.suspend(&id).await.unwrap();
    let agent = runtime.get_agent(&id).unwrap();
    assert_eq!(agent.status, AgentStatus::Suspended);

    // Resume
    runtime.resume(&id).await.unwrap();
    let agent = runtime.get_agent(&id).unwrap();
    assert_eq!(agent.status, AgentStatus::Running);

    // Terminate
    runtime.terminate(&id).await.unwrap();
    let agent = runtime.get_agent(&id).unwrap();
    assert_eq!(agent.status, AgentStatus::Terminated);
}

#[tokio::test]
async fn test_agent_quarantine_strips_capabilities() {
    let mut runtime = test_runtime();
    let id = runtime.spawn("dangerous", "1.0.0").await.unwrap();

    // Grant some capabilities first
    let agent = runtime.get_agent_mut(&id).unwrap();
    agent.grant_capability(Capability::NetworkAccess);
    agent.grant_capability(Capability::FileSystemRead);
    assert_eq!(agent.capabilities.len(), 2);

    // Quarantine
    runtime.quarantine(&id).await.unwrap();
    let agent = runtime.get_agent(&id).unwrap();
    assert_eq!(agent.status, AgentStatus::Quarantined);
    assert!(agent.capabilities.is_empty());
}

// ── Policy Engine Integration ─────────────────────────────────────

#[test]
fn test_policy_default_safety() {
    let engine = PolicyEngine::with_defaults();

    // Workspace reads allowed
    assert!(engine.evaluate("file:read", "/workspace/data.txt").is_allowed());

    // System writes denied
    assert!(!engine.evaluate("file:write", "/etc/passwd").is_allowed());

    // Internal network denied
    assert!(!engine.evaluate("network:connect", "192.168.1.1:8080").is_allowed());

    // Code execution denied
    assert!(!engine.evaluate("exec:shell", "/bin/bash").is_allowed());
}

// ── Audit Ledger Integration ──────────────────────────────────────

#[test]
fn test_audit_ledger_full_chain() {
    let mut ledger = AuditLedger::new();
    let agent1 = AgentId::new("agent-1");
    let agent2 = AgentId::new("agent-2");

    ledger.append(
        agent1.clone(),
        "file:read".to_string(),
        "/workspace/data.txt".to_string(),
        true,
        AuditSeverity::Info,
    );
    ledger.append(
        agent1.clone(),
        "file:write".to_string(),
        "/etc/passwd".to_string(),
        false,
        AuditSeverity::Critical,
    );
    ledger.append(
        agent2.clone(),
        "network:connect".to_string(),
        "api.example.com:443".to_string(),
        true,
        AuditSeverity::Info,
    );

    assert!(ledger.verify_chain());
    assert_eq!(ledger.len(), 3);
    assert_eq!(ledger.entries_for_agent(&agent1).len(), 2);
    assert_eq!(ledger.entries_for_agent(&agent2).len(), 1);
}

#[test]
fn test_audit_ledger_tamper_evidence() {
    let mut ledger = AuditLedger::new();
    let agent = AgentId::new("agent");

    for i in 0..5 {
        ledger.append(
            agent.clone(),
            format!("action:{}", i),
            format!("target:{}", i),
            true,
            AuditSeverity::Info,
        );
    }

    assert!(ledger.verify_chain());

    // Tamper with an entry
    ledger.entries_mut()[2].action = "TAMPERED".to_string();
    assert!(!ledger.verify_chain());
}

// ── Sandbox Integration ───────────────────────────────────────────

#[test]
fn test_sandbox_trust_based_boundaries() {
    let mut sandbox = Sandbox::new();

    let unverified = AgentId::new("unverified");
    sandbox.create_boundary(unverified.clone(), TrustLevel::Unverified);
    assert!(sandbox.check_capability(&unverified, &Capability::TimerAccess).is_ok());
    assert!(sandbox.check_capability(&unverified, &Capability::NetworkAccess).is_err());

    let sovereign = AgentId::new("sovereign");
    sandbox.create_boundary(sovereign.clone(), TrustLevel::Sovereign);
    assert!(sandbox.check_capability(&sovereign, &Capability::PolicyModify).is_ok());
}

// ── Guard Integration ─────────────────────────────────────────────

#[tokio::test]
async fn test_guard_intervention_pipeline() {
    let mut runtime = test_runtime();
    let mut guard = Guard::with_config(GuardConfig {
        max_actions_per_minute: 5,
        throttle_threshold: 8,
        quarantine_threshold: 10,
        min_trust_score: 0.05,
        max_violations_per_minute: 3,
        max_idle_seconds: 3600,
    });

    let id = runtime.spawn("busy-agent", "1.0.0").await.unwrap();
    guard.register_agent(&id);

    // Normal actions
    for _ in 0..4 {
        assert!(guard.record_action(&id).is_none());
    }

    // 5th triggers alert
    assert_eq!(guard.record_action(&id), Some(GuardAction::Alert));

    // Continue until quarantine
    for _ in 0..5 {
        guard.record_action(&id);
    }
    assert_eq!(guard.record_action(&id), Some(GuardAction::Quarantine));

    runtime.quarantine(&id).await.unwrap();
    assert_eq!(runtime.get_agent(&id).unwrap().status, AgentStatus::Quarantined);
}

// ── Observability Integration ─────────────────────────────────────

#[test]
fn test_observability_metrics_registration() {
    let handle = ferris_aegis_observability::init_test().expect("init_test must succeed");
    handle.metrics.requests_total.inc();
    handle.metrics.tool_ok("file_read");
    handle.metrics.tool_error("file_read");
}

// ── MCP Tool Integration ──────────────────────────────────────────

#[test]
fn test_mcp_file_read_rejects_relative() {
    // Test the inner function directly
    let result = ferris_aegis_mcp::read_file_inner("relative/path.txt", 65536);
    assert!(result.is_err());
}

#[test]
fn test_mcp_file_read_rejects_nonexistent() {
    let result = ferris_aegis_mcp::read_file_inner("/nonexistent/file.txt", 65536);
    assert!(result.is_err());
}

#[test]
fn test_mcp_file_read_works_on_real_file() {
    let result = ferris_aegis_mcp::read_file_inner(file!(), 1024);
    assert!(result.is_ok());
    assert!(result.unwrap().contains("test_mcp_file_read_works_on_real_file"));
}
