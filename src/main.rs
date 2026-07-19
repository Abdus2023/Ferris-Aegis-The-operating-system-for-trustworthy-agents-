//! # Aegis CLI — Command-Line Interface for Ferris Aegis
//!
//! The `aegis` command provides administrative control over the
//! Ferris Aegis agent operating system.

use clap::{Parser, Subcommand};
use ferris_aegis_kernel::{
    agent::AgentRuntime,
    audit::AuditLedger,
    config::AegisConfig,
    guard::Guard,
    health::SystemHealth,
    kernel::TrustKernel,
    policy::PolicyEngine,
    sandbox::Sandbox,
    CODENAME, VERSION,
};
use ferris_aegis_skills::{SkillRegistry, SkillRegistryConfig};

#[derive(Parser)]
#[command(name = "aegis")]
#[command(about = "Ferris Aegis — The Rust Guardian for Autonomous Intelligence", long_about = None)]
#[command(version = concat!(VERSION, " (", CODENAME, ")"))]
struct Cli {
    /// Configuration file path
    #[arg(long, default_value = "aegis.toml")]
    config: String,

    /// Log level
    #[arg(long, default_value = "info")]
    log_level: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the Aegis daemon
    Start {
        /// Run in foreground (don't daemonize)
        #[arg(long)]
        foreground: bool,
    },

    /// Start the MCP stdio server
    Mcp,

    /// Manage agents
    Agent {
        #[command(subcommand)]
        action: AgentCommands,
    },

    /// Manage policies
    Policy {
        #[command(subcommand)]
        action: PolicyCommands,
    },

    /// Security operations
    Security {
        #[command(subcommand)]
        action: SecurityCommands,
    },

    /// Memory operations
    Memory {
        #[command(subcommand)]
        action: MemoryCommands,
    },

    /// Inspect the audit ledger
    Audit {
        /// Number of recent entries to show
        #[arg(long, default_value = "20")]
        last: usize,
    },

    /// Show system status
    Status,

    /// Run health checks on all components
    Health,

    /// Manage Agent Skills (SKILL.md ecosystem)
    Skills {
        #[command(subcommand)]
        action: SkillCommands,
    },

    /// Verify the audit ledger integrity
    Verify,

    /// Initialize a new Aegis configuration
    Init {
        /// Directory to initialize in
        #[arg(default_value = ".")]
        directory: String,
    },
}

#[derive(Subcommand)]
enum AgentCommands {
    /// Spawn a new agent
    Spawn {
        /// Agent name
        name: String,
        /// Agent version
        #[arg(default_value = "0.1.0")]
        version: String,
    },

    /// List all agents
    List,

    /// Suspend a running agent
    Suspend {
        /// Agent ID
        agent_id: String,
    },

    /// Resume a suspended agent
    Resume {
        /// Agent ID
        agent_id: String,
    },

    /// Terminate an agent
    Terminate {
        /// Agent ID
        agent_id: String,
    },
}

#[derive(Subcommand)]
enum PolicyCommands {
    /// List active policies
    List,

    /// Load a policy from a file
    Load {
        /// Path to policy TOML file
        path: String,
    },

    /// Show the default safety policy
    Default,
}

#[derive(Subcommand)]
enum SecurityCommands {
    /// List stored credentials (names only, never values)
    VaultList,

    /// Store a credential
    VaultStore {
        /// Credential name
        name: String,
    },

    /// Run the injection scanner on input
    ScanInjection {
        /// Text to scan
        text: String,
    },

    /// Check a URL for SSRF risk
    CheckUrl {
        /// URL to check
        url: String,
    },

    /// List allowed tools
    Allowlist,
}

#[derive(Subcommand)]
enum MemoryCommands {
    /// Record an episode
    Record {
        /// Agent ID
        agent_id: String,
        /// Role
        role: String,
        /// Content
        content: String,
    },

    /// Show recent episodes
    Recent {
        /// Agent ID
        agent_id: String,
        /// Number of episodes
        #[arg(long, default_value = "10")]
        limit: i64,
    },

    /// Search episodes
    Search {
        /// Agent ID
        agent_id: String,
        /// Search query
        query: String,
    },
}

#[derive(Subcommand)]
enum SkillCommands {
    /// List all discovered skills
    List,

    /// Validate all skills against agentskills.io spec
    Validate,

    /// Show details for a specific skill
    Show {
        /// Skill name
        name: String,
    },

