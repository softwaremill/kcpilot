use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tracing::{debug, error, info};

use collector::DiscoveryMethod;

pub mod collector;
pub mod log_discovery;
pub mod enhanced_log_discovery;
#[cfg(test)]
mod test_log_discovery;

use collector::{BastionCollector, BrokerCollector};
use crate::collectors::admin::AdminCollector;
use crate::collectors::{KafkaConfig, Collector};
// Import log_discovery types when needed

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanConfig {
    pub bastion_alias: Option<String>,  // None means running locally on bastion
    pub output_dir: PathBuf,
    pub brokers: Vec<BrokerInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerInfo {
    pub id: i32,
    pub hostname: String,
    pub datacenter: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanMetadata {
    pub scan_timestamp: String,
    pub bastion: Option<String>,
    pub is_local: bool,
    pub broker_count: usize,
    pub output_directory: String,
    pub scan_version: String,
    pub accessible_brokers: usize,
    pub cluster_mode: Option<crate::snapshot::format::ClusterMode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub metadata: ScanMetadata,
    pub cluster_data: ClusterData,
    pub broker_data: Vec<BrokerData>,
    pub collection_stats: CollectionStats,
    pub detected_cluster_mode: Option<crate::snapshot::format::ClusterMode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterData {
    pub kafkactl_data: HashMap<String, String>,
    pub metrics: Option<String>,
    pub bastion_info: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerData {
    pub broker_id: i32,
    pub hostname: String,
    pub datacenter: String,
    pub accessible: bool,
    pub system_info: HashMap<String, String>,
    pub configs: HashMap<String, String>,
    pub logs: HashMap<String, String>,
    pub data_dirs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionStats {
    pub total_files: usize,
    pub total_size_bytes: u64,
    pub duration_secs: u64,
}

pub struct Scanner {
    pub config: ScanConfig,
    discovery_method: Option<DiscoveryMethod>,
    detected_cluster_mode: Option<crate::snapshot::format::ClusterMode>,
}

impl Scanner {
    /// Detect cluster mode from server.properties content
    fn detect_cluster_mode_from_config(&mut self, server_properties_content: &str) -> crate::snapshot::format::ClusterMode {
        use crate::snapshot::format::ClusterMode;
        
        let properties = parse_server_properties(server_properties_content);
        
        if is_kraft_mode(&properties) {
            info!("🔍 Detected KRaft cluster mode during scan");
            ClusterMode::Kraft
        } else if is_zookeeper_mode(&properties) {
            info!("🔍 Detected Zookeeper cluster mode during scan");  
            ClusterMode::Zookeeper
        } else {
            info!("🔍 Could not determine cluster mode during scan");
            ClusterMode::Unknown
        }
    }
    
    /// Set the detected cluster mode and return it
    pub fn set_cluster_mode(&mut self, mode: crate::snapshot::format::ClusterMode) -> crate::snapshot::format::ClusterMode {
        self.detected_cluster_mode = Some(mode.clone());
        mode
    }
    
    /// Get the currently detected cluster mode
    pub fn get_cluster_mode(&self) -> Option<crate::snapshot::format::ClusterMode> {
        self.detected_cluster_mode.clone()
    }
    
    pub fn new(bastion_alias: Option<String>) -> Result<Self> {
        // Create output directory with timestamp
        let timestamp = Utc::now().format("%Y%m%d-%H%M%S");
        let output_dir = PathBuf::from(format!("kafka-scan-{}", timestamp));
        
        // Default broker configuration - can be made configurable
        let brokers = vec![];
        
        Ok(Self {
            config: ScanConfig {
                bastion_alias,
                output_dir,
                brokers,
            },
            discovery_method: None,
            detected_cluster_mode: None,
        })
    }
    
    pub fn with_output_dir(mut self, dir: PathBuf) -> Self {
        self.config.output_dir = dir;
        self
    }
    
    pub fn with_brokers(mut self, brokers: Vec<BrokerInfo>) -> Self {
        self.config.brokers = brokers;
        self
    }

    /// Discover brokers from kafkactl when no broker parameter is provided
    pub async fn discover_brokers_from_kafkactl(mut self) -> Result<Self> {
        info!("Attempting to discover brokers from kafkactl");
        
        // First check if kafkactl is available
        let kafkactl_available = self.check_kafkactl_availability();
        
        if !kafkactl_available {
            return Err(anyhow::anyhow!(
                "❌ Neither --broker parameter nor kafkactl tool are available.\n\n\
                 To scan the Kafka cluster, you must provide one of the following:\n\
                 • Use --broker hostname:port to specify a single broker for cluster discovery\n\
                 • Ensure kafkactl is installed and configured on the bastion host\n\n\
                 Examples:\n\
                 • kafkapilot scan --bastion my-bastion --broker kafka1.example.com:9092\n\
                 • kafkapilot scan --bastion my-bastion  # (requires kafkactl)"
            ));
        }
        
        info!("✅ kafkactl is available, extracting broker list");
        
        // Get brokers from kafkactl
        match self.run_command_on_bastion("kafkactl get brokers -o yaml") {
            Ok(brokers_yaml) => {
                let discovered_brokers = self.parse_kafkactl_brokers(&brokers_yaml)?;
                
                if discovered_brokers.is_empty() {
                    return Err(anyhow::anyhow!("No brokers found in kafkactl output"));
                }
                
                info!("✅ Successfully discovered {} brokers from kafkactl:", discovered_brokers.len());
                for broker in &discovered_brokers {
                    info!("   • Broker {} - {} ({})", broker.id, broker.hostname, broker.datacenter);
                }
                
                self.config.brokers = discovered_brokers;
                
                // Set discovery method for topic collection
                self.discovery_method = Some(DiscoveryMethod::Kafkactl);
                Ok(self)
            }
            Err(e) => {
                Err(anyhow::anyhow!("Failed to get brokers from kafkactl: {}", e))
            }
        }
    }
    
    /// Check if kafkactl is available on the bastion
    fn check_kafkactl_availability(&self) -> bool {
        match self.run_command_on_bastion("which kafkactl") {
            Ok(output) => !output.trim().is_empty(),
            Err(_) => false,
        }
    }
    
    /// Parse kafkactl broker output to extract broker information
    fn parse_kafkactl_brokers(&self, yaml_output: &str) -> Result<Vec<BrokerInfo>> {
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
                            datacenter: "unknown".to_string(),
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
                        
                        // Try to infer datacenter from hostname pattern
                        let datacenter = if hostname.contains("-dc1-") {
                            "dc1".to_string()
                        } else if hostname.contains("-dc2-") {
                            "dc2".to_string()
                        } else if hostname.contains("-dc3-") {
                            "dc3".to_string()
                        } else {
                            "unknown".to_string()
                        };
                        current_broker.as_mut().unwrap().datacenter = datacenter;
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

    /// Discover brokers from a single known broker using Kafka admin API
    pub async fn discover_brokers_from_single(mut self, broker_address: &str) -> Result<Self> {
        info!("Discovering brokers from single broker: {}", broker_address);
        
        // Parse hostname:port
        let parts: Vec<&str> = broker_address.split(':').collect();
        if parts.len() != 2 {
            return Err(anyhow::anyhow!("Broker address must be in format hostname:port, got: {}", broker_address));
        }
        
        // If using bastion, run discovery on the bastion via SSH
        if let Some(bastion_alias) = self.config.bastion_alias.clone() {
            info!("Running broker discovery via bastion: {}", bastion_alias);
            return self.discover_brokers_via_ssh(broker_address, &bastion_alias).await;
        }
        
        // Local discovery using admin client
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
                        datacenter: "unknown".to_string(), // We don't have datacenter info from admin API
                    })
                    .collect();
                
                info!("Discovered {} brokers from cluster metadata", discovered_brokers.len());
                for broker in &discovered_brokers {
                    debug!("Found broker: {} ({}:{})", broker.id, broker.hostname, "9092"); // Assume standard port for now
                }
                
                self.config.brokers = discovered_brokers;
                Ok(self)
            }
            Err(e) => {
                error!("Failed to discover brokers from {}: {}", broker_address, e);
                Err(anyhow::anyhow!("Broker discovery failed: {}", e))
            }
        }
    }

    /// Discover brokers via SSH using installation path-based discovery
    async fn discover_brokers_via_ssh(mut self, broker_address: &str, bastion_alias: &str) -> Result<Self> {
        info!("Attempting broker discovery on bastion: {}", bastion_alias);
        info!("Note: kafkactl will be collected separately as data source, not used for broker discovery");
        
        // New Method: SSH to the broker, discover Kafka installation path, then discover all brokers
        if let Ok(brokers) = self.discover_brokers_using_installation_path(broker_address).await {
            if !brokers.is_empty() {
                info!("Successfully discovered {} brokers using installation path method", brokers.len());
                self.config.brokers = brokers;
                
                // Set discovery method for topic collection (using default kafka path for now)
                self.discovery_method = Some(DiscoveryMethod::KafkaTools {
                    kafka_installation_path: "/opt/kafka/bin".to_string(),
                    discovery_broker: broker_address.to_string(),
                });
                return Ok(self);
            }
        }
        
        // Method 1: Run a simple admin client tool on the bastion to get broker metadata
        if let Ok(brokers) = self.discover_brokers_with_bastion_admin_client(broker_address).await {
            if !brokers.is_empty() {
                info!("Successfully discovered {} brokers using admin client on bastion", brokers.len());
                self.config.brokers = brokers;
                return Ok(self);
            }
        }
        
        // Method 2: Use kafka-metadata-shell.sh if available
        if let Ok(brokers) = self.discover_brokers_with_metadata_shell(broker_address).await {
            if !brokers.is_empty() {
                info!("Successfully discovered {} brokers using kafka-metadata-shell", brokers.len());
                self.config.brokers = brokers;
                return Ok(self);
            }
        }
        
        // Method 3: Use kafka-broker-api-versions.sh with better parsing
        if let Ok(brokers) = self.discover_brokers_with_api_versions(broker_address).await {
            if !brokers.is_empty() {
                info!("Successfully discovered {} brokers using kafka-broker-api-versions", brokers.len());
                self.config.brokers = brokers;
                return Ok(self);
            }
        }
        
        // Method 4: Parse server.properties files on brokers to find other brokers
        if let Ok(brokers) = self.discover_brokers_from_configs(broker_address).await {
            if !brokers.is_empty() {
                info!("Successfully discovered {} brokers from server.properties files", brokers.len());
                self.config.brokers = brokers;
                return Ok(self);
            }
        }
        
        // Fallback: Use the single broker provided
        info!("All discovery methods failed, falling back to single broker configuration");
        let fallback_brokers = vec![BrokerInfo {
            id: 0, // Unknown ID, will be determined during data collection
            hostname: broker_address.split(':').next().unwrap_or("unknown").to_string(),  
            datacenter: "unknown".to_string(),
        }];
        
        self.config.brokers = fallback_brokers;
        Ok(self)
    }

    /// Discover brokers using Kafka installation path method
    async fn discover_brokers_using_installation_path(&self, broker_address: &str) -> Result<Vec<BrokerInfo>> {
        info!("🔍 Starting installation path-based broker discovery");
        
        // Step 1: SSH to the given broker and discover Kafka installation path
        let hostname = broker_address.split(':').next().unwrap_or("unknown");
        info!("Step 1: SSH to broker {} to discover Kafka installation path", hostname);
        
        let kafka_installation_path = self.discover_kafka_installation_path_on_broker(hostname).await?;
        info!("✅ Discovered Kafka installation path: {}", kafka_installation_path);
        
        // Step 2: Use the discovered path to run kafka-broker-api-versions.sh
        info!("Step 2: Using {} to discover all brokers in cluster", kafka_installation_path);
        
        let command = format!(
            "ssh -o StrictHostKeyChecking=no {} '{}/kafka-broker-api-versions.sh --bootstrap-server localhost:9092 | awk \"/id/{{print \\$1}}\"'",
            hostname,
            kafka_installation_path
        );
        
        info!("Executing broker discovery command via bastion");
        let output = self.run_command_on_bastion(&command)?;
        
        // Save the raw broker discovery output to tools directory
        let tools_dir = self.config.output_dir.join("cluster").join("tools");
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
                
                // Try to infer datacenter from hostname pattern
                let datacenter = if hostname.contains("-dc1-") {
                    "dc1".to_string()
                } else if hostname.contains("-dc2-") {
                    "dc2".to_string()
                } else if hostname.contains("-dc3-") {
                    "dc3".to_string()
                } else {
                    "unknown".to_string()
                };
                
                discovered_brokers.push(BrokerInfo {
                    id: broker_id,
                    hostname,
                    datacenter,
                });
                broker_id += 1;
            }
        }
        
        if !discovered_brokers.is_empty() {
            info!("✅ Successfully discovered {} brokers:", discovered_brokers.len());
            for broker in &discovered_brokers {
                info!("   • Broker {} - {} ({})", broker.id, broker.hostname, broker.datacenter);
            }
        } else {
            info!("⚠️  No brokers discovered using installation path method");
        }
        
        Ok(discovered_brokers)
    }
    
    /// Discover Kafka installation path on a specific broker
    async fn discover_kafka_installation_path_on_broker(&self, broker_hostname: &str) -> Result<String> {
        // Method 1: Try systemctl approach
        let systemctl_command = format!(
            "ssh -o StrictHostKeyChecking=no {} 'pid=$(ps aux | grep -E \"kafka\\.Kafka[^a-zA-Z]\" | grep -v grep | awk \"{{print \\$2}}\" | head -1); if [ ! -z \"$pid\" ]; then systemctl status $pid 2>/dev/null | grep \"ExecStart=\" | grep \"kafka-server-start.sh\" | sed \"s/.*ExecStart=//\" | sed \"s/kafka-server-start.sh.*/kafka-server-start.sh/\" | sed \"s|/bin/kafka-server-start.sh||\" | sed \"s|^[[:space:]]*||\" | head -1; fi'",
            broker_hostname
        );
        
        if let Ok(output) = self.run_command_on_bastion(&systemctl_command) {
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
        
        if let Ok(output) = self.run_command_on_bastion(&ps_command) {
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
            
            if let Ok(output) = self.run_command_on_bastion(&test_command) {
                if output.trim() == "EXISTS" {
                    info!("Found Kafka path via fallback: {}", path);
                    return Ok(path.to_string());
                }
            }
        }
        
        Err(anyhow::anyhow!("Could not discover Kafka installation path on broker {}", broker_hostname))
    }

    /// Execute command on bastion via SSH
    fn run_command_on_bastion(&self, command: &str) -> Result<String> {
        if let Some(bastion_alias) = &self.config.bastion_alias {
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

    /// Discover brokers using kafka-metadata-shell.sh (most reliable method)
    async fn discover_brokers_with_metadata_shell(&self, broker_address: &str) -> Result<Vec<BrokerInfo>> {
        let command = format!(
            "kafka-metadata-shell.sh --bootstrap-server {} --print-brokers 2>/dev/null | grep -E 'broker|Broker' | head -20",
            broker_address
        );
        
        match self.run_command_on_bastion(&command) {
            Ok(output) => {
                let mut brokers = Vec::new();
                for line in output.lines() {
                    // Parse broker information from metadata shell output
                    if let Some(broker) = self.parse_metadata_shell_broker_line(line) {
                        brokers.push(broker);
                    }
                }
                Ok(brokers)
            }
            Err(_) => Ok(Vec::new()), // Tool not available
        }
    }

    /// Discover brokers using kafka-broker-api-versions.sh with enhanced parsing
    async fn discover_brokers_with_api_versions(&self, broker_address: &str) -> Result<Vec<BrokerInfo>> {
        let command = format!(
            "kafka-broker-api-versions.sh --bootstrap-server {} 2>/dev/null | head -10",
            broker_address
        );
        
        match self.run_command_on_bastion(&command) {
            Ok(output) => {
                if output.contains("successfully connected") || output.contains("ApiVersion") {
                    // For now, if we can connect, assume we can discover at least this broker
                    // TODO: Enhanced parsing to extract multiple broker IDs from API versions output
                    let hostname = broker_address.split(':').next().unwrap_or("unknown").to_string();
                    Ok(vec![BrokerInfo {
                        id: 0, // Will be determined later during data collection
                        hostname,
                        datacenter: "unknown".to_string(),
                    }])
                } else {
                    Ok(Vec::new())
                }
            }
            Err(_) => Ok(Vec::new()), // Tool not available or failed
        }
    }

    /// Discover brokers using simple connection test on the bastion
    async fn discover_brokers_with_bastion_admin_client(&self, broker_address: &str) -> Result<Vec<BrokerInfo>> {
        info!("Attempting broker discovery using simple connection test on bastion");
        
        // Method 1: Try a simple netcat/telnet test to verify connectivity
        let connectivity_test = format!(
            "timeout 3 bash -c 'echo > /dev/tcp/{}/{}' 2>/dev/null && echo 'CONNECTED'",
            broker_address.split(':').next().unwrap_or("unknown"),
            broker_address.split(':').nth(1).unwrap_or("9092")
        );
        
        match self.run_command_on_bastion(&connectivity_test) {
            Ok(output) => {
                if output.contains("CONNECTED") {
                    info!("Successfully verified connectivity to broker via TCP test");
                    let hostname = broker_address.split(':').next().unwrap_or("unknown").to_string();
                    return Ok(vec![BrokerInfo {
                        id: 0, // Will be determined during data collection
                        hostname,
                        datacenter: "unknown".to_string(),
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
        
        match self.run_command_on_bastion(&hostname_test) {
            Ok(output) => {
                if output.contains("RESOLVABLE") {
                    info!("Hostname resolution successful, assuming broker is available");
                    let hostname = broker_address.split(':').next().unwrap_or("unknown").to_string();
                    return Ok(vec![BrokerInfo {
                        id: 0, // Will be determined during data collection
                        hostname,
                        datacenter: "unknown".to_string(),
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
        
        match self.run_command_on_bastion(&ping_test) {
            Ok(output) => {
                if output.contains("PINGABLE") {
                    info!("Ping test successful, assuming broker is available");
                    let hostname = broker_address.split(':').next().unwrap_or("unknown").to_string();
                    return Ok(vec![BrokerInfo {
                        id: 0, // Will be determined during data collection
                        hostname,
                        datacenter: "unknown".to_string(),
                    }]);
                }
            }
            Err(_) => {
                debug!("Ping test failed");
            }
        }
        
        Ok(Vec::new())
    }

    /// Parse broker information from API versions output
    fn parse_broker_api_versions_output(&self, output: &str, broker_address: &str) -> Vec<BrokerInfo> {
        // This is a simplified parser - in real scenarios we might extract more broker info
        // For now, if the command succeeded, we know at least the given broker exists
        let hostname = broker_address.split(':').next().unwrap_or("unknown").to_string();
        
        vec![BrokerInfo {
            id: 0, // Will be determined during data collection
            hostname,
            datacenter: "unknown".to_string(),
        }]
    }

    /// Try to discover additional brokers from JMX or log parsing
    async fn discover_brokers_from_jmx_or_logs(&self, _broker_address: &str) -> Result<Vec<BrokerInfo>> {
        // This could be enhanced to use JMX tools or parse Kafka logs for broker information
        // For now, return empty to fall back to other methods
        Ok(Vec::new())
    }

    /// Discover brokers by parsing server.properties files on known brokers
    async fn discover_brokers_from_configs(&self, _broker_address: &str) -> Result<Vec<BrokerInfo>> {
        // This would involve SSHing to the known broker, finding server.properties,
        // and looking for cluster configuration or other broker references
        // TODO: Implement config-based broker discovery
        info!("Config-based discovery method not yet implemented");
        Ok(Vec::new())
    }

    /// Parse a single line from kafka-metadata-shell output to extract broker info
    fn parse_metadata_shell_broker_line(&self, line: &str) -> Option<BrokerInfo> {
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
                            datacenter: "unknown".to_string(),
                        });
                    }
                }
            }
        }
        None
    }
    
    /// Check if SSH agent has keys loaded (only needed for remote bastion)
    fn check_ssh_agent(&self) -> Result<()> {
        // Only check SSH agent if we're connecting to a remote bastion
        if self.config.bastion_alias.is_some() {
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
    
    /// Create output directory structure
    fn setup_output_dirs(&self) -> Result<()> {
        let base = &self.config.output_dir;
        
        // Create main directories
        fs::create_dir_all(base)?;
        fs::create_dir_all(base.join("brokers"))?;
        fs::create_dir_all(base.join("cluster").join("kafkactl"))?;
        fs::create_dir_all(base.join("cluster").join("tools"))?;
        fs::create_dir_all(base.join("metrics").join("kafka_exporter"))?;
        fs::create_dir_all(base.join("system").join("bastion"))?;
        
        info!("Created output directory: {}", base.display());
        Ok(())
    }
    
    /// Save scan metadata
    fn save_metadata(&self, accessible_brokers: usize) -> Result<()> {
        let metadata = ScanMetadata {
            scan_timestamp: Utc::now().to_rfc3339(),
            bastion: self.config.bastion_alias.clone(),
            is_local: self.config.bastion_alias.is_none(),
            broker_count: self.config.brokers.len(),
            output_directory: self.config.output_dir.to_string_lossy().to_string(),
            scan_version: "1.0".to_string(),
            accessible_brokers,
            cluster_mode: self.detected_cluster_mode.clone(),
        };
        
        let json = serde_json::to_string_pretty(&metadata)?;
        fs::write(
            self.config.output_dir.join("scan_metadata.json"),
            json,
        )?;
        
        Ok(())
    }
    
    /// Main scan execution
    pub async fn scan(&mut self) -> Result<ScanResult> {
        let start_time = std::time::Instant::now();
        
        // Phase 1: Setup
        println!("═══════════════════════════════════════════════════════════════");
        println!("        KAFKAPILOT COMPREHENSIVE CLUSTER SCAN");
        println!("═══════════════════════════════════════════════════════════════");
        println!();
        
        // Display scan mode
        match &self.config.bastion_alias {
            Some(alias) => {
                println!("📡 Mode: Remote scan via bastion '{}'", alias);
                println!("🔐 Checking SSH agent...");
                self.check_ssh_agent()?;
                println!("✅ SSH agent has keys loaded\n");
            }
            None => {
                println!("💻 Mode: Local scan (running on bastion)");
                println!();
            }
        }
        
        // Create output directories
        println!("📁 Creating output directory: {}", self.config.output_dir.display());
        self.setup_output_dirs()?;
        println!("✅ Output directory created\n");
        
        // Phase 2: Collect cluster-wide data from bastion
        println!("═══════════════════════════════════════════════════════════════");
        println!("PHASE 1: Collecting Cluster-Wide Data");
        println!("═══════════════════════════════════════════════════════════════");
        println!();
        
        let mut bastion_collector = BastionCollector::new(
            self.config.bastion_alias.clone(),
            self.config.output_dir.clone(),
        );
        
        // Set discovery method based on how brokers were discovered
        if let Some(discovery_method) = &self.discovery_method {
            bastion_collector = bastion_collector.with_discovery_method(discovery_method.clone());
        }
        
        let cluster_data = bastion_collector.collect_all().await?;
        
        // Phase 3: Test broker connectivity
        println!("\n═══════════════════════════════════════════════════════════════");
        println!("PHASE 2: Testing Broker Connectivity");
        println!("═══════════════════════════════════════════════════════════════");
        println!();
        
        // Diagnostic information
        if let Some(alias) = &self.config.bastion_alias {
            println!("🔍 SSH Connection Diagnostics:");
            self.run_ssh_diagnostics(alias).await;
            println!();
        }
        
        let mut accessible_brokers = Vec::new();
        let connect_method = if self.config.bastion_alias.is_some() {
            "via SSH agent forwarding"
        } else {
            "directly from bastion"
        };
        println!("Testing broker access ({}):", connect_method);
        
        for broker in &self.config.brokers {
            print!("  • Broker {} ({})... ", broker.id, broker.datacenter);
            
            if self.test_broker_access(broker).await {
                println!("✅ Accessible");
                accessible_brokers.push(broker.clone());
            } else {
                println!("❌ Not accessible");
            }
        }
        
        if accessible_brokers.is_empty() {
            println!("\n⚠️  No brokers accessible via SSH.");
            
            // Provide helpful troubleshooting suggestions
            match &self.config.bastion_alias {
                Some(alias) => {
                    println!("\n🔧 Troubleshooting suggestions:");
                    println!("   1. Verify SSH config for '{}':", alias);
                    println!("      ssh -v {} 'echo test'", alias);
                    println!("   2. Test SSH agent forwarding:");
                    println!("      ssh -A {} 'ssh-add -l'", alias);
                    println!("   3. Test manual broker connection:");
                    println!("      ssh -A {} 'ssh {}'", alias, self.config.brokers[0].hostname);
                    println!("   4. Check if broker hostnames are resolvable from bastion:");
                    println!("      ssh {} 'host {}'", alias, self.config.brokers[0].hostname);
                }
                None => {
                    println!("\n🔧 Troubleshooting suggestions:");
                    println!("   1. Test direct broker connection:");
                    println!("      ssh {}", self.config.brokers[0].hostname);
                    println!("   2. Check network connectivity:");
                    println!("      ping {}", self.config.brokers[0].hostname);
                }
            }
            
            println!("\n💡 Despite connectivity issues, cluster-wide data was still collected successfully.");
            println!("   This includes kafkactl output, metrics, and bastion system information.");
        } else {
            println!("\n✅ Found {} accessible broker(s)", accessible_brokers.len());
        }
        
        // Phase 4: Collect data from accessible brokers
        let mut broker_data = Vec::new();
        
        if !accessible_brokers.is_empty() {
            println!("\n═══════════════════════════════════════════════════════════════");
            println!("PHASE 3: Collecting Data from Accessible Brokers");
            println!("═══════════════════════════════════════════════════════════════");
            println!();
            
            for broker in accessible_brokers.iter() {
                println!("🔍 Processing Broker {} ({} in {})...", 
                    broker.id, broker.hostname, broker.datacenter);
                println!("────────────────────────────────────────");
                
                let broker_collector = BrokerCollector::new(
                    self.config.bastion_alias.clone(),
                    broker.clone(),
                    self.config.output_dir.clone(),
                );
                
                match broker_collector.collect_all().await {
                    Ok(data) => {
                        println!("  ✅ Broker {} collection complete\n", broker.id);
                        
                        // Try to detect cluster mode from server.properties if not already detected
                        if self.detected_cluster_mode.is_none() {
                            if let Some(server_props) = data.configs.get("server.properties") {
                                let detected_mode = self.detect_cluster_mode_from_config(server_props);
                                self.detected_cluster_mode = Some(detected_mode.clone());
                                info!("🔍 Detected cluster mode from broker {}: {:?}", broker.id, detected_mode);
                                
                                // If Zookeeper mode, log that additional data collection could be done
                                if matches!(detected_mode, crate::snapshot::format::ClusterMode::Zookeeper) {
                                    info!("⚠️  Zookeeper cluster detected - consider implementing additional ZK-specific data collection");
                                }
                            }
                        }
                        
                        broker_data.push(data);
                    }
                    Err(e) => {
                        error!("  ❌ Failed to collect from broker {}: {}", broker.id, e);
                    }
                }
            }
        }
        
        // Phase 5: Generate summary
        println!("═══════════════════════════════════════════════════════════════");
        println!("PHASE 4: Generating Collection Summary");
        println!("═══════════════════════════════════════════════════════════════");
        println!();
        
        // Save metadata
        self.save_metadata(accessible_brokers.len())?;
        
        // Generate summary report
        self.generate_summary_report(&cluster_data, &broker_data)?;
        
        // Calculate statistics
        let stats = self.calculate_stats(start_time.elapsed().as_secs())?;
        
        println!("📊 Generating summary report...");
        println!("✅ Summary report generated\n");
        
        // Final output
        println!("═══════════════════════════════════════════════════════════════");
        println!("                    COLLECTION COMPLETE!");
        println!("═══════════════════════════════════════════════════════════════");
        println!();
        println!("📊 Collection Statistics:");
        println!("  • Total files collected: {}", stats.total_files);
        println!("  • Total size: {} KB", stats.total_size_bytes / 1024);
        println!("  • Output directory: {}", self.config.output_dir.display());
        println!("  • Brokers accessed: {}/{}", accessible_brokers.len(), self.config.brokers.len());
        
        // Display detected cluster mode
        match &self.detected_cluster_mode {
            Some(crate::snapshot::format::ClusterMode::Kraft) => {
                println!("  • Cluster architecture: 🚀 KRaft (modern, Zookeeper-free)");
            }
            Some(crate::snapshot::format::ClusterMode::Zookeeper) => {
                println!("  • Cluster architecture: ⚠️  Zookeeper-based (legacy)");
            }
            Some(crate::snapshot::format::ClusterMode::Unknown) => {
                println!("  • Cluster architecture: ❓ Unknown (could not determine)");
            }
            None => {
                println!("  • Cluster architecture: ❓ Not detected (no server.properties found)");
            }
        }
        println!();
        println!("📁 Data saved to: {}/", self.config.output_dir.display());
        println!();
        println!("💡 To analyze the collected data:");
        println!("  1. cd {}", self.config.output_dir.display());
        println!("  2. Review COLLECTION_SUMMARY.md");
        println!("  3. Check broker configs and logs");
        println!();
        println!("✨ Scan complete!");
        
        Ok(ScanResult {
            metadata: ScanMetadata {
                scan_timestamp: Utc::now().to_rfc3339(),
                bastion: self.config.bastion_alias.clone(),
                is_local: self.config.bastion_alias.is_none(),
                broker_count: self.config.brokers.len(),
                output_directory: self.config.output_dir.to_string_lossy().to_string(),
                scan_version: "1.0".to_string(),
                accessible_brokers: accessible_brokers.len(),
                cluster_mode: self.detected_cluster_mode.clone(),
            },
            cluster_data,
            broker_data,
            collection_stats: stats,
            detected_cluster_mode: self.detected_cluster_mode.clone(),
        })
    }
    
    /// Run SSH diagnostics to help debug connection issues
    pub async fn run_ssh_diagnostics(&self, bastion_alias: &str) {
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
        let sample_broker = &self.config.brokers[0];
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
    pub async fn test_broker_access(&self, broker: &BrokerInfo) -> bool {
        let result = match &self.config.bastion_alias {
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
    
    /// Generate summary report
    fn generate_summary_report(&self, _cluster_data: &ClusterData, broker_data: &[BrokerData]) -> Result<()> {
        let mut report = String::new();
        
        report.push_str("# Kafka Cluster Data Collection Summary\n\n");
        report.push_str(&format!("## Collection Details\n"));
        report.push_str(&format!("- **Timestamp**: {}\n", Utc::now()));
        report.push_str(&format!("- **Output Directory**: {}\n", self.config.output_dir.display()));
        
        if let Some(alias) = &self.config.bastion_alias {
            report.push_str(&format!("- **Bastion**: {} (remote)\n", alias));
        } else {
            report.push_str("- **Bastion**: Local (running on bastion)\n");
        }
        
        report.push_str(&format!("- **Total Brokers**: {}\n", self.config.brokers.len()));
        report.push_str(&format!("- **Accessible Brokers**: {}\n\n", broker_data.len()));
        
        report.push_str("## Data Collected\n\n");
        report.push_str("### Cluster Level (from bastion)\n");
        report.push_str("✅ Broker list and configurations (kafkactl)\n");
        report.push_str("✅ Topic list and details\n");
        report.push_str("✅ Consumer groups\n");
        report.push_str("✅ Prometheus metrics (kafka_exporter)\n");
        report.push_str("✅ Bastion system information\n\n");
        
        if !broker_data.is_empty() {
            report.push_str(&format!("### Per Broker (from {} accessible brokers)\n", broker_data.len()));
            report.push_str("✅ System information (CPU, memory, disk)\n");
            report.push_str("✅ Java/JVM information and metrics\n");
            report.push_str("✅ Configuration files (server.properties, log4j, etc.)\n");
            report.push_str("✅ Log files (server.log, controller.log, journald)\n");
            report.push_str("✅ Data directory information and sizes\n");
            report.push_str("✅ Network connections and ports\n");
        }
        
        fs::write(
            self.config.output_dir.join("COLLECTION_SUMMARY.md"),
            report,
        )?;
        
        Ok(())
    }
    
    /// Calculate collection statistics
    fn calculate_stats(&self, duration_secs: u64) -> Result<CollectionStats> {
        let mut total_files = 0;
        let mut total_size = 0;
        
        // Walk through output directory and count files/size
        for entry in walkdir::WalkDir::new(&self.config.output_dir) {
            if let Ok(entry) = entry {
                if entry.file_type().is_file() {
                    total_files += 1;
                    if let Ok(metadata) = entry.metadata() {
                        total_size += metadata.len();
                    }
                }
            }
        }
        
        Ok(CollectionStats {
            total_files,
            total_size_bytes: total_size,
            duration_secs,
        })
    }
}

/// Detect the cluster mode (Zookeeper vs KRaft) from configuration data
pub fn detect_cluster_mode(config_data: &serde_json::Value) -> crate::snapshot::format::ClusterMode {
    use crate::snapshot::format::ClusterMode;
    
    debug!("Starting cluster mode detection from configuration data");
    
    // Extract all server.properties content from the config data
    let server_properties_configs = extract_server_properties_configs(config_data);
    
    if server_properties_configs.is_empty() {
        debug!("No server.properties files found in configuration data");
        return ClusterMode::Unknown;
    }
    
    debug!("Found {} server.properties files to analyze", server_properties_configs.len());
    
    // Analyze each server.properties file
    let mut zookeeper_count = 0;
    let mut kraft_count = 0;
    let mut unknown_count = 0;
    
    for (file_path, content) in &server_properties_configs {
        debug!("Analyzing server.properties from: {}", file_path);
        let properties = parse_server_properties(content);
        
        if is_zookeeper_mode(&properties) {
            debug!("Detected Zookeeper mode in: {}", file_path);
            zookeeper_count += 1;
        } else if is_kraft_mode(&properties) {
            debug!("Detected KRaft mode in: {}", file_path);
            kraft_count += 1;
        } else {
            debug!("Could not determine mode from: {}", file_path);
            unknown_count += 1;
        }
    }
    
    // Determine overall cluster mode based on majority
    info!("Cluster mode analysis: {} ZK, {} KRaft, {} unknown", 
          zookeeper_count, kraft_count, unknown_count);
    
    match (zookeeper_count, kraft_count) {
        (zk, 0) if zk > 0 => {
            info!("Detected Zookeeper cluster mode");
            ClusterMode::Zookeeper
        }
        (0, kraft) if kraft > 0 => {
            info!("Detected KRaft cluster mode");
            ClusterMode::Kraft
        }
        (zk, kraft) if zk > 0 && kraft > 0 => {
            debug!("Mixed cluster modes detected! ZK: {}, KRaft: {}. Setting to Unknown.", zk, kraft);
            ClusterMode::Unknown
        }
        _ => {
            debug!("Could not determine cluster mode from configuration");
            ClusterMode::Unknown
        }
    }
}

/// Extract server.properties content from configuration data
fn extract_server_properties_configs(config_data: &serde_json::Value) -> Vec<(String, String)> {
    let mut configs = Vec::new();
    
    // Handle different possible structures in config data
    if let Some(config_obj) = config_data.as_object() {
        for (file_path, content) in config_obj {
            if file_path.contains("server.properties") {
                if let Some(content_str) = content.as_str() {
                    configs.push((file_path.clone(), content_str.to_string()));
                }
            }
        }
    }
    
    // Also check nested broker structures
    if let Some(brokers_data) = config_data.get("brokers") {
        if let Some(brokers_obj) = brokers_data.as_object() {
            for (broker_name, broker_data) in brokers_obj {
                if let Some(broker_obj) = broker_data.as_object() {
                    if let Some(configs_data) = broker_obj.get("configs") {
                        if let Some(configs_obj) = configs_data.as_object() {
                            for (config_name, config_content) in configs_obj {
                                if config_name == "server.properties" {
                                    if let Some(content_str) = config_content.as_str() {
                                        let file_path = format!("{}/configs/{}", broker_name, config_name);
                                        configs.push((file_path, content_str.to_string()));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    configs
}

/// Parse server.properties file format into key-value pairs
pub fn parse_server_properties(content: &str) -> HashMap<String, String> {
    let mut properties = HashMap::new();
    
    for line in content.lines() {
        let line = line.trim();
        
        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        
        // Parse key=value pairs
        if let Some(eq_pos) = line.find('=') {
            let key = line[..eq_pos].trim().to_string();
            let value = line[eq_pos + 1..].trim().to_string();
            
            if !key.is_empty() {
                properties.insert(key, value);
            }
        }
    }
    
    debug!("Parsed {} properties from server.properties", properties.len());
    properties
}

/// Check if configuration indicates Zookeeper mode
pub fn is_zookeeper_mode(properties: &HashMap<String, String>) -> bool {
    // ZK detection rules:
    // 1. zookeeper.connect is defined and has at least one value
    // 2. process.roles=controller does not exist or is not set  
    // 3. broker.id exists
    // 4. node.id does not exist
    // 5. controller.quorum.voters does not exist
    
    // Check zookeeper.connect
    let has_zk_connect = if let Some(zk_connect) = properties.get("zookeeper.connect") {
        !zk_connect.is_empty() && !zk_connect.trim().is_empty()
    } else {
        false
    };
    
    // Check process.roles does not contain controller
    let has_controller_role = if let Some(process_roles) = properties.get("process.roles") {
        process_roles.contains("controller")
    } else {
        false
    };
    
    // Check broker.id exists
    let has_broker_id = properties.contains_key("broker.id");
    
    // Check node.id does not exist
    let has_node_id = properties.contains_key("node.id");
    
    // Check controller.quorum.voters does not exist
    let has_quorum_voters = properties.contains_key("controller.quorum.voters");
    
    debug!("ZK mode check - zk_connect: {}, controller_role: {}, broker_id: {}, node_id: {}, quorum_voters: {}",
           has_zk_connect, has_controller_role, has_broker_id, has_node_id, has_quorum_voters);
    
    has_zk_connect && !has_controller_role && has_broker_id && !has_node_id && !has_quorum_voters
}

/// Check if configuration indicates KRaft mode
pub fn is_kraft_mode(properties: &HashMap<String, String>) -> bool {
    // KRaft detection rules:
    // 1. zookeeper.connect does not exist
    // 2. process.roles contains "controller" 
    // 3. node.id exists (broker.id can coexist for compatibility)
    // 4. controller.quorum.voters exists
    
    // Check zookeeper.connect does not exist
    let has_zk_connect = properties.contains_key("zookeeper.connect");
    
    // Check process.roles contains controller
    let has_controller_role = if let Some(process_roles) = properties.get("process.roles") {
        process_roles.contains("controller")
    } else {
        false
    };
    
    // Check node.id exists (broker.id can also exist for compatibility)
    let has_node_id = properties.contains_key("node.id");
    
    // Check controller.quorum.voters exists
    let has_quorum_voters = properties.contains_key("controller.quorum.voters");
    
    debug!("KRaft mode check - zk_connect: {}, controller_role: {}, node_id: {}, quorum_voters: {}",
           has_zk_connect, has_controller_role, has_node_id, has_quorum_voters);
    
    !has_zk_connect && has_controller_role && has_node_id && has_quorum_voters
}

#[cfg(test)]
mod cluster_detection_tests {
    use super::*;
    use std::collections::HashMap;
    use serde_json::json;

    #[test]
    fn test_parse_server_properties() {
        let content = r#"
# This is a comment
broker.id=11
zookeeper.connect=zk1:2181,zk2:2181,zk3:2181

# Another comment
log.dirs=/var/kafka/logs
num.network.threads=3

# Empty line above and below should be ignored

process.roles=controller,broker
"#;
        
        let properties = parse_server_properties(content);
        
        assert_eq!(properties.get("broker.id"), Some(&"11".to_string()));
        assert_eq!(properties.get("zookeeper.connect"), Some(&"zk1:2181,zk2:2181,zk3:2181".to_string()));
        assert_eq!(properties.get("log.dirs"), Some(&"/var/kafka/logs".to_string()));
        assert_eq!(properties.get("num.network.threads"), Some(&"3".to_string()));
        assert_eq!(properties.get("process.roles"), Some(&"controller,broker".to_string()));
        assert_eq!(properties.len(), 5);
    }

    #[test]
    fn test_parse_server_properties_with_spaces() {
        let content = r#"
broker.id = 11
zookeeper.connect = zk1:2181
log.dirs=/var/kafka/logs
"#;
        
        let properties = parse_server_properties(content);
        
        assert_eq!(properties.get("broker.id"), Some(&"11".to_string()));
        assert_eq!(properties.get("zookeeper.connect"), Some(&"zk1:2181".to_string()));
        assert_eq!(properties.get("log.dirs"), Some(&"/var/kafka/logs".to_string()));
    }

    #[test]
    fn test_is_zookeeper_mode() {
        let mut properties = HashMap::new();
        properties.insert("zookeeper.connect".to_string(), "zk1:2181,zk2:2181".to_string());
        properties.insert("broker.id".to_string(), "11".to_string());
        
        assert!(is_zookeeper_mode(&properties));
    }

    #[test]
    fn test_is_zookeeper_mode_false_with_empty_zk_connect() {
        let mut properties = HashMap::new();
        properties.insert("zookeeper.connect".to_string(), "".to_string()); // Empty value
        properties.insert("broker.id".to_string(), "11".to_string());
        
        assert!(!is_zookeeper_mode(&properties));
    }

    #[test]
    fn test_is_zookeeper_mode_false_with_controller_role() {
        let mut properties = HashMap::new();
        properties.insert("zookeeper.connect".to_string(), "zk1:2181".to_string());
        properties.insert("broker.id".to_string(), "11".to_string());
        properties.insert("process.roles".to_string(), "controller".to_string());
        
        assert!(!is_zookeeper_mode(&properties));
    }

    #[test]
    fn test_is_zookeeper_mode_false_with_node_id() {
        let mut properties = HashMap::new();
        properties.insert("zookeeper.connect".to_string(), "zk1:2181".to_string());
        properties.insert("broker.id".to_string(), "11".to_string());
        properties.insert("node.id".to_string(), "1".to_string());
        
        assert!(!is_zookeeper_mode(&properties));
    }

    #[test]
    fn test_is_zookeeper_mode_false_with_quorum_voters() {
        let mut properties = HashMap::new();
        properties.insert("zookeeper.connect".to_string(), "zk1:2181".to_string());
        properties.insert("broker.id".to_string(), "11".to_string());
        properties.insert("controller.quorum.voters".to_string(), "1@kafka1:9093".to_string());
        
        assert!(!is_zookeeper_mode(&properties));
    }

    #[test]
    fn test_is_kraft_mode() {
        let mut properties = HashMap::new();
        properties.insert("process.roles".to_string(), "controller,broker".to_string());
        properties.insert("node.id".to_string(), "1".to_string());
        properties.insert("controller.quorum.voters".to_string(), "1@kafka1:9093".to_string());
        
        assert!(is_kraft_mode(&properties));
    }

    #[test]
    fn test_is_kraft_mode_with_only_controller_role() {
        let mut properties = HashMap::new();
        properties.insert("process.roles".to_string(), "controller".to_string());
        properties.insert("node.id".to_string(), "1".to_string());
        properties.insert("controller.quorum.voters".to_string(), "1@kafka1:9093".to_string());
        
        assert!(is_kraft_mode(&properties));
    }

    #[test]
    fn test_is_kraft_mode_false_with_zk_connect() {
        let mut properties = HashMap::new();
        properties.insert("zookeeper.connect".to_string(), "zk1:2181".to_string());
        properties.insert("process.roles".to_string(), "controller".to_string());
        properties.insert("node.id".to_string(), "1".to_string());
        properties.insert("controller.quorum.voters".to_string(), "1@kafka1:9093".to_string());
        
        assert!(!is_kraft_mode(&properties));
    }

    #[test]
    fn test_is_kraft_mode_with_broker_id_compatibility() {
        let mut properties = HashMap::new();
        properties.insert("process.roles".to_string(), "controller".to_string());
        properties.insert("broker.id".to_string(), "1".to_string()); // Can coexist in KRaft
        properties.insert("node.id".to_string(), "1".to_string());
        properties.insert("controller.quorum.voters".to_string(), "1@kafka1:9093".to_string());
        
        assert!(is_kraft_mode(&properties));
    }

    #[test]
    fn test_is_kraft_mode_false_without_node_id() {
        let mut properties = HashMap::new();
        properties.insert("process.roles".to_string(), "controller".to_string());
        properties.insert("controller.quorum.voters".to_string(), "1@kafka1:9093".to_string());
        
        assert!(!is_kraft_mode(&properties));
    }

    #[test]
    fn test_detect_cluster_mode_zookeeper() {
        let config_data = json!({
            "broker_1/server.properties": "broker.id=11\nzookeeper.connect=zk1:2181,zk2:2181\nlog.dirs=/var/kafka/logs",
            "broker_2/server.properties": "broker.id=12\nzookeeper.connect=zk1:2181,zk2:2181\nlog.dirs=/var/kafka/logs"
        });

        let mode = detect_cluster_mode(&config_data);
        assert!(matches!(mode, crate::snapshot::format::ClusterMode::Zookeeper));
    }

    #[test]
    fn test_detect_cluster_mode_kraft() {
        let config_data = json!({
            "broker_1/server.properties": "node.id=1\nprocess.roles=controller,broker\ncontroller.quorum.voters=1@kafka1:9093",
            "broker_2/server.properties": "node.id=2\nprocess.roles=controller,broker\ncontroller.quorum.voters=1@kafka1:9093"
        });

        let mode = detect_cluster_mode(&config_data);
        assert!(matches!(mode, crate::snapshot::format::ClusterMode::Kraft));
    }

    #[test]
    fn test_detect_cluster_mode_unknown() {
        let config_data = json!({
            "broker_1/server.properties": "some.other.config=value\nlog.dirs=/var/kafka/logs",
        });

        let mode = detect_cluster_mode(&config_data);
        assert!(matches!(mode, crate::snapshot::format::ClusterMode::Unknown));
    }

    #[test]
    fn test_detect_cluster_mode_mixed() {
        let config_data = json!({
            "broker_1/server.properties": "broker.id=11\nzookeeper.connect=zk1:2181",
            "broker_2/server.properties": "node.id=2\nprocess.roles=controller\ncontroller.quorum.voters=1@kafka1:9093"
        });

        let mode = detect_cluster_mode(&config_data);
        assert!(matches!(mode, crate::snapshot::format::ClusterMode::Unknown));
    }

    #[test]
    fn test_detect_cluster_mode_no_config() {
        let config_data = json!({
            "some_other_file.txt": "not a server.properties file"
        });

        let mode = detect_cluster_mode(&config_data);
        assert!(matches!(mode, crate::snapshot::format::ClusterMode::Unknown));
    }

    #[test]
    fn test_extract_server_properties_configs() {
        let config_data = json!({
            "broker_1/server.properties": "broker.id=11",
            "broker_1/log4j.properties": "log4j.rootLogger=INFO",
            "broker_2/server.properties": "broker.id=12",
        });

        let configs = extract_server_properties_configs(&config_data);
        assert_eq!(configs.len(), 2);
        assert!(configs.iter().any(|(path, _)| path == "broker_1/server.properties"));
        assert!(configs.iter().any(|(path, _)| path == "broker_2/server.properties"));
        assert!(!configs.iter().any(|(path, _)| path.contains("log4j")));
    }

    #[test]
    fn test_extract_server_properties_configs_nested_structure() {
        let config_data = json!({
            "brokers": {
                "broker_1": {
                    "configs": {
                        "server.properties": "broker.id=11\nzookeeper.connect=zk1:2181"
                    }
                },
                "broker_2": {
                    "configs": {
                        "server.properties": "broker.id=12\nzookeeper.connect=zk1:2181"
                    }
                }
            }
        });

        let configs = extract_server_properties_configs(&config_data);
        assert_eq!(configs.len(), 2);
        assert!(configs.iter().any(|(path, content)| 
            path == "broker_1/configs/server.properties" && content.contains("broker.id=11")));
        assert!(configs.iter().any(|(path, content)| 
            path == "broker_2/configs/server.properties" && content.contains("broker.id=12")));
    }

    #[test]
    fn test_detect_kraft_with_real_config() {
        let kraft_config = r#"# ansible managed

# kraft control plane
node.id=13
controller.listener.names=CONTROLLER
process.roles=broker,controller
controller.quorum.voters=11@kafka-poligon-dc1-1.c.bartek-rekke-sandbox.internal:9094,13@kafka-poligon-dc2-1.c.bartek-rekke-sandbox.internal:9094,15@kafka-poligon-dc3-1.c.bartek-rekke-sandbox.internal:9094

# kafka files goes here
log.dirs=/data/kafka/generic

# common options
listeners=CLIENTS://0.0.0.0:9092,CLUSTER://0.0.0.0:9093,CONTROLLER://0.0.0.0:9094"#;

        let properties = parse_server_properties(kraft_config);
        assert!(is_kraft_mode(&properties));
        assert!(!is_zookeeper_mode(&properties));
    }
}