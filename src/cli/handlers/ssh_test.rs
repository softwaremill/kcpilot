use anyhow::Result;
use crate::scan::{Scanner, types::BrokerInfo};
use crate::scan::bastion::{run_ssh_diagnostics, test_broker_access};

pub async fn handle_ssh_test_command(bastion: Option<String>) -> Result<()> {
    println!("🔍 KafkaPilot SSH Connectivity Test");
    println!("═══════════════════════════════════════");

    let scanner = Scanner::new(bastion.clone())?;

    match &bastion {
        Some(alias) => {
            println!("Mode: Remote via bastion '{}'", alias);
            println!("\n🔍 Running SSH diagnostics...");
            
            // For SSH diagnostics, we need a sample broker - use a dummy one if no brokers configured
            let sample_broker = if !scanner.config.brokers.is_empty() {
                &scanner.config.brokers[0]
            } else {
                // Create a dummy broker for diagnostic purposes
                &BrokerInfo {
                    id: 0,
                    hostname: "localhost".to_string(),
                }
            };
            
            run_ssh_diagnostics(alias, sample_broker).await;
        }
        None => {
            println!("Mode: Local/direct connections");
            println!("\n🔍 Testing direct broker connections...");
        }
    }

    println!("\n🔗 Testing individual broker connectivity:");
    println!("{}", "─".repeat(50));

    for broker in &scanner.config.brokers {
        print!("• {}: ", broker.hostname);
        let accessible = test_broker_access(scanner.config.bastion_alias.as_ref(), broker).await;
        if accessible {
            println!("✅ Connected");
        } else {
            println!("❌ Failed");
        }
    }

    println!("\n💡 Use 'RUST_LOG=debug' for detailed SSH error messages");

    Ok(())
}