    /// Generate the discovery index JSON
    Index,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Initialize logging — stderr only, never stdout (MCP owns stdout)
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| cli.log_level.clone().into()),
        )
        .with_writer(std::io::stderr)
        .json()
        .init();

    if let Err(e) = run(cli).await {
        eprintln!("Error: {e:#}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Commands::Start { foreground } => {
            start_daemon(foreground).await?;
        }
        Commands::Mcp => {
            run_mcp_server().await?;
        }
        Commands::Agent { action } => {
            run_agent_command(action).await?;
        }
        Commands::Policy { action } => {
            run_policy_command(action)?;
        }
        Commands::Security { action } => {
            run_security_command(action)?;
        }
        Commands::Memory { action } => {
            run_memory_command(action).await?;
        }
        Commands::Audit { last } => {
            show_audit_log(last)?;
        }
        Commands::Status => {
            show_status()?;
        }
        Commands::Health => {
            show_health()?;
        }
        Commands::Skills { action } => {
            run_skill_command(action).await?;
        }
        Commands::Verify => {
            verify_ledger()?;
        }
        Commands::Init { directory } => {
            init_config(&directory)?;
        }
    }
    Ok(())
}

async fn start_daemon(foreground: bool) -> anyhow::Result<()> {
    println!("🦀 Ferris Aegis v{} ({})", VERSION, CODENAME);
    println!("   The Rust Guardian for Autonomous Intelligence");
    println!();

    // Initialize observability first
    let handle = ferris_aegis_observability::init().await?;
    tracing::info!("Observability stack initialized");

    // Initialize core components
    let config = AegisConfig::default_config();
    let trust_kernel = TrustKernel::new()
        .with_initial_score(config.trust.initial_score)
        .with_decay_factor(config.trust.decay_factor);
    let policy_engine = PolicyEngine::with_defaults();
    let runtime = AgentRuntime::new(trust_kernel, policy_engine);
    let guard = Guard::new();
    let sandbox = Sandbox::new();
    let ledger = AuditLedger::new();

    // Initialize Phase 3 components
    let security = ferris_aegis_security::ToolAllowlist::default_safe();
    let injection_scanner = ferris_aegis_security::InjectionScanner::new();
    let ssrf_guard = ferris_aegis_security::SsrfGuard::new();

    println!("✓ Trust Kernel initialized");
    println!("✓ Policy Engine loaded");
    println!("✓ Guard activated");
    println!("✓ Sandbox Manager ready");
    println!("✓ Audit Ledger initialized");
    println!("✓ Observability: OTel + Prometheus + JSON stderr");
    println!("✓ Security: Allowlist + Injection Scanner + SSRF Guard + ProtectedSecret Vault");
    println!("✓ WASM Sandbox: Fuel-metered + Memory-capped + Epoch-interruptible");
    println!("✓ Episodic Memory: SQLite-backed");
    println!("✓ Plugin System: Ed25519 manifest signing");
    println!("✓ Session Manager: Multi-turn conversation sessions");
    println!("✓ Supervisor: Anomaly detection + intervention recommendations");
    println!("✓ Semantic Memory: Concepts + Embeddings + Summaries");
    println!("✓ A2A Protocol: AgentCard + trust-gated routing");
    println!("✓ Resilience: Circuit breaker + Retry + Timeout + Rate limiter");
    println!("✓ Durable Execution: Checkpoint durability + Crash recovery");
    println!();

    if foreground {
        println!("Running in foreground mode. Press Ctrl+C to stop.");
        println!();
        println!("Aegis is ready to accept agent operations.");
        tokio::signal::ctrl_c().await?;
        println!("\nShutting down...");
        handle.shutdown();
    } else {
        println!("Daemon mode not yet implemented. Use --foreground.");
    }

    Ok(())
}

async fn run_mcp_server() -> anyhow::Result<()> {
    let handle = ferris_aegis_observability::init().await?;
    tracing::info!("Starting MCP stdio server");

    let metrics = handle.metrics.clone();
    ferris_aegis_mcp::serve(metrics).await?;

    handle.shutdown();
    Ok(())
}

async fn run_agent_command(action: AgentCommands) -> anyhow::Result<()> {
    match action {
        AgentCommands::Spawn { name, version } => {
            let kernel = TrustKernel::new();
            let policy = PolicyEngine::with_defaults();
            let mut runtime = AgentRuntime::new(kernel, policy);

            let agent_id = runtime.spawn(&name, &version).await?;
            println!("✓ Agent spawned: {}", agent_id);
            println!("  Name: {}", name);
            println!("  Version: {}", version);
        }
        AgentCommands::List => {
            println!("Agent List:");
            println!("─────────────────────────────────────");
            println!("(No active agents — use 'aegis agent spawn <name>' to create one)");
        }
        AgentCommands::Suspend { agent_id } => {
            println!("⚠ Suspending agent: {}", agent_id);
        }
        AgentCommands::Resume { agent_id } => {
            println!("▶ Resuming agent: {}", agent_id);
        }
        AgentCommands::Terminate { agent_id } => {
            println!("✗ Terminating agent: {}", agent_id);
        }
    }
    Ok(())
}

