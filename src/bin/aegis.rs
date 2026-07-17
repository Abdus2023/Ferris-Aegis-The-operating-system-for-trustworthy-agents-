//! # Aegis CLI — Command-Line Interface for Ferris Aegis
//!
//! The `aegis` command provides administrative control over the
//! Ferris Aegis agent operating system.

use clap::{Parser, Subcommand};
use ferris_aegis::{
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

    /// Show agent details
    Info {
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

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| cli.log_level.clone().into()),
        )
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
        Commands::Agent { action } => {
            run_agent_command(action).await?;
        }
        Commands::Policy { action } => {
            run_policy_command(action)?;
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

    // Initialize core components
    let config = AegisConfig::default_config();
    let trust_kernel = TrustKernel::new()
        .with_initial_score(config.trust.initial_score)
        .with_decay_factor(config.trust.decay_factor);
    let policy_engine = PolicyEngine::with_defaults();
    let mut runtime = AgentRuntime::new(trust_kernel, policy_engine);
    let mut guard = Guard::new();
    let sandbox = Sandbox::new();
    let ledger = AuditLedger::new();

    println!("✓ Trust Kernel initialized");
    println!("✓ Policy Engine loaded (default safety policy)");
    println!("✓ Guard activated");
    println!("✓ Sandbox Manager ready");
    println!("✓ Audit Ledger initialized (genesis: {})", ledger.latest_hash());
    println!();

    if foreground {
        println!("Running in foreground mode. Press Ctrl+C to stop.");
        println!();
        // In a real implementation, this would start the event loop
        // For now, just show we're ready
        println!("Aegis is ready to accept agent operations.");
    } else {
        println!("Daemon mode not yet implemented. Use --foreground.");
    }

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
            println!("{:<40} {:<12} {}", "ID", "Status", "Name");
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
        AgentCommands::Info { agent_id } => {
            println!("Agent Info: {}", agent_id);
            println!("  Status: Unknown (no running daemon)");
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
            let policy = ferris_aegis::policy::Policy::default_safety();
            println!("Default Safety Policy:");
            println!("  Name: {}", policy.name);
            println!("  Version: {}", policy.version);
            println!("  Priority: {}", policy.priority);
            println!("  Default Effect: deny");
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

fn show_audit_log(last: usize) -> anyhow::Result<()> {
    println!("Audit Ledger (last {} entries):", last);
    println!("───────────────────────────────────────────────────");
    println!("(No entries — start the daemon to begin recording)");
    Ok(())
}

fn show_status() -> anyhow::Result<()> {
    println!("🦀 Ferris Aegis System Status");
    println!("═════════════════════════════");
    println!("Version:  {} ({})", VERSION, CODENAME);
    println!("Status:   Ready (no daemon running)");
    println!();
    println!("Components:");
    println!("  Trust Kernel:   ○ Ready");
    println!("  Policy Engine:  ○ Ready");
    println!("  Agent Runtime:  ○ Ready");
    println!("  Sandbox:        ○ Ready");
    println!("  Guard:          ○ Ready");
    println!("  Audit Ledger:   ○ Ready");
    println!();
    println!("Use 'aegis start --foreground' to launch the daemon.");
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

    // Create default policies directory
    let policies_dir = path.join("policies");
    std::fs::create_dir_all(&policies_dir)?;
    println!("  Policies: {}", policies_dir.display());

    // Write default safety policy
    let default_policy = r#"[policy]
name = "default-safety"
version = "1.0.0"
priority = 100
enabled = true
default_effect = "deny"

[[rules]]
action = "file:write"
effect = "deny"
targets = ["/etc/*", "/var/*", "/sys/*", "/proc/*"]
description = "Deny writes to system directories"

[[rules]]
action = "network:connect"
effect = "deny"
targets = ["10.*", "172.16.*", "192.168.*", "localhost:*"]
description = "Deny connections to internal networks"

[[rules]]
action = "file:read"
effect = "allow"
targets = ["/workspace/*"]
description = "Allow reads from workspace directory"

[[rules]]
action = "exec:*"
effect = "deny"
targets = []
description = "Deny arbitrary code execution"
"#;
    let policy_path = policies_dir.join("default-safety.toml");
    std::fs::write(&policy_path, default_policy)?;
    println!("  Default policy: {}", policy_path.display());

    Ok(())
}
