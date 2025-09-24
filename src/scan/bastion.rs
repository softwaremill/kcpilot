use anyhow::{Context, Result};
use std::process::Command;
use tracing::{debug, info};

use crate::scan::types::BrokerInfo;

/// Execute command on bastion via SSH
pub fn run_command_on_bastion(bastion_alias: Option<&String>, command: &str) -> Result<String> {
    if let Some(bastion_alias) = bastion_alias {
        let output = Command::new("ssh")
            .arg(bastion_alias)
            .arg(command)
            .output()
            .context(format!("Failed to execute command on bastion: {}", command))?;
            
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow::anyhow!("Command failed on bastion: {}", stderr))
        }
    } else {
        Err(anyhow::anyhow!("No bastion configured for SSH command execution"))
    }
}

/// Check if SSH agent has keys loaded (only needed for remote bastion)
pub fn check_ssh_agent(bastion_alias: Option<&String>) -> Result<()> {
    // Only check SSH agent if we're connecting to a remote bastion
    if bastion_alias.is_some() {
        let output = Command::new("ssh-add")
            .arg("-l")
            .output()
            .context("Failed to check SSH agent")?;
        
        if !output.status.success() || output.stdout.is_empty() {
            return Err(anyhow::anyhow!(
                "No SSH keys in agent. Run: ssh-add ~/.ssh/your-key"
            ));
        }
        
        info!("SSH agent has keys loaded");
    }
    Ok(())
}

/// Run SSH diagnostics to help debug connection issues
pub async fn run_ssh_diagnostics(bastion_alias: &str, sample_broker: &BrokerInfo) {
    // Test bastion connectivity
    print!("  • Bastion connectivity... ");
    let bastion_test = Command::new("ssh")
        .arg("-o")
        .arg("ConnectTimeout=5")
        .arg("-o")
        .arg("StrictHostKeyChecking=no")
        .arg(bastion_alias)
        .arg("echo 'bastion-ok'")
        .output();
        
    match bastion_test {
        Ok(result) if result.status.success() => {
            println!("✅ Connected");
            let output = String::from_utf8_lossy(&result.stdout);
            if !output.contains("bastion-ok") {
                println!("    ⚠️ Unexpected output: {}", output.trim());
            }
        }
        Ok(result) => {
            println!("❌ Failed (exit code: {})", result.status.code().unwrap_or(-1));
            let stderr = String::from_utf8_lossy(&result.stderr);
            if !stderr.is_empty() {
                println!("    Error: {}", stderr.trim());
            }
        }
        Err(e) => {
            println!("❌ Error executing SSH: {}", e);
            return;
        }
    }
    
    // Test SSH agent forwarding
    print!("  • SSH agent forwarding... ");
    let agent_test = Command::new("ssh")
        .arg("-A")
        .arg("-o")
        .arg("ConnectTimeout=5")
        .arg("-o")
        .arg("StrictHostKeyChecking=no")
        .arg(bastion_alias)
        .arg("ssh-add -l")
        .output();
        
    match agent_test {
        Ok(result) if result.status.success() => {
            println!("✅ Working");
            let output = String::from_utf8_lossy(&result.stdout);
            let key_count = output.lines().count();
            println!("    SSH keys available: {}", key_count);
        }
        Ok(result) => {
            println!("❌ Failed");
            let stderr = String::from_utf8_lossy(&result.stderr);
            if stderr.contains("agent has no identities") {
                println!("    Issue: No SSH keys in agent");
                println!("    Solution: Run 'ssh-add ~/.ssh/your-key'");
            } else if !stderr.is_empty() {
                println!("    Error: {}", stderr.trim());
            }
        }
        Err(e) => {
            println!("❌ Error: {}", e);
        }
    }
    
    // Test broker hostname resolution from bastion
    print!("  • Sample broker hostname resolution... ");
    let resolve_test = Command::new("ssh")
        .arg("-o")
        .arg("ConnectTimeout=5")
        .arg("-o")
        .arg("StrictHostKeyChecking=no")
        .arg(bastion_alias)
        .arg(format!("host {} || getent hosts {} || echo 'resolution-failed'", 
            sample_broker.hostname, sample_broker.hostname))
        .output();
        
    match resolve_test {
        Ok(result) if result.status.success() => {
            let output = String::from_utf8_lossy(&result.stdout);
            if output.contains("resolution-failed") {
                println!("❌ Cannot resolve {}", sample_broker.hostname);
                println!("    This suggests DNS or hostname configuration issues");
            } else {
                println!("✅ {} resolves", sample_broker.hostname);
            }
        }
        Ok(_) => {
            println!("⚠️ Cannot test hostname resolution");
        }
        Err(e) => {
            println!("❌ Error: {}", e);
        }
    }
}