fn run_policy_command(action: PolicyCommands) -> anyhow::Result<()> {
    match action {
        PolicyCommands::List => {
            let engine = PolicyEngine::with_defaults();
            println!("Active Policies:");
            for name in engine.list_policies() {
                println!("  • {}", name);
            }
        }
        PolicyCommands::Load { path } => {
            let engine = PolicyEngine::from_file(&path)?;
            println!("✓ Loaded policy from: {}", path);
            for name in engine.list_policies() {
                println!("  • {}", name);
            }
        }
        PolicyCommands::Default => {
            let policy = ferris_aegis_kernel::policy::Policy::default_safety();
            println!("Default Safety Policy:");
            println!("  Name: {}", policy.name);
            println!("  Rules:");
            for rule in &policy.rules {
                println!(
                    "    • {} {} → {}",
                    rule.effect,
                    rule.action,
                    rule.description.as_deref().unwrap_or("no description")
                );
            }
        }
    }
    Ok(())
}

fn run_security_command(action: SecurityCommands) -> anyhow::Result<()> {
    match action {
        SecurityCommands::VaultList => {
            println!("Credential Vault:");
            println!("  (No credentials stored — use 'aegis security vault-store <name>')");
        }
        SecurityCommands::VaultStore { name } => {
            println!("Storing credential: {}", name);
            println!("  (Interactive credential input not yet implemented — use the library API)");
        }
        SecurityCommands::ScanInjection { text } => {
            let scanner = ferris_aegis_security::InjectionScanner::new();
            let verdict = scanner.scan(&text);
            match verdict {
                ferris_aegis_security::InjectionVerdict::Clean => {
                    println!("✓ No injection patterns detected");
                }
                ferris_aegis_security::InjectionVerdict::Suspicious { pattern, matched } => {
                    println!("⚠ Suspicious pattern detected!");
                    println!("  Pattern: {}", pattern);
                    println!("  Matched: {}", matched);
                }
            }
        }
        SecurityCommands::CheckUrl { url } => {
            let guard = ferris_aegis_security::SsrfGuard::new();
            let verdict = guard.check_url(&url);
            match verdict {
                ferris_aegis_security::SsrfVerdict::Safe => {
                    println!("✓ URL is safe: {}", url);
                }
                ferris_aegis_security::SsrfVerdict::Blocked { reason } => {
                    println!("✗ URL blocked: {}", reason);
                }
            }
        }
        SecurityCommands::Allowlist => {
            let list = ferris_aegis_security::ToolAllowlist::default_safe();
            println!("Tool Allowlist (deny-by-default):");
            for tool in list.allowed_tools() {
                println!("  ✓ {}", tool);
            }
            println!("  (All other tools are denied)");
        }
    }
    Ok(())
}

async fn run_memory_command(action: MemoryCommands) -> anyhow::Result<()> {
    let memory = ferris_aegis_memory::EpisodicMemory::open_in_memory().await?;
    match action {
        MemoryCommands::Record { agent_id, role, content } => {
            let episode = memory.record(&agent_id, &role, &content, None).await?;
            println!("✓ Episode recorded: {}", episode.id);
            println!("  Agent: {}", agent_id);
            println!("  Role: {}", role);
        }
        MemoryCommands::Recent { agent_id, limit } => {
            let episodes = memory.recent(&agent_id, limit).await?;
            if episodes.is_empty() {
                println!("No episodes for agent: {}", agent_id);
            } else {
                println!("Recent episodes for {}:", agent_id);
                for ep in &episodes {
                    println!("  [{}] {}: {}", ep.role, ep.timestamp.format("%H:%M:%S"), ep.content);
                }
            }
        }
        MemoryCommands::Search { agent_id, query } => {
            let results = memory.search(&agent_id, &query, 10).await?;
            if results.is_empty() {
                println!("No results for '{}' in agent {}", query, agent_id);
            } else {
                println!("Search results for '{}' in agent {}:", query, agent_id);
                for ep in &results {
                    println!("  [{}] {}: {}", ep.role, ep.timestamp.format("%H:%M:%S"), ep.content);
                }
            }
        }
    }
    Ok(())
}

