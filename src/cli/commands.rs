use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "kafkapilot",
    about = "Kafka cluster health diagnostics and remediation tool",
    version,
    author
)]
pub struct Cli {
    /// Enable verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,
    
    /// Output format for logs
    #[arg(long, value_enum, default_value = "text", global = true)]
    pub log_format: String,
    
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Scan a Kafka cluster for health issues
    Scan {
        /// SSH bastion alias (from ~/.ssh/config). If not provided, assumes running locally on bastion
        #[arg(short, long)]
        bastion: Option<String>,
        
        /// Output directory for the scan results
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    
    /// Analyze a previously collected snapshot
    Analyze {
        /// Path to the snapshot file
        #[arg(value_name = "SNAPSHOT")]
        snapshot: PathBuf,
        
        /// Report format
        #[arg(short, long, value_enum, default_value = "terminal")]
        report: ReportFormat,
        
        /// Output file path (required for markdown format, ignored for terminal)
        #[arg(short, long)]
        output: Option<PathBuf>,
        
        /// Enable LLM debug logging to llmdbg.txt
        #[arg(long)]
        llmdbg: bool,
        
        /// LLM request timeout in seconds (default: 300)
        #[arg(long, default_value = "300")]
        llm_timeout: u64,
    },
    
    /// Continuously monitor cluster health
    Watch {
        /// Monitoring interval in seconds
        #[arg(short, long, default_value = "60")]
        interval: u64,
        
        /// Enable alerting
        #[arg(short, long)]
        alert: bool,
    },
    
    /// Apply automated fixes for detected issues
    Fix {
        /// Path to the snapshot file
        #[arg(value_name = "SNAPSHOT")]
        snapshot: PathBuf,
        
        /// Dry run mode (show what would be fixed)
        #[arg(long)]
        dry_run: bool,
        
        /// Interactive mode
        #[arg(short, long)]
        interactive: bool,
    },
    
    /// Manage KafkaPilot configuration
    Config {
        #[command(subcommand)]
        subcommand: ConfigCommands,
    },
    
    /// Show information about KafkaPilot
    Info,
}

#[derive(Subcommand, Debug)]
pub enum ConfigCommands {
    /// Show current configuration
    Show,
    
    /// Set a configuration value
    Set {
        /// Configuration key
        key: String,
        
        /// Configuration value
        value: String,
    },
    
    /// Get a configuration value
    Get {
        /// Configuration key
        key: String,
    },
    
    /// Reset configuration to defaults
    Reset,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum ReportFormat {
    Terminal,
    Json,
    Html,
    Markdown,
    Pdf,
}