use anyhow::Result;
use tracing::{debug, info};

use crate::scan::types::BrokerInfo;
use crate::scan::bastion::run_command_on_bastion;
use crate::collectors::admin::AdminCollector;
use crate::collectors::{KafkaConfig, Collector};

/// Discover brokers from kafkactl when no broker parameter is provided
pub async fn discover_brokers_from_kafkactl(bastion_alias: Option<&String>) -> Result<Vec<BrokerInfo>> {
    info!("Attempting to discover brokers from kafkactl");
    
    info!("✅ kafkactl is available, extracting broker list");
    
    // Get brokers from kafkactl
    match run_command_on_bastion(bastion_alias, "kafkactl get brokers -o yaml") {
        Ok(brokers_yaml) => {
            let discovered_brokers = parse_kafkactl_brokers(&brokers_yaml)?;
            
            if discovered_brokers.is_empty() {
                return Err(anyhow::anyhow!("No brokers found in kafkactl output"));
            }
            
            info!("✅ Successfully discovered {} brokers from kafkactl:", discovered_brokers.len());
            for broker in &discovered_brokers {
                info!("   • Broker {} - {}", broker.id, broker.hostname);
            }
            
            Ok(discovered_brokers)
        }
        Err(e) => {
            Err(anyhow::anyhow!("Failed to get brokers from kafkactl: {}", e))
        }
    }
}

/// Parse kafkactl broker output to extract broker information
pub fn parse_kafkactl_brokers(yaml_output: &str) -> Result<Vec<BrokerInfo>> {
    let mut brokers = Vec::new();
    let mut current_broker: Option<BrokerInfo> = None;
    let mut in_broker_section = false;
    
    // Parse brokers from kafkactl YAML output
    
    for line in yaml_output.lines() {
        let line = line.trim();
        
        // Start of a new broker entry - must be "- id:" to distinguish from config entries
        if line.starts_with("- id:") {
            // Save previous broker if exists
            if let Some(broker) = current_broker.take() {
                if broker.id != -1 && !broker.hostname.is_empty() {
                    brokers.push(broker);
                }
            }
            
            // Parse ID directly from this line
            if let Some(id_str) = line.strip_prefix("- id:") {
                if let Ok(id) = id_str.trim().parse::<i32>() {
                    current_broker = Some(BrokerInfo {
                        id,
                        hostname: String::new(),
                    });
                    in_broker_section = true;
                }
            }
            continue;
        }
        
        if in_broker_section && current_broker.is_some() {
            // Parse broker fields - ID is already handled above
            if line.starts_with("address:") {
                if let Some(addr_str) = line.strip_prefix("address:") {
                    let address = addr_str.trim().trim_matches('"');
                    // Extract hostname from hostname:port format
                    let hostname = if let Some(colon_pos) = address.find(':') {
                        &address[..colon_pos]
                    } else {
                        address
                    };
                    current_broker.as_mut().unwrap().hostname = hostname.to_string();
                }
            } else if line.starts_with("port:") || line.starts_with("rack:") {
                // Additional fields we might want to capture later
                continue;
            }
        }
    }
    
    // Add the last broker
    if let Some(broker) = current_broker {
        if broker.id != -1 && !broker.hostname.is_empty() {
            brokers.push(broker);
        }
    }
    
    // Filter out invalid brokers
    let valid_brokers: Vec<BrokerInfo> = brokers
        .into_iter()
        .filter(|b| b.id != -1 && !b.hostname.is_empty())
        .collect();
    
    Ok(valid_brokers)
}

/// Discover brokers from a single known broker using Kafka admin API (local mode)
pub async fn discover_brokers_from_single_local(broker_address: &str) -> Result<Vec<BrokerInfo>> {
    info!("Running local broker discovery");
    let admin_collector = AdminCollector::new();
    let mut kafka_config = KafkaConfig::default();
    kafka_config.bootstrap_servers = vec![broker_address.to_string()];
    kafka_config.timeout_secs = 30;
    
    // Use the collector to discover all brokers
    match admin_collector.collect(&kafka_config).await {
        Ok(admin_output) => {
            // Convert AdminBrokerInfo to our BrokerInfo
            let discovered_brokers: Vec<BrokerInfo> = admin_output
                .brokers
                .into_iter()
                .map(|b| BrokerInfo {
                    id: b.id,
                    hostname: b.host,
                })
                .collect();
            
            info!("Discovered {} brokers from cluster metadata", discovered_brokers.len());
            for broker in &discovered_brokers {
                debug!("Found broker: {} ({}:{})", broker.id, broker.hostname, "9092"); // Assume standard port for now
            }
            
            Ok(discovered_brokers)
        }
        Err(e) => {
            Err(anyhow::anyhow!("Broker discovery failed: {}", e))
        }
    }
}