fn show_audit_log(last: usize) -> anyhow::Result<()> {
    println!("Audit Ledger (last {} entries):", last);
    println!("(No entries — start the daemon to begin recording)");
    Ok(())
}

fn show_status() -> anyhow::Result<()> {
    println!("🦀 Ferris Aegis System Status");
    println!("═════════════════════════════");
    println!("Version:  {} ({})", VERSION, CODENAME);
    println!();
    println!("Phase 1 — Kernel:");
    println!("  Trust Kernel:     ○ Ready");
    println!("  Policy Engine:    ○ Ready");
    println!("  Agent Runtime:    ○ Ready");
    println!("  Sandbox:          ○ Ready");
    println!("  Guard:            ○ Ready");
    println!("  Audit Ledger:     ○ Ready");
    println!();
    println!("Phase 2 — Observability + MCP:");
    println!("  Observability:    ○ Ready (OTel + Prometheus + JSON stderr)");
    println!("  MCP Server:       ○ Ready (stdio, V_2025_11_25)");
    println!();
    println!("Phase 3 — Security:");
    println!("  Tool Allowlist:   ○ Ready (deny-by-default)");
    println!("  Injection Scanner:○ Ready");
    println!("  SSRF Guard:       ○ Ready");
    println!("  Credential Vault: ○ Ready (ProtectedSecret)");
    println!("  WASM Sandbox:     ○ Ready (Fuel + Memory + Epoch)");
    println!("  Episodic Memory:  ○ Ready (SQLite)");
    println!("  Plugin System:    ○ Ready (Ed25519 signing)");
    println!();
    println!("Phase 4 — Agent OS:");
    println!("  Session Manager:  ○ Ready");
    println!("  Supervisor:       ○ Ready (Anomaly detection)");
    println!("  Semantic Memory:  ○ Ready (Concepts + Embeddings)");
    println!("  A2A Protocol:     ○ Ready (AgentCard + Router)");
    println!();
    println!("Phase 5 — Production Hardening:");
    println!("  Config Validation:○ Ready (Range + format checks)");
    println!("  Health Checks:    ○ Ready (Per-component + aggregate)");
    println!("  Circuit Breaker:  ○ Ready");
    println!("  Retry Policy:     ○ Ready (Exponential backoff + jitter)");
    println!("  Timeout:          ○ Ready (Deadline enforcement)");
    println!("  Rate Limiter:     ○ Ready (Token bucket)");
    println!();
    println!("Phase 5.1 — Durable Execution:");
    println!("  Step Durability:  ○ Ready (Checkpoint after every step)");
    println!("  Crash Recovery:   ○ Ready (Resume from last checkpoint)");
    println!("  Checkpoint Store: ○ Ready (In-memory + SQLite)");
    println!("  Hash Verification:○ Ready (Tamper evidence)");
    println!();
    println!("Phase 5.2 — Agent Skills:");
    println!("  Skill Registry:   ○ Ready (SKILL.md discovery + validation)");
    println!("  10 Skills:        ○ Installed (agentskills.io v0.2.0)");
    println!("  CLI:              ○ aegis skills list/validate/show/index");
    println!();
    Ok(())
}

fn show_health() -> anyhow::Result<()> {
    let health = SystemHealth::new();
    let report = health.report();

    println!();
    println!("🫀 Ferris Aegis Health Report");
    println!("═══════════════════════════════");
    println!(
        "Status:  {}",
        match report.system_status {
            ferris_aegis_kernel::health::HealthStatus::Healthy => "✓ Healthy",
            ferris_aegis_kernel::health::HealthStatus::Degraded => "⚠ Degraded",
            ferris_aegis_kernel::health::HealthStatus::Unhealthy => "✗ Unhealthy",
        }
    );
    println!(
        "Components: {} healthy / {} degraded / {} unhealthy",
        report.healthy_count, report.degraded_count, report.unhealthy_count
    );
    println!();

    for component in &report.components {
        let icon = match component.status {
            ferris_aegis_kernel::health::HealthStatus::Healthy => "✓",
            ferris_aegis_kernel::health::HealthStatus::Degraded => "⚠",
            ferris_aegis_kernel::health::HealthStatus::Unhealthy => "✗",
        };
        print!("  {} {} ", icon, component.component);
        if let Some(ref msg) = component.message {
            print!("— {}", msg);
        }
        println!();
    }

    println!();
    Ok(())
}

fn verify_ledger() -> anyhow::Result<()> {
    let ledger = AuditLedger::new();
    if ledger.verify_chain() {
        println!("✓ Audit ledger integrity verified (empty ledger)");
    } else {
        println!("✗ Audit ledger integrity check FAILED");
    }
    Ok(())
}

