//! # Sentinel Agent Example
//!
//! A minimal example showing how to spawn and interact with agents
//! using the Ferris Aegis framework.

use ferris_aegis_kernel::{
    agent::AgentRuntime,
    audit::{AuditLedger, AuditSeverity},
    guard::Guard,
    kernel::TrustKernel,
    policy::PolicyEngine,
    sandbox::Sandbox,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("🦀 Ferris Aegis — Sentinel Agent Example");
    println!("══════════════════════════════════════════\n");

    // 1. Initialize the Trust Kernel
    println!("▸ Initializing Trust Kernel...");
    let mut trust_kernel = TrustKernel::new()
        .with_initial_score(0.15)
        .with_decay_factor(0.999);

    // 2. Load the Policy Engine
    println!("▸ Loading Policy Engine with default safety policy...");
    let policy_engine = PolicyEngine::with_defaults();
    println!("  Loaded {} policies", policy_engine.policy_count());

    // 3. Create the Agent Runtime
    println!("▸ Creating Agent Runtime...");
    let mut runtime = AgentRuntime::new(trust_kernel, policy_engine);

    // 4. Initialize the Guard
    println!("▸ Activating Guard...");
    let mut guard = Guard::new();

    // 5. Initialize the Sandbox
    println!("▸ Preparing Sandbox Manager...");
    let mut sandbox = Sandbox::new();

    // 6. Initialize the Audit Ledger
    println!("▸ Initializing Audit Ledger...");
    let mut ledger = AuditLedger::new();

    println!("\n✓ All systems initialized. Spawning agents...\n");

    // ── Spawn a Sentinel Agent ──────────────────────────────────
    let sentinel_id = runtime.spawn("sentinel", "1.0.0").await?;
    println!("🤖 Spawned sentinel agent: {}", sentinel_id);

    // Register with Guard
    guard.register_agent(&sentinel_id);

    // Create sandbox boundary
    sandbox.create_boundary(sentinel_id.clone(), ferris_aegis_kernel::kernel::TrustLevel::Probationary);

    // ── Build trust through positive actions ────────────────────
    println!("\n▸ Building trust through positive actions...");
    for i in 0..5 {
        let score = runtime.trust_kernel_mut().reinforce(&sentinel_id, 0.1).unwrap();
        ledger.append(
            sentinel_id.clone(),
            format!("action:good-{}", i),
            format!("target:{}", i),
            true,
            AuditSeverity::Info,
        );
        println!("  Action {}: Trust score → {:.3}", i, score.value());
    }

    // ── Check trust level ──────────────────────────────────────
    let record = runtime.trust_kernel().get_record(&sentinel_id).unwrap();
    println!("\n✓ Sentinel trust level: {} (score: {:.3})", record.level, record.score.value());

    // ── Test policy enforcement ─────────────────────────────────
    println!("\n▸ Testing policy enforcement...");

    let verdict = runtime.policy_engine().evaluate("file:read", "/workspace/data.txt");
    println!("  file:read /workspace/data.txt → {:?}", verdict.is_allowed());

    let verdict = runtime.policy_engine().evaluate("file:write", "/etc/passwd");
    println!("  file:write /etc/passwd → {:?}", verdict.is_allowed());

    let verdict = runtime.policy_engine().evaluate("exec:shell", "/bin/bash");
    println!("  exec:shell /bin/bash → {:?}", verdict.is_allowed());

    // ── Test sandbox capabilities ──────────────────────────────
    println!("\n▸ Checking sandbox capabilities...");
    let boundary = sandbox.get_boundary(&sentinel_id).unwrap();
    println!("  Capabilities: {} granted", boundary.capabilities.len());
    for cap in &boundary.capabilities {
        println!("    ✓ {}", cap);
    }

    // ── Simulate a policy violation ────────────────────────────
    println!("\n▸ Simulating a policy violation...");
    runtime.trust_kernel_mut().penalize(&sentinel_id, 0.15);
    ledger.append(
        sentinel_id.clone(),
        "file:write".to_string(),
        "/etc/passwd".to_string(),
        false,
        AuditSeverity::Critical,
    );
    let record = runtime.trust_kernel().get_record(&sentinel_id).unwrap();
    println!("  ✗ Trust score after violation: {:.3} ({})", record.score.value(), record.level);

    // Check if Guard should intervene
    if let Some(action) = guard.check_trust(&sentinel_id, runtime.trust_kernel()) {
        println!("  🛡️  Guard action: {}", action);
        if action == ferris_aegis_kernel::guard::GuardAction::Quarantine {
            runtime.quarantine(&sentinel_id).await?;
            println!("  ⚠️  Agent has been quarantined!");
        }
    }

    // ── Verify audit ledger ─────────────────────────────────────
    println!("\n▸ Audit Ledger verification...");
    println!("  Total entries: {}", ledger.len());
    println!("  Chain valid: {}", ledger.verify_chain());
    println!("  Latest hash: {}...", &ledger.latest_hash()[..16.min(ledger.latest_hash().len())]);

    // ── Final status ────────────────────────────────────────────
    println!("\n══════════════════════════════════════════");
    println!("Final Status:");
    let agent = runtime.get_agent(&sentinel_id).unwrap();
    println!("  Agent: {} ({})", agent.name, agent.id);
    println!("  Status: {}", agent.status);
    println!("  Actions: {}", agent.action_count);
    println!("  Capabilities: {}", agent.capabilities.len());

    let record = runtime.trust_kernel().get_record(&sentinel_id).unwrap();
    println!("  Trust: {:.3} ({})", record.score.value(), record.level);
    println!("  Positive: {} | Negative: {}", record.positive_interactions, record.negative_interactions);

    Ok(())
}