/// Discover brokers using Kafka installation path method
pub async fn discover_brokers_using_installation_path(bastion_alias: Option<&String>, broker_address: &str, output_dir: &std::path::Path) -> Result<Vec<BrokerInfo>> {
    info!("🔍 Starting installation path-based broker discovery");
    
    // Step 1: SSH to the given broker and discover Kafka installation path
    let hostname = broker_address.split(':').next().unwrap_or("unknown");
    info!("Step 1: SSH to broker {} to discover Kafka installation path", hostname);
    
    let kafka_installation_path = discover_kafka_installation_path_on_broker(bastion_alias, hostname).await?;
    info!("✅ Discovered Kafka installation path: {}", kafka_installation_path);
    
    // Step 2: Use the discovered path to run kafka-broker-api-versions.sh
    info!("Step 2: Using {} to discover all brokers in cluster", kafka_installation_path);
    
    let command = format!(
        "ssh -o StrictHostKeyChecking=no {} '{}/kafka-broker-api-versions.sh --bootstrap-server localhost:9092 | awk \"/id/{{print \\$1}}\"'",
        hostname,
        kafka_installation_path
    );
    
    info!("Executing broker discovery command via bastion");
    let output = run_command_on_bastion(bastion_alias, &command)?;
    
    // Save the raw broker discovery output to tools directory
    let tools_dir = output_dir.join("cluster").join("tools");
    std::fs::create_dir_all(&tools_dir)?;
    std::fs::write(tools_dir.join("broker_discovery_kafka_tools.txt"), &output)?;
    info!("💾 Saved broker discovery output to cluster/tools/broker_discovery_kafka_tools.txt");
    
    // Step 3: Parse the output to extract broker hostnames
    let mut discovered_brokers = Vec::new();
    let mut broker_id = 0; // We'll assign sequential IDs since we don't have actual broker IDs
    
    for line in output.lines() {
        let line = line.trim();
        if !line.is_empty() && line.contains(':') {
            // Expected format: hostname:port
            let hostname = line.split(':').next().unwrap_or(line).to_string();
            
            discovered_brokers.push(BrokerInfo {
                id: broker_id,
                hostname,
            });
            broker_id += 1;
        }
    }
    
    if !discovered_brokers.is_empty() {
        info!("✅ Successfully discovered {} brokers:", discovered_brokers.len());
        for broker in &discovered_brokers {
            info!("   • Broker {} - {}", broker.id, broker.hostname);
        }
    } else {
        info!("⚠️  No brokers discovered using installation path method");
    }
    
    Ok(discovered_brokers)
}

/// Discover Kafka installation path on a specific broker
async fn discover_kafka_installation_path_on_broker(bastion_alias: Option<&String>, broker_hostname: &str) -> Result<String> {
    // Method 1: Try systemctl approach
    let systemctl_command = format!(
        "ssh -o StrictHostKeyChecking=no {} 'pid=$(ps aux | grep -E \"kafka\\.Kafka[^a-zA-Z]\" | grep -v grep | awk \"{{print \\$2}}\" | head -1); if [ ! -z \"$pid\" ]; then systemctl status $pid 2>/dev/null | grep \"ExecStart=\" | grep \"kafka-server-start.sh\" | sed \"s/.*ExecStart=//\" | sed \"s/kafka-server-start.sh.*/kafka-server-start.sh/\" | sed \"s|/bin/kafka-server-start.sh||\" | sed \"s|^[[:space:]]*||\" | head -1; fi'",
        broker_hostname
    );
    
    if let Ok(output) = run_command_on_bastion(bastion_alias, &systemctl_command) {
        let path = output.trim();
        if !path.is_empty() && path.contains("/") {
            let kafka_bin_path = format!("{}/bin", path);
            info!("Found Kafka path via systemctl: {}", kafka_bin_path);
            return Ok(kafka_bin_path);
        }
    }
    
    // Method 2: Parse ps aux output directly for installation path
    let ps_command = format!(
        "ssh -o StrictHostKeyChecking=no {} 'ps aux | grep kafka.Kafka | grep -v grep | head -1'",
        broker_hostname
    );
    
    if let Ok(output) = run_command_on_bastion(bastion_alias, &ps_command) {
        let line = output.trim();
        
        // Look for classpath argument that contains kafka installation libs
        if let Some(cp_start) = line.find("-cp ") {
            let after_cp = &line[cp_start + 4..];
            if let Some(cp_end) = after_cp.find(" kafka.Kafka") {
                let classpath = &after_cp[..cp_end];
                
                // Extract path from first jar in classpath (should be kafka libs)
                if let Some(first_jar) = classpath.split(':').next() {
                    if first_jar.contains("/libs/") && first_jar.contains("kafka") {
                        if let Some(libs_pos) = first_jar.rfind("/libs/") {
                            let kafka_base = &first_jar[..libs_pos];
                            // Clean up path - remove any trailing /bin if it exists
                            let clean_base = kafka_base.trim_end_matches("/bin");
                            let kafka_path = format!("{}/bin", clean_base);
                            info!("Found Kafka path via ps aux classpath: {}", kafka_path);
                            return Ok(kafka_path);
                        }
                    }
                }
            }
        }
        
        // Look for kafka logs directory parameter which can help identify installation
        if line.contains("-Dkafka.logs.dir=") {
            if let Some(logs_start) = line.find("-Dkafka.logs.dir=") {
                let after_logs = &line[logs_start + 17..];
                if let Some(logs_end) = after_logs.find(' ') {
                    let logs_dir = &after_logs[..logs_end];
                    // Common pattern: /opt/kafka/bin/../logs -> /opt/kafka/bin
                    if logs_dir.contains("/../logs") {
                        let kafka_path = logs_dir.replace("/../logs", "");
                        info!("Found Kafka path via logs directory: {}", kafka_path);
                        return Ok(kafka_path);
                    }
                    // Alternative: /opt/kafka/logs -> /opt/kafka/bin
                    if logs_dir.contains("/logs") {
                        let base_path = logs_dir.trim_end_matches("/logs");
                        let kafka_path = format!("{}/bin", base_path);
                        info!("Found Kafka path via logs directory (alt): {}", kafka_path);
                        return Ok(kafka_path);
                    }
                }
            }
        }
    }
    
    // Fallback to common installation paths
    let fallback_paths = vec![
        "/opt/kafka/bin",
        "/usr/local/kafka/bin",
        "/home/kafka/kafka/bin",
    ];
    
    for path in fallback_paths {
        let test_command = format!(
            "ssh -o StrictHostKeyChecking=no {} 'test -x {}/kafka-broker-api-versions.sh && echo \"EXISTS\"'",
            broker_hostname, path
        );
        
        if let Ok(output) = run_command_on_bastion(bastion_alias, &test_command) {
            if output.trim() == "EXISTS" {
                info!("Found Kafka path via fallback: {}", path);
                return Ok(path.to_string());
            }
        }
    }
    
    Err(anyhow::anyhow!("Could not discover Kafka installation path on broker {}", broker_hostname))
}

