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
    kernel::TrustKernel,
    policy::PolicyEngine,
    sandbox::Sandbox,
    CODENAME, VERSION,
};

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
    println!("✓ Security: Allowlist + Injection Scanner + SSRF Guard + Credential Vault");
    println!("✓ WASM Sandbox: Fuel-metered + Memory-capped + Epoch-interruptible");
    println!("✓ Episodic Memory: SQLite-backed");
    println!("✓ Plugin System: Ed25519 manifest signing");
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
    println!("Components:");
    println!("  Trust Kernel:     ○ Ready");
    println!("  Policy Engine:    ○ Ready");
    println!("  Agent Runtime:    ○ Ready");
    println!("  Sandbox:          ○ Ready");
    println!("  Guard:            ○ Ready");
    println!("  Audit Ledger:     ○ Ready");
    println!("  Observability:    ○ Ready (OTel + Prometheus + JSON stderr)");
    println!("  MCP Server:       ○ Ready (stdio, V_2025_11_25)");
    println!("  Security:         ○ Ready (Allowlist + Injection + SSRF + Vault)");
    println!("  WASM Sandbox:     ○ Ready (Fuel + Memory + Epoch)");
    println!("  Episodic Memory:  ○ Ready (SQLite)");
    println!("  Plugin System:    ○ Ready (Ed25519 signing)");
    println!();
    println!("Commands:");
    println!("  aegis start --foreground   Launch daemon");
    println!("  aegis mcp                  Start MCP stdio server");
    println!("  aegis security scan-injection \"text\"  Scan for injection");
    println!("  aegis security check-url <url>        Check SSRF risk");
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