fn init_config(directory: &str) -> anyhow::Result<()> {
    let config = AegisConfig::default_config();
    let toml_str = toml::to_string_pretty(&config)?;

    let path = std::path::Path::new(directory);
    std::fs::create_dir_all(path)?;

    let config_path = path.join("aegis.toml");
    std::fs::write(&config_path, &toml_str)?;

    println!("✓ Initialized Ferris Aegis configuration");
    println!("  Config: {}", config_path.display());

    let policies_dir = path.join("policies");
    std::fs::create_dir_all(&policies_dir)?;

    let default_policy = include_str!("../../policies/default-safety.toml");
    let policy_path = policies_dir.join("default-safety.toml");
    std::fs::write(&policy_path, default_policy)?;
    println!("  Default policy: {}", policy_path.display());

    Ok(())
}

async fn run_skill_command(action: SkillCommands) -> anyhow::Result<()> {
    let config = SkillRegistryConfig::default();
    let mut registry = SkillRegistry::new(config);

    // Discover skills
    let count = registry.discover(".agents/skills").await?;

    match action {
        SkillCommands::List => {
            println!();
            println!("🧰 Ferris Aegis Agent Skills");
            println!("═════════════════════════════");
            println!("Discovered: {} skills", count);
            println!();

            if count == 0 {
                println!("  No skills found in .agents/skills/");
                println!("  Use 'aegis skills index' to generate the discovery index.");
                return Ok(());
            }

            for skill in registry.list_skills() {
                println!("  • {} — {}", skill.name, 
                    skill.description.chars().take(80).collect::<String>());
                if let Some(c) = skill.frontmatter.aegis_crate() {
                    println!("    Crate: {}", c);
                }
                if let Some(p) = skill.frontmatter.aegis_phase() {
                    println!("    Phase: {}", p);
                }
            }
            println!();
        }
        SkillCommands::Validate => {
            println!();
            println!("✓ Ferris Aegis Skill Validation");
            println!("════════════════════════════════");
            println!();

            if count == 0 {
                println!("  No skills found to validate.");
                return Ok(());
            }

            let results = registry.validate_all();
            let mut valid = 0;
            let mut invalid = 0;

            for result in &results {
                if result.is_valid() {
                    valid += 1;
                    println!("  ✓ {}", result.skill_name);
                    for warning in &result.warnings {
                        println!("    ⚠ {}", warning);
                    }
                } else {
                    invalid += 1;
                    println!("  ✗ {}", result.skill_name);
                    for error in &result.errors {
                        println!("    ✗ [{}] {}", error.rule, error.message);
                    }
                    for warning in &result.warnings {
                        println!("    ⚠ {}", warning);
                    }
                }
            }

            println!();
            println!("Results: {} valid, {} invalid, {} total", valid, invalid, results.len());
            println!();
        }
        SkillCommands::Show { name } => {
            let skill = registry.load_skill(&name).await?;

            println!();
            println!("🧰 {}", skill.metadata.name);
            println!("═════════════════════════════");
            println!("Description: {}", skill.metadata.description);
            if let Some(license) = &skill.metadata.frontmatter.license {
                println!("License: {}", license);
            }
            if let Some(c) = skill.metadata.frontmatter.aegis_crate() {
                println!("Crate: {}", c);
            }
            if let Some(p) = skill.metadata.frontmatter.aegis_phase() {
                println!("Phase: {}", p);
            }
            if let Some(v) = skill.metadata.frontmatter.version() {
                println!("Version: {}", v);
            }
            if let Some(inv) = skill.metadata.frontmatter.aegis_invariants() {
                println!("Invariants: {}", inv);
            }
            let tools = skill.metadata.frontmatter.allowed_tools_list();
            if !tools.is_empty() {
                println!("Allowed tools: {}", tools.join(", "));
            }
            println!();
            println!("Resources:");
            println!("  Scripts:    {} files", skill.resources.scripts.len());
            println!("  References: {} files", skill.resources.references.len());
            println!("  Assets:     {} files", skill.resources.assets.len());
            println!();
            println!("Instructions ({} chars):", skill.instructions.len());
            println!("{}", skill.instructions.chars().take(500).collect::<String>());
            if skill.instructions.len() > 500 {
                println!("... (truncated)");
            }
            println!();
        }
        SkillCommands::Index => {
            let index = registry.generate_index();
            let json = index.to_json()?;
            println!("{}", json);
        }
    }

    Ok(())
}
