//! # Integration Tests for Ferris Aegis
//!
//! Full end-to-end tests that exercise the complete system.

use ferris_aegis::{
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
    // Test exact boundary values
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

#[tokio::test]
async fn test_cannot_suspend_terminated_agent() {
    let mut runtime = test_runtime();
    let id = runtime.spawn("test", "1.0.0").await.unwrap();
    runtime.terminate(&id).await.unwrap();

    let result = runtime.suspend(&id).await;
    assert!(result.is_err());
}

// ── Policy Engine Integration ─────────────────────────────────────

#[test]
fn test_policy_default_safety() {
    let engine = PolicyEngine::with_defaults();

    // Workspace reads allowed
    assert!(engine.evaluate("file:read", "/workspace/data.txt").is_allowed());
    assert!(engine.evaluate("file:read", "/workspace/subdir/file.rs").is_allowed());

    // System writes denied
    assert!(!engine.evaluate("file:write", "/etc/passwd").is_allowed());
    assert!(!engine.evaluate("file:write", "/var/log/syslog").is_allowed());

    // Internal network denied
    assert!(!engine.evaluate("network:connect", "192.168.1.1:8080").is_allowed());
    assert!(!engine.evaluate("network:connect", "localhost:3000").is_allowed());

    // Code execution denied
    assert!(!engine.evaluate("exec:shell", "/bin/bash").is_allowed());
    assert!(!engine.evaluate("exec:python", "/usr/bin/python3").is_allowed());
}

#[test]
fn test_policy_multiple_policies() {
    let mut engine = PolicyEngine::with_defaults();

    // Add a permissive policy at lower priority
    let mut permissive = Policy::new("permissive", "1.0")
        .with_priority(1)
        .with_default_effect(Effect::Allow);
    permissive.add_rule(PolicyRule {
        action: "file:write".to_string(),
        effect: Effect::Allow,
        targets: vec!["/workspace/output/*".to_string()],
        condition: None,
        description: Some("Allow writes to output dir".to_string()),
    });
    engine.add_policy(permissive);

    // High-priority deny should still win for /etc
    assert!(!engine.evaluate("file:write", "/etc/hosts").is_allowed());

    // Low-priority allow should work for workspace output
    assert!(engine.evaluate("file:write", "/workspace/output/result.json").is_allowed());
}

// ── Audit Ledger Integration ──────────────────────────────────────

#[test]
fn test_audit_ledger_full_chain() {
    let mut ledger = AuditLedger::new();
    let agent1 = AgentId::new("agent-1");
    let agent2 = AgentId::new("agent-2");

    // Add entries for multiple agents
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

    // Verify chain integrity
    assert!(ledger.verify_chain());
    assert_eq!(ledger.len(), 3);

    // Filter by agent
    assert_eq!(ledger.entries_for_agent(&agent1).len(), 2);
    assert_eq!(ledger.entries_for_agent(&agent2).len(), 1);

    // Filter by severity
    let critical = ledger.entries_with_severity(AuditSeverity::Critical);
    assert_eq!(critical.len(), 1);
    assert_eq!(critical[0].action, "file:write");
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

    // Unverified agent gets minimal sandbox
    let unverified = AgentId::new("unverified");
    sandbox.create_boundary(unverified.clone(), TrustLevel::Unverified);
    assert!(sandbox.check_capability(&unverified, &Capability::TimerAccess).is_ok());
    assert!(sandbox.check_capability(&unverified, &Capability::NetworkAccess).is_err());

    // Sovereign agent gets full sandbox
    let sovereign = AgentId::new("sovereign");
    sandbox.create_boundary(sovereign.clone(), TrustLevel::Sovereign);
    assert!(sandbox.check_capability(&sovereign, &Capability::PolicyModify).is_ok());
    assert!(sandbox.check_capability(&sovereign, &Capability::ProcessSpawn).is_ok());
}

#[test]
fn test_sandbox_grant_revoke() {
    let mut sandbox = Sandbox::new();
    let agent_id = AgentId::new("test");
    sandbox.create_boundary(agent_id.clone(), TrustLevel::Probationary);

    // Initially no network access
    assert!(sandbox.check_capability(&agent_id, &Capability::NetworkAccess).is_err());

    // Grant network access
    sandbox.grant_capability(&agent_id, Capability::NetworkAccess).unwrap();
    assert!(sandbox.check_capability(&agent_id, &Capability::NetworkAccess).is_ok());

    // Revoke it
    sandbox.revoke_capability(&agent_id, &Capability::NetworkAccess).unwrap();
    assert!(sandbox.check_capability(&agent_id, &Capability::NetworkAccess).is_err());
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

    // Normal actions should be fine
    for _ in 0..4 {
        let action = guard.record_action(&id);
        assert!(action.is_none());
    }

    // 5th action triggers alert
    let action = guard.record_action(&id);
    assert_eq!(action, Some(GuardAction::Alert));

    // Continue until quarantine
    for _ in 0..5 {
        guard.record_action(&id);
    }
    let action = guard.record_action(&id);
    assert_eq!(action, Some(GuardAction::Quarantine));

    // Quarantine the agent
    runtime.quarantine(&id).await.unwrap();
    let agent = runtime.get_agent(&id).unwrap();
    assert_eq!(agent.status, AgentStatus::Quarantined);
}

// ── Full System Integration ───────────────────────────────────────

#[tokio::test]
async fn test_full_system_workflow() {
    // 1. Initialize all components
    let mut kernel = TrustKernel::new();
    let policy = PolicyEngine::with_defaults();
    let mut runtime = AgentRuntime::new(kernel, policy);
    let mut guard = Guard::new();
    let mut sandbox = Sandbox::new();
    let mut ledger = AuditLedger::new();

    // 2. Spawn an agent
    let agent_id = runtime.spawn("workflow-agent", "1.0.0").await.unwrap();
    guard.register_agent(&agent_id);

    // 3. Create sandbox based on initial trust
    let trust_kernel = runtime.trust_kernel();
    let trust_level = trust_kernel
        .get_record(&agent_id)
        .map(|r| r.level)
        .unwrap_or(TrustLevel::Unverified);
    sandbox.create_boundary(agent_id.clone(), trust_level);

    // 4. Agent performs allowed actions (building trust)
    for i in 0..5 {
        // Check policy
        let policy_engine = runtime.policy_engine();
        let verdict = policy_engine.evaluate("file:read", "/workspace/data.txt");
        let allowed = verdict.is_allowed();

        // Record in audit ledger
        ledger.append(
            agent_id.clone(),
            "file:read".to_string(),
            format!("/workspace/data-{}.txt", i),
            allowed,
            if allowed { AuditSeverity::Info } else { AuditSeverity::Warning },
        );

        // Reinforce trust for good behavior
        if allowed {
            runtime.trust_kernel_mut().reinforce(&agent_id, 0.08);
        }

        // Check guard
        guard.record_action(&agent_id);
    }

    // 5. Verify trust has improved
    let record = runtime.trust_kernel().get_record(&agent_id).unwrap();
    assert!(record.score.value() > 0.1); // Should have improved from initial 0.1

    // 6. Try a denied action
    let policy_engine = runtime.policy_engine();
    let verdict = policy_engine.evaluate("file:write", "/etc/passwd");
    assert!(!verdict.is_allowed());

    // Record the denial in the audit ledger
    ledger.append(
        agent_id.clone(),
        "file:write".to_string(),
        "/etc/passwd".to_string(),
        false,
        AuditSeverity::Critical,
    );

    // Penalize trust
    runtime.trust_kernel_mut().penalize(&agent_id, 0.1);

    // Record the violation with the guard
    guard.record_violation(&agent_id);

    // 7. Verify audit chain
    assert!(ledger.verify_chain());
    assert_eq!(ledger.len(), 6); // 5 allowed + 1 denied

    // 8. Verify final state
    let agent = runtime.get_agent(&agent_id).unwrap();
    assert_eq!(agent.status, AgentStatus::Running);
}