/// Test if a broker is accessible via SSH
pub async fn test_broker_access(bastion_alias: Option<&String>, broker: &BrokerInfo) -> bool {
    let result = match bastion_alias {
        Some(alias) => {
            // Remote bastion: test via SSH chain
            let ssh_command = format!(
                "ssh -o ConnectTimeout=3 -o StrictHostKeyChecking=no {} 'true'",
                broker.hostname
            );
            
            debug!("Testing SSH chain: ssh -A {} {}", alias, ssh_command);
            
            let output = Command::new("ssh")
                .arg("-A")
                .arg("-o")
                .arg("ConnectTimeout=10")
                .arg("-o") 
                .arg("StrictHostKeyChecking=no")
                .arg(alias)
                .arg(ssh_command)
                .output();
            
            match output {
                Ok(result) => {
                    if !result.status.success() {
                        let stderr = String::from_utf8_lossy(&result.stderr);
                        debug!("SSH chain failed for {}: {}", broker.hostname, stderr);
                    }
                    result.status.success()
                },
                Err(e) => {
                    debug!("SSH chain error for {}: {}", broker.hostname, e);
                    false
                }
            }
        }
        None => {
            // Local bastion: test direct SSH to broker
            debug!("Testing direct SSH to {}", broker.hostname);
            
            let output = Command::new("ssh")
                .arg("-o")
                .arg("ConnectTimeout=10")
                .arg("-o")
                .arg("StrictHostKeyChecking=no")
                .arg("-o")
                .arg("BatchMode=yes")
                .arg(&broker.hostname)
                .arg("true")
                .output();
            
            match output {
                Ok(result) => {
                    if !result.status.success() {
                        let stderr = String::from_utf8_lossy(&result.stderr);
                        debug!("Direct SSH failed for {}: {}", broker.hostname, stderr);
                    }
                    result.status.success()
                },
                Err(e) => {
                    debug!("Direct SSH error for {}: {}", broker.hostname, e);
                    false
                }
            }
        }
    };
    
    if result {
        debug!("✓ SSH access confirmed for {}", broker.hostname);
    } else {
        debug!("✗ SSH access failed for {}", broker.hostname);
    }
    
    result
}

/// Check if kafkactl is available on the bastion
pub fn check_kafkactl_availability(bastion_alias: Option<&String>) -> bool {
    match run_command_on_bastion(bastion_alias, "which kafkactl") {
        Ok(output) => !output.trim().is_empty(),
        Err(_) => false,
    }
}

/// Discover brokers using simple connection test on the bastion
pub async fn discover_brokers_with_bastion_admin_client(bastion_alias: Option<&String>, broker_address: &str) -> Result<Vec<BrokerInfo>> {
    info!("Attempting broker discovery using simple connection test on bastion");
    
    // Method 1: Try a simple netcat/telnet test to verify connectivity
    let connectivity_test = format!(
        "timeout 3 bash -c 'echo > /dev/tcp/{}/{}' 2>/dev/null && echo 'CONNECTED'",
        broker_address.split(':').next().unwrap_or("unknown"),
        broker_address.split(':').nth(1).unwrap_or("9092")
    );
    
    match run_command_on_bastion(bastion_alias, &connectivity_test) {
        Ok(output) => {
            if output.contains("CONNECTED") {
                info!("Successfully verified connectivity to broker via TCP test");
                let hostname = broker_address.split(':').next().unwrap_or("unknown").to_string();
                return Ok(vec![BrokerInfo {
                    id: 0, // Will be determined during data collection
                    hostname,
                }]);
            }
        }
        Err(_) => {
            debug!("TCP connectivity test failed");
        }
    }
    
    // Method 2: Try to resolve the hostname 
    let hostname_test = format!(
        "nslookup {} >/dev/null 2>&1 && echo 'RESOLVABLE'",
        broker_address.split(':').next().unwrap_or("unknown")
    );
    
    match run_command_on_bastion(bastion_alias, &hostname_test) {
        Ok(output) => {
            if output.contains("RESOLVABLE") {
                info!("Hostname resolution successful, assuming broker is available");
                let hostname = broker_address.split(':').next().unwrap_or("unknown").to_string();
                return Ok(vec![BrokerInfo {
                    id: 0, // Will be determined during data collection
                    hostname,
                }]);
            }
        }
        Err(_) => {
            debug!("Hostname resolution test failed");
        }
    }
    
    // Method 3: Basic ping test
    let ping_test = format!(
        "ping -c 1 -W 3 {} >/dev/null 2>&1 && echo 'PINGABLE'",
        broker_address.split(':').next().unwrap_or("unknown")
    );
    
    match run_command_on_bastion(bastion_alias, &ping_test) {
        Ok(output) => {
            if output.contains("PINGABLE") {
                info!("Ping test successful, assuming broker is available");
                let hostname = broker_address.split(':').next().unwrap_or("unknown").to_string();
                return Ok(vec![BrokerInfo {
                    id: 0, // Will be determined during data collection
                    hostname,
                }]);
            }
        }
        Err(_) => {
            debug!("Ping test failed");
        }
    }
    
    Ok(Vec::new())
}