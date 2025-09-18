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
        
        /// Single broker hostname:port to discover cluster from. If not provided, uses hardcoded broker list
        #[arg(long)]
        broker: Option<String>,
    },
    
    /// Analyze previously collected scan data
    Analyze {
        /// Path to the scanned data directory
        #[arg(value_name = "SCANNED_DATA")]
        scanned_data: PathBuf,
        
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
    
    
    
    /// Display current KafkaPilot configuration
    Config,
    
    /// Show information about KafkaPilot
    Info,
    
    /// Manage analysis tasks
    Task {
        #[command(subcommand)]
        action: TaskCommand,
    },
    
    /// Test SSH connectivity to brokers
    TestSsh {
        /// SSH bastion alias (from ~/.ssh/config). If not provided, assumes running locally on bastion
        #[arg(short, long)]
        bastion: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum TaskCommand {
    /// List all available tasks
    List {
        /// Show full task details
        #[arg(long)]
        detailed: bool,
    },
    
    /// Test a specific task
    Test {
        /// Task ID to test
        task_id: String,
        
        /// Path to scanned data directory
        scanned_data: PathBuf,
        
        /// Enable debug output
        #[arg(long)]
        debug: bool,
    },
    
    /// Create a new task from a template
    New {
        /// Task ID
        id: String,
        
        /// Task name
        #[arg(long)]
        name: Option<String>,
        
        /// Output directory (default: analysis_tasks)
        #[arg(long, default_value = "analysis_tasks")]
        output_dir: PathBuf,
    },
    
    /// Show details of a specific task
    Show {
        /// Task ID
        task_id: String,
    },
}


#[derive(Debug, Clone, ValueEnum)]
pub enum ReportFormat {
    Terminal,
    Json,
    Markdown,
}