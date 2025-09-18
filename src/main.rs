use anyhow::Result;
use clap::Parser;
use kafkapilot::cli::commands::{Cli, Commands};
use kafkapilot::cli::handlers::{handle_scan_command, handle_analyze_command, handle_task_command, handle_ssh_test_command};
use kafkapilot::cli::utils::{init_logging, print_info};

#[tokio::main]
async fn main() -> Result<()> {
    // Parse CLI arguments
    let cli = Cli::parse();

    // Initialize logging
    init_logging(cli.verbose, &cli.log_format);

    // Execute command
    match cli.command {
        Commands::Scan {
            bastion,
            output,
            broker,
        } => handle_scan_command(bastion, output, broker).await,

        Commands::Analyze { snapshot, report, output, llmdbg, llm_timeout } => {
            handle_analyze_command(snapshot, report, output, llmdbg, llm_timeout).await
        },

        Commands::Watch { .. } => {
            println!("Watch command not yet implemented");
            Ok(())
        }

        Commands::Fix { .. } => {
            println!("Fix command not yet implemented");
            Ok(())
        }

        Commands::Config { subcommand } => {
            println!("Config command not yet implemented");
            println!("  Subcommand: {:?}", subcommand);
            Ok(())
        }

        Commands::Info => {
            print_info();
            Ok(())
        }

        Commands::Task { action } => {
            handle_task_command(action).await
        }

        Commands::TestSsh { bastion } => {
            handle_ssh_test_command(bastion).await
        }
    }
}


