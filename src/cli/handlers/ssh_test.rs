use anyhow::Result;
use crate::scan::Scanner;

pub async fn handle_ssh_test_command(bastion: Option<String>) -> Result<()> {
    println!("🔍 KafkaPilot SSH Connectivity Test");
    println!("═══════════════════════════════════════");

    let scanner = Scanner::new(bastion.clone())?;

    match &bastion {
        Some(alias) => {
            println!("Mode: Remote via bastion '{}'", alias);
            println!("\n🔍 Running SSH diagnostics...");
            scanner.run_ssh_diagnostics(alias).await;
        }
        None => {
            println!("Mode: Local/direct connections");
            println!("\n🔍 Testing direct broker connections...");
        }
    }

    println!("\n🔗 Testing individual broker connectivity:");
    println!("{}", "─".repeat(50));

    for broker in &scanner.config.brokers {
        print!("• {} ({}): ", broker.hostname, broker.datacenter);
        let accessible = scanner.test_broker_access(broker).await;
        if accessible {
            println!("✅ Connected");
        } else {
            println!("❌ Failed");
        }
    }

    println!("\n💡 Use 'RUST_LOG=debug' for detailed SSH error messages");

    Ok(())
}