use anyhow::Result;
use crate::scan::Scanner;
use std::path::PathBuf;
use tracing::info;

pub async fn handle_scan_command(
    bastion: Option<String>,
    output: Option<PathBuf>,
    broker: Option<String>,
) -> Result<()> {
    info!("Starting Kafka cluster scan");

    // bastion is already Option<String>, pass it directly
    // None means we're running locally on the bastion
    let mut scanner = Scanner::new(bastion)?;

    // Set custom output directory if provided
    if let Some(output_path) = output {
        scanner = scanner.with_output_dir(output_path);
    }

    // Handle broker discovery based on input parameters
    match broker {
        Some(broker_address) => {
            // Single broker provided - discover cluster from it
            info!("Using broker discovery from: {}", broker_address);
            scanner = scanner.discover_brokers_from_single(&broker_address).await?;
        }
        None => {
            // No broker provided - try to discover from kafkactl
            info!("No broker provided, attempting to discover brokers from kafkactl");
            scanner = scanner.discover_brokers_from_kafkactl().await?;
        }
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