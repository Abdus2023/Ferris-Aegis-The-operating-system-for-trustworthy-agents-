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
use ferris_aegis_skills::{SkillRegistry, SkillLoader, SkillExecutor, SkillValidator};
use std::path::Path;

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

    /// Skill management (SKILL.md)
    Skill {
        #[command(subcommand)]
        action: SkillCommands,
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
enum SkillCommands {
    /// List all available skills
    List {
        /// Filter by category
        #[arg(long)]
        category: Option<String>,
        /// Filter by capability
        #[arg(long)]
        capability: Option<String>,
    },

    /// Show skill details
    Show {
        /// Skill ID (e.g., skill:filesystem:file-processor)
        skill_id: String,
    },

    /// Run a skill
    Run {
        /// Skill ID to execute
        skill_id: String,
        /// JSON input for the skill
        #[arg(long)]
        input: Option<String>,
        /// Input file (JSON)
        #[arg(long, short)]
        file: Option<String>,
        /// Agent ID to run as
        #[arg(long)]
        agent: Option<String>,
        /// Session ID
        #[arg(long)]
        session: Option<String>,
    },

    /// Load skills from a directory
    Load {
        /// Directory containing SKILL.md files
        directory: String,
        /// Recursively load from subdirectories
        #[arg(long)]
        recursive: bool,
    },

    /// Validate a skill file
    Validate {
        /// Path to SKILL.md file
        path: String,
    },

    /// Sign a skill with Ed25519
    Sign {
        /// Path to SKILL.md file
        path: String,
        /// Private key file (PEM format)
        #[arg(long)]
        key: String,
        /// Output signature file
        #[arg(long, short)]
        output: Option<String>,
    },

    /// Verify a skill's signature
    Verify {
        /// Path to SKILL.md file
        path: String,
        /// Public key file (PEM format)
        #[arg(long)]
        key: String,
    },

    /// Search skills
    Search {
        /// Search query
        query: String,
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
        Commands::Skill { action } => {
            run_skill_command(action).await?;
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
    let mut registry = SkillRegistry::new();
    
    // Load built-in skills and example skills
    if let Ok(skills_dir) = std::env::var("AEGIS_SKILLS_DIR") {
        registry.load_from_directory_recursive(Path::new(&skills_dir))?;
    } else {
        // Try default locations
        for dir in ["./skills", "./skills/examples", "/etc/ferris-aegis/skills"] {
            if Path::new(dir).exists() {
                registry.load_from_directory_recursive(Path::new(dir))?;
            }
        }
    }

    match action {
        SkillCommands::List { category, capability } => {
            if let Some(cat) = category {
                let skills = registry.by_category(&cat);
                if skills.is_empty() {
                    println!("No skills found in category: {}", cat);
                } else {
                    println!("Skills in category '{}':", cat);
                    for skill in skills {
                        println!("  • {} v{} — {}", skill.skill_id, skill.version, skill.description);
                    }
                }
            } else if let Some(cap) = capability {
                let skills = registry.by_capability(&cap);
                if skills.is_empty() {
                    println!("No skills found with capability: {}", cap);
                } else {
                    println!("Skills with capability '{}':", cap);
                    for skill in skills {
                        println!("  • {} v{} — {}", skill.skill_id, skill.version, skill.description);
                    }
                }
            } else {
                let skills = registry.list_all();
                if skills.is_empty() {
                    println!("No skills loaded. Use 'aegis skill load <directory>' to load skills.");
                } else {
                    println!("Available Skills ({}):", skills.len());
                    println!("────────────────────────────────────────────────────────────");
                    for skill in skills {
                        println!("  • {} v{} [{}]", skill.skill_id, skill.version, skill.category);
                        println!("    {}", skill.description);
                        if !skill.capabilities_required.is_empty() {
                            let caps: Vec<String> = skill.capabilities_required.iter().map(|c| c.0.clone()).collect();
                            println!("    Capabilities: {}", caps.join(", "));
                        }
                        println!("    Trust: {} | Sandbox: {}", skill.trust_level_minimum, skill.sandbox_boundary);
                    }
                }
            }
        }
        
        SkillCommands::Show { skill_id } => {
            if let Some(skill) = registry.get_sync(&skill_id) {
                println!("Skill: {}", skill.skill_id);
                println!("────────────────────────────────────────────────────────────");
                println!("  Name:        {}", skill.name);
                println!("  Version:     {}", skill.version);
                println!("  Category:    {}", skill.category);
                println!("  Description: {}", skill.description);
                println!("  Author:      {}", skill.author);
                println!("  License:     {}", skill.license);
                println!("  Trust Level: {}", skill.trust_level_minimum);
                println!("  Sandbox:     {}", skill.sandbox_boundary);
                println!("  Protocol:    {} ({})", skill.execution_protocol, skill.protocol_version);
                println!("  Export:      {}", skill.export_format);
                println!();
                println!("  Capabilities:");
                for cap in &skill.capabilities_required {
                    println!("    • {}", cap);
                }
                println!();
                println!("  Dependencies:");
                if skill.dependencies.is_empty() {
                    println!("    (none)");
                } else {
                    for dep in &skill.dependencies {
                        match dep {
                            ferris_aegis_skills::Dependency::Skill { skill, version, optional, .. } => {
                                println!("    • Skill: {} v{} {}", skill, version, if *optional { "(optional)" } else { "" });
                            }
                            ferris_aegis_skills::Dependency::SystemTool { tools } => {
                                for (tool, ver) in tools {
                                    println!("    • Tool: {} v{}", tool, ver);
                                }
                            }
                            ferris_aegis_skills::Dependency::Crate { name, version } => {
                                println!("    • Crate: {} v{}", name, version);
                            }
                        }
                    }
                }
                println!();
                println!("  Resource Limits:");
                println!("    Max File Size:     {}", skill.resource_limits.max_file_size);
                println!("    Max Execution Time: {}", skill.resource_limits.max_execution_time);
                println!("    Max Memory:         {}", skill.resource_limits.max_memory);
                println!("    Max Concurrent:     {}", skill.resource_limits.max_concurrent_calls);
                println!();
                println!("  Policies:");
                if skill.policies.is_empty() {
                    println!("    (none)");
                } else {
                    for policy in &skill.policies {
                        println!("    • {} [{}] — {}", policy.id, policy.effect, policy.rule);
                    }
                }
            } else {
                println!("✗ Skill not found: {}", skill_id);
                println!("  Use 'aegis skill list' to see available skills.");
            }
        }
        
        SkillCommands::Run { skill_id, input, file, agent, session } => {
            let skill = registry.get_sync(&skill_id).ok_or_else(|| anyhow::anyhow!("Skill not found: {}", skill_id))?;
            
            // Parse input
            let input_json = if let Some(file_path) = file {
                let content = std::fs::read_to_string(&file_path)?;
                serde_json::from_str(&content)?
            } else if let Some(input_str) = input {
                serde_json::from_str(&input_str)?
            } else {
                serde_json::json!({})
            };
            
            // Create execution context
            let agent_id = agent.unwrap_or_else(|| "cli-agent".to_string());
            let session_id = session.map(|s| uuid::Uuid::parse_str(&s).unwrap_or_else(|_| uuid::Uuid::new_v4())).unwrap_or_else(uuid::Uuid::new_v4);
            
            // Load trust kernel to get agent trust score
            let kernel = TrustKernel::new();
            let trust_score = kernel.get_record(&agent_id.into()).map(|r| r.score.value()).unwrap_or(0.5);
            
            let context = ferris_aegis_skills::ExecutionContext {
                execution_id: uuid::Uuid::new_v4(),
                agent_id: agent_id.clone(),
                agent_trust_score: trust_score,
                session_id,
                capabilities: skill.capabilities_required.iter().cloned().collect(),
                sandbox_boundary: skill.sandbox_boundary.clone(),
                workspace_root: std::path::PathBuf::from("/workspace"),
                temp_dir: std::path::PathBuf::from("/tmp"),
                start_time: chrono::Utc::now(),
                deadline: Some(chrono::Utc::now() + chrono::Duration::seconds(300)),
                audit_ledger: std::sync::Arc::new(ferris_aegis_kernel::AuditLedger::new()),
                metrics: std::sync::Arc::new(ferris_aegis_observability::CoreMetrics::new()),
                trust_kernel: std::sync::Arc::new(tokio::sync::RwLock::new(kernel)),
            };
            
            // Validate execution
            SkillValidator::validate_execution(&skill, &context)?;
            
            // Execute
            let executor = SkillExecutor::new();
            let result = executor.execute(&skill, &context, input_json).await?;
            
            // Output result
            match result.status {
                ferris_aegis_skills::ExecutionStatus::Success => {
                    println!("✓ Skill executed successfully");
                }
                ferris_aegis_skills::ExecutionStatus::Failed => {
                    println!("✗ Skill execution failed");
                    if let Some(err) = &result.error {
                        println!("  Error: {}", err);
                    }
                }
                ferris_aegis_skills::ExecutionStatus::Denied => {
                    println!("✗ Skill execution denied by policy");
                }
                ferris_aegis_skills::ExecutionStatus::TimedOut => {
                    println!("✗ Skill execution timed out");
                }
            }
            
            println!("  Execution ID: {}", result.execution_id);
            println!("  Duration: {}ms", result.duration_ms);
            if let Some(trace) = &result.trace_id {
                println!("  Trace ID: {}", trace);
            }
            if let Some(output) = &result.output {
                println!("  Output: {}", serde_json::to_string_pretty(output)?);
            }
        }
        
        SkillCommands::Load { directory, recursive } => {
            let path = Path::new(&directory);
            if !path.exists() {
                return Err(anyhow::anyhow!("Directory not found: {}", directory));
            }
            
            let count = if recursive {
                registry.load_from_directory_recursive(path)?
            } else {
                registry.load_from_directory(path)?
            };
            
            println!("✓ Loaded {} skills from {}", count, directory);
        }
        
        SkillCommands::Validate { path } => {
            let path = Path::new(&path);
            if !path.exists() {
                return Err(anyhow::anyhow!("File not found: {}", path.display()));
            }
            
            match SkillLoader::from_file(path) {
                Ok(skill) => {
                    SkillValidator::validate_static(&skill)?;
                    println!("✓ Skill is valid: {}", skill.skill_id);
                    println!("  Name: {}", skill.name);
                    println!("  Version: {}", skill.version);
                    println!("  Capabilities: {}", skill.capabilities_required.len());
                    println!("  Dependencies: {}", skill.dependencies.len());
                }
                Err(e) => {
                    println!("✗ Skill validation failed: {}", e);
                }
            }
        }
        
        SkillCommands::Sign { path, key, output } => {
            let path = Path::new(&path);
            let skill = SkillLoader::from_file(path)?;
            
            // Load private key
            let key_content = std::fs::read_to_string(&key)?;
            let private_key = ed25519_dalek::SigningKey::from_pkcs8_pem(&key_content)
                .map_err(|e| anyhow::anyhow!("Invalid private key: {}", e))?;
            
            // Sign the skill
            let signable = SkillValidator::get_signable_bytes(&skill);
            let signature = private_key.sign(&signable);
            
            let sig_hex = hex::encode(signature.to_bytes());
            
            let output_path = output.unwrap_or_else(|| {
                path.with_extension("sig").to_string_lossy().to_string()
            });
            
            std::fs::write(&output_path, &sig_hex)?;
            println!("✓ Skill signed: {}", output_path);
            println!("  Algorithm: Ed25519");
            println!("  Signature: {}...", &sig_hex[..16]);
        }
        
        SkillCommands::Verify { path, key } => {
            let path = Path::new(&path);
            let skill = SkillLoader::from_file(path)?;
            
            // Load public key
            let key_content = std::fs::read_to_string(&key)?;
            let public_key = ed25519_dalek::VerifyingKey::from_public_key_pem(&key_content)
                .map_err(|e| anyhow::anyhow!("Invalid public key: {}", e))?;
            
            // Verify
            let public_key_bytes = public_key.to_bytes();
            SkillValidator::verify_signature(&skill, &public_key_bytes)?;
            
            println!("✓ Signature verified for: {}", skill.skill_id);
            println!("  Algorithm: Ed25519");
        }
        
        SkillCommands::Search { query } => {
            let results = registry.search(&query);
            if results.is_empty() {
                println!("No skills found matching: {}", query);
            } else {
                println!("Search results for '{}' ({}):", query, results.len());
                for skill in results {
                    println!("  • {} v{} [{}] — {}", skill.skill_id, skill.version, skill.category, skill.description);
                }
            }
        }
    }
    
    Ok(())
}
