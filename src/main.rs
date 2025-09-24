use anyhow::Result;
use clap::Parser;
use kafkapilot::cli::commands::{Cli, Commands};
use kafkapilot::cli::handlers::{handle_scan_command, handle_analyze_command, handle_task_command, handle_ssh_test_command, handle_config_command};
use kafkapilot::cli::utils::{init_logging, print_info};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    init_logging(cli.verbose, &cli.log_format);

    match cli.command {
        Commands::Scan {
            bastion,
            output,
            broker,
        } => handle_scan_command(bastion, output, broker).await,

        Commands::Analyze { scanned_data, report, output, llmdbg, llm_timeout } => {
            handle_analyze_command(scanned_data, report, output, llmdbg, llm_timeout).await
        }

        Commands::Config => {
            handle_config_command()
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