/// Discover brokers using kafka-metadata-shell.sh (most reliable method)
pub async fn discover_brokers_with_metadata_shell(bastion_alias: Option<&String>, broker_address: &str) -> Result<Vec<BrokerInfo>> {
    let command = format!(
        "kafka-metadata-shell.sh --bootstrap-server {} --print-brokers 2>/dev/null | grep -E 'broker|Broker' | head -20",
        broker_address
    );
    
    match run_command_on_bastion(bastion_alias, &command) {
        Ok(output) => {
            let mut brokers = Vec::new();
            for line in output.lines() {
                // Parse broker information from metadata shell output
                if let Some(broker) = parse_metadata_shell_broker_line(line) {
                    brokers.push(broker);
                }
            }
            Ok(brokers)
        }
        Err(_) => Ok(Vec::new()), // Tool not available
    }
}

/// Discover brokers using kafka-broker-api-versions.sh with enhanced parsing
pub async fn discover_brokers_with_api_versions(bastion_alias: Option<&String>, broker_address: &str) -> Result<Vec<BrokerInfo>> {
    let command = format!(
        "kafka-broker-api-versions.sh --bootstrap-server {} 2>/dev/null | head -10",
        broker_address
    );
    
    match run_command_on_bastion(bastion_alias, &command) {
        Ok(output) => {
            if output.contains("successfully connected") || output.contains("ApiVersion") {
                // For now, if we can connect, assume we can discover at least this broker
                // TODO: Enhanced parsing to extract multiple broker IDs from API versions output
                let hostname = broker_address.split(':').next().unwrap_or("unknown").to_string();
                Ok(vec![BrokerInfo {
                    id: 0, // Will be determined later during data collection
                    hostname,
                }])
            } else {
                Ok(Vec::new())
            }
        }
        Err(_) => Ok(Vec::new()), // Tool not available or failed
    }
}

/// Discover brokers by parsing server.properties files on known brokers
pub async fn discover_brokers_from_configs(bastion_alias: Option<&String>, _broker_address: &str) -> Result<Vec<BrokerInfo>> {
    // This would involve SSHing to the known broker, finding server.properties,
    // and looking for cluster configuration or other broker references
    // TODO: Implement config-based broker discovery
    info!("Config-based discovery method not yet implemented");
    let _ = bastion_alias; // Suppress unused variable warning
    Ok(Vec::new())
}

/// Parse a single line from kafka-metadata-shell output to extract broker info
fn parse_metadata_shell_broker_line(line: &str) -> Option<BrokerInfo> {
    // Expected format might be something like "Broker 11: kafka-host:9092"
    // This is a placeholder - actual format depends on kafka-metadata-shell output
    if line.contains("Broker") && line.contains(":") {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            if let Ok(id) = parts[1].trim_end_matches(':').parse::<i32>() {
                if let Some(address) = parts.get(2) {
                    let hostname = address.split(':').next().unwrap_or(address).to_string();
                    return Some(BrokerInfo {
                        id,
                        hostname,
                    });
                }
            }
        }
    }
    None
}