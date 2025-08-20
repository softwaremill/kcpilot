use anyhow::Result;
use clap::Parser;
use kafkapilot::cli::commands::{Cli, Commands};
use kafkapilot::scan::Scanner;
use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

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
            ..
        } => {
            info!("Starting Kafka cluster scan");
            
            // bastion is already Option<String>, pass it directly
            // None means we're running locally on the bastion
            let mut scanner = Scanner::new(bastion)?;
            
            // Set custom output directory if provided
            if let Some(output_path) = output {
                scanner = scanner.with_output_dir(output_path);
            }
            
            // Run the scan
            let result = scanner.scan().await?;
            
            // Log results
            info!(
                "Scan completed. Collected data from {} brokers out of {}",
                result.metadata.accessible_brokers,
                result.metadata.broker_count
            );
            
            Ok(())
        }
        
        Commands::Analyze { snapshot, report } => {
            println!("Analyze command not yet implemented");
            println!("  Snapshot: {}", snapshot.display());
            println!("  Report type: {:?}", report);
            Ok(())
        }
        
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
    }
}

fn init_logging(verbose: bool, log_format: &str) {
    let env_filter = if verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    };
    
    let fmt_layer = if log_format == "json" {
        fmt::layer()
            .json()
            .with_current_span(false)
            .with_span_list(false)
            .boxed()
    } else {
        fmt::layer()
            .with_target(false)
            .with_thread_ids(false)
            .with_thread_names(false)
            .boxed()
    };
    
    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .init();
}

fn print_info() {
    println!("KafkaPilot v{}", env!("CARGO_PKG_VERSION"));
    println!("{}", env!("CARGO_PKG_DESCRIPTION"));
    println!();
    println!("Authors: {}", env!("CARGO_PKG_AUTHORS"));
    println!("License: {}", env!("CARGO_PKG_LICENSE"));
    println!();
    println!("For more information, visit: https://github.com/yourusername/kafkapilot");
}