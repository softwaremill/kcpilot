use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use super::{BrokerData, BrokerInfo, ClusterData};
use super::log_discovery::LogDiscovery;
use super::enhanced_log_discovery::EnhancedLogDiscovery;

/// Discovery method used for broker/topic discovery
#[derive(Debug, Clone)]
pub enum DiscoveryMethod {
    /// Using --broker parameter with kafka tools
    KafkaTools { kafka_installation_path: String, discovery_broker: String },
    /// Using kafkactl (fallback when no --broker provided)
    Kafkactl,
}

/// Collector for bastion-level data (kafkactl, metrics, etc.)
pub struct BastionCollector {
    bastion_alias: Option<String>,  // None means we're running locally on the bastion
    output_dir: PathBuf,
    discovery_method: Option<DiscoveryMethod>,
}

impl BastionCollector {
    pub fn new(bastion_alias: Option<String>, output_dir: PathBuf) -> Self {
        Self {
            bastion_alias,
            output_dir,
            discovery_method: None,
        }
    }

    /// Set the discovery method to use for topic and other cluster resource discovery
    pub fn with_discovery_method(mut self, discovery_method: DiscoveryMethod) -> Self {
        self.discovery_method = Some(discovery_method);
        self
    }

    /// Execute command on bastion (either locally or via SSH)
    fn run_on_bastion(&self, command: &str) -> Result<String> {
        let output = match &self.bastion_alias {
            Some(alias) => {
                // Remote execution via SSH
                Command::new("ssh")
                    .arg(alias)
                    .arg(command)
                    .output()
                    .context(format!("Failed to execute on bastion: {}", command))?
            }
            None => {
                // Local execution (we're on the bastion)
                Command::new("sh")
                    .arg("-c")
                    .arg(command)
                    .output()
                    .context(format!("Failed to execute locally: {}", command))?
            }
        };
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Command failed: {}", stderr));
        }
        
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Check if kafkactl is available on the bastion
    fn check_kafkactl_availability(&self) -> bool {
        match self.run_on_bastion("which kafkactl") {
            Ok(output) => !output.trim().is_empty(),
            Err(_) => false,
        }
    }
    
    /// Collect all bastion-level data
    pub async fn collect_all(&self) -> Result<ClusterData> {
        let location = match &self.bastion_alias {
            Some(alias) => format!("from bastion '{}'", alias),
            None => "locally (running on bastion)".to_string(),
        };
        let mut kafkactl_data = HashMap::new();
        let kafkactl_dir = self.output_dir.join("cluster").join("kafkactl");
        
        // Check if kafkactl is available
        if !self.check_kafkactl_availability() {
            println!("⚠️  kafkactl not available {} - skipping kafkactl data collection", location);
            println!("   Continuing with other data collection methods...");
            
            // Create empty kafkactl directory to maintain expected structure
            fs::create_dir_all(&kafkactl_dir)?;
            fs::write(kafkactl_dir.join("unavailable.txt"), 
                "kafkactl tool was not available on the bastion host during scan")?;
        } else {
            println!("📊 Collecting kafkactl data {}...", location);
            
            // Get broker list
            print!("  • Getting broker list... ");
            if let Ok(brokers) = self.run_on_bastion("kafkactl get brokers -o yaml") {
                fs::write(kafkactl_dir.join("brokers.yaml"), &brokers)?;
                kafkactl_data.insert("brokers".to_string(), brokers);
                println!("✓");
            } else {
                println!("⚠");
            }
            
            // Get topics using the appropriate discovery method
            println!("📋 Topic Discovery Method:");
            match &self.discovery_method {
                Some(DiscoveryMethod::KafkaTools { kafka_installation_path, discovery_broker }) => {
                    println!("   🔧 Using kafka-topics.sh from installation path: {}", kafka_installation_path);
                    println!("   🎯 Discovery broker: {}", discovery_broker);
                    let tools_dir = self.output_dir.join("cluster").join("tools");
                    self.collect_topics_with_kafka_tools(kafka_installation_path, discovery_broker, &tools_dir, &mut kafkactl_data)?;
                }
                Some(DiscoveryMethod::Kafkactl) => {
                    println!("   🛠️  Using kafkactl (explicit method)");
                    self.collect_topics_with_kafkactl(&kafkactl_dir, &mut kafkactl_data)?;
                }
                None => {
                    println!("   🛠️  Using kafkactl (default fallback)");
                    self.collect_topics_with_kafkactl(&kafkactl_dir, &mut kafkactl_data)?;
                }
            }
            
            // Get consumer groups
            print!("  • Getting consumer groups... ");
            if let Ok(consumer_groups) = self.run_on_bastion("kafkactl get consumer-groups -o yaml") {
                fs::write(kafkactl_dir.join("consumer_groups.yaml"), &consumer_groups)?;
                kafkactl_data.insert("consumer_groups".to_string(), consumer_groups);
                println!("✓");
            } else {
                println!("⚠");
            }
            
            // Get individual broker configs
            println!("  • Getting broker configurations:");
            for broker_id in [11, 12, 13, 14, 15, 16] {
                print!("    - Broker {}... ", broker_id);
                if let Ok(config) = self.run_on_bastion(&format!("kafkactl describe broker {} -o yaml", broker_id)) {
                    fs::write(
                        kafkactl_dir.join(format!("broker_{}_config.yaml", broker_id)),
                        &config,
                    )?;
                    kafkactl_data.insert(format!("broker_{}_config", broker_id), config);
                    println!("✓");
                } else {
                    println!("⚠");
                }
            }
        }
        
        println!("✅ Kafkactl data collected\n");
        
        // Collect kafka_exporter metrics
        println!("📈 Collecting kafka_exporter metrics...");
        let metrics_dir = self.output_dir.join("metrics").join("kafka_exporter");
        let metrics = self.run_on_bastion("curl -s http://localhost:9308/metrics").ok();
        
        if let Some(ref m) = metrics {
            fs::write(metrics_dir.join("prometheus_metrics.txt"), m)?;
            if m.lines().count() > 0 {
                println!("✅ Kafka exporter metrics collected ({} lines)\n", m.lines().count());
            } else {
                println!("⚠️  No metrics from kafka_exporter\n");
            }
        } else {
            println!("⚠️  No metrics from kafka_exporter\n");
        }
        
        // Collect bastion system info
        println!("💻 Collecting bastion system info...");
        let mut bastion_info = HashMap::new();
        let system_dir = self.output_dir.join("system").join("bastion");
        
        let commands = vec![
            ("hostname", "hostname -f"),
            ("uptime", "uptime"),
            ("memory", "free -h"),
            ("disk", "df -h"),
            ("processes", "ps aux | grep -E 'kafka|zookeeper' | grep -v grep"),
            ("lscpu", "lscpu 2>/dev/null"),
            ("cpuinfo", "cat /proc/cpuinfo 2>/dev/null"),
        ];
        
        for (name, cmd) in commands {
            if let Ok(output) = self.run_on_bastion(cmd) {
                fs::write(system_dir.join(format!("{}.txt", name)), &output)?;
                bastion_info.insert(name.to_string(), output);
            }
        }
        
        println!("✅ Bastion system info collected");
        
        Ok(ClusterData {
            kafkactl_data,
            metrics,
            bastion_info,
        })
    }

    /// Collect topics using kafka-topics command (preferred method when --broker is provided)
    fn collect_topics_with_kafka_tools(
        &self, 
        kafka_installation_path: &str, 
        discovery_broker: &str,
        tools_dir: &PathBuf,
        kafkactl_data: &mut HashMap<String, String>
    ) -> Result<()> {
        print!("  • Getting topics (kafka-tools)... ");
        
        let hostname = discovery_broker.split(':').next().unwrap_or(discovery_broker);
        let kafka_topics_cmd = format!(
            "ssh -o StrictHostKeyChecking=no {} '{}/kafka-topics.sh --bootstrap-server localhost:9092 --describe'",
            hostname,
            kafka_installation_path
        );
        
        println!();
        println!("     🔍 Command: ssh {} '{}/kafka-topics.sh --bootstrap-server localhost:9092 --describe'", hostname, kafka_installation_path);
        
        if let Ok(topics_output) = self.run_on_bastion(&kafka_topics_cmd) {
            let topic_count = topics_output.lines()
                .filter(|line| line.starts_with("Topic:"))
                .count();
            
            fs::write(tools_dir.join("topics_kafka_tools.txt"), &topics_output)?;
            kafkactl_data.insert("topics_kafka_tools".to_string(), topics_output.clone());
            
            // Also get list format for compatibility
            let list_cmd = format!(
                "ssh -o StrictHostKeyChecking=no {} '{}/kafka-topics.sh --bootstrap-server localhost:9092 --list'",
                hostname,
                kafka_installation_path
            );
            
            if let Ok(topics_list) = self.run_on_bastion(&list_cmd) {
                let list_topic_count = topics_list.lines().filter(|line| !line.trim().is_empty()).count();
                fs::write(tools_dir.join("topics_list.txt"), &topics_list)?;
                kafkactl_data.insert("topics_list".to_string(), topics_list);
                println!("     ✅ Successfully collected {} topics using kafka-tools.sh", list_topic_count);
            }
            
            println!("     📝 Detailed topic info: {} topic descriptions", topic_count);
            println!("     💾 Saved to: topics_kafka_tools.txt, topics_list.txt");
        } else {
            println!("     ❌ kafka-topics.sh command failed");
            println!("     🔄 Falling back to kafkactl method...");
            let kafkactl_dir = self.output_dir.join("cluster").join("kafkactl");
            self.collect_topics_with_kafkactl(&kafkactl_dir, kafkactl_data)?;
        }
        
        Ok(())
    }

    /// Collect topics using kafkactl (fallback method)
    fn collect_topics_with_kafkactl(
        &self,
        kafkactl_dir: &PathBuf,
        kafkactl_data: &mut HashMap<String, String>
    ) -> Result<()> {
        print!("  • Getting topics (kafkactl)... ");
        println!();
        println!("     🔍 Command: kafkactl get topics -o yaml");
        
        if let Ok(topics) = self.run_on_bastion("kafkactl get topics -o yaml") {
            let topic_count = topics.lines()
                .filter(|line| line.trim().starts_with("- name:"))
                .count();
            
            fs::write(kafkactl_dir.join("topics.yaml"), &topics)?;
            kafkactl_data.insert("topics".to_string(), topics.clone());
            println!("     ✅ Successfully collected {} topics using kafkactl", topic_count);
            
            // Get topic details - extract topic names from YAML and get detailed info
            print!("     🔍 Getting detailed topic descriptions... ");
            let mut topics_detailed = String::new();
            let mut detailed_count = 0;
            
            // Parse topic names from YAML output (simplified - assumes standard kafkactl YAML format)
            for line in topics.lines() {
                if line.trim().starts_with("- name:") {
                    if let Some(topic_name) = line.split(':').nth(1) {
                        let topic = topic_name.trim().trim_matches('"');
                        if !topic.is_empty() {
                            if let Ok(details) = self.run_on_bastion(&format!("kafkactl describe topic {} -o yaml", topic)) {
                                topics_detailed.push_str(&format!("---\n# Topic: {}\n", topic));
                                topics_detailed.push_str(&details);
                                topics_detailed.push_str("\n\n");
                                detailed_count += 1;
                            }
                        }
                    }
                }
            }
            fs::write(kafkactl_dir.join("topics_detailed.yaml"), &topics_detailed)?;
            kafkactl_data.insert("topics_detailed".to_string(), topics_detailed);
            println!("✅ ({} detailed descriptions)", detailed_count);
            println!("     💾 Saved to: topics.yaml, topics_detailed.yaml");
        } else {
            println!("     ❌ kafkactl command failed");
            println!("     ⚠️  No topic data could be collected");
        }
        
        Ok(())
    }
}

/// Collector for individual broker data
pub struct BrokerCollector {
    bastion_alias: Option<String>,  // None means we're running locally on the bastion
    broker: BrokerInfo,
    output_dir: PathBuf,
}

impl BrokerCollector {
    pub fn new(bastion_alias: Option<String>, broker: BrokerInfo, output_dir: PathBuf) -> Self {
        Self {
            bastion_alias,
            broker,
            output_dir,
        }
    }
    
    /// Execute command on broker through bastion (using agent forwarding)
    fn run_on_broker(&self, command: &str) -> Result<String> {
        let ssh_command = format!(
            "ssh -o StrictHostKeyChecking=no {} '{}'",
            self.broker.hostname,
            command
        );
        
        let output = match &self.bastion_alias {
            Some(alias) => {
                // Remote bastion: SSH to bastion, then SSH to broker
                Command::new("ssh")
                    .arg("-A")  // Agent forwarding
                    .arg(alias)
                    .arg(ssh_command)
                    .output()
                    .context(format!("Failed to execute on broker {} via bastion: {}", self.broker.id, command))?
            }
            None => {
                // Local bastion: SSH directly to broker
                Command::new("sh")
                    .arg("-c")
                    .arg(&ssh_command)
                    .output()
                    .context(format!("Failed to execute on broker {}: {}", self.broker.id, command))?
            }
        };
        
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
    
    /// Extract Kafka config path from process command line
    fn extract_config_from_process(&self, ps_output: &str) -> Option<String> {
        // Look for common patterns in Kafka startup commands
        let line = ps_output.lines().next()?;
        
        // Pattern 1: kafka-server-start.sh /path/to/server.properties
        if let Some(server_start_pos) = line.find("kafka-server-start") {
            let after_start = &line[server_start_pos..];
            let parts: Vec<&str> = after_start.split_whitespace().collect();
            for (i, part) in parts.iter().enumerate() {
                if part.ends_with("server.properties") && i > 0 {
                    return Some(part.to_string());
                }
            }
        }
        
        // Pattern 2: Direct java invocation with config file
        let parts: Vec<&str> = line.split_whitespace().collect();
        for (i, part) in parts.iter().enumerate() {
            if part.ends_with("server.properties") && i > 0 {
                return Some(part.to_string());
            }
        }
        
        // Pattern 3: Look for --override or config parameters
        for (i, part) in parts.iter().enumerate() {
            if *part == "--override" && i + 1 < parts.len() {
                let next_part = parts[i + 1];
                if next_part.contains("server.properties") || next_part.starts_with("config") {
                    // Extract the config file path
                    if let Some(eq_pos) = next_part.find('=') {
                        return Some(next_part[eq_pos + 1..].to_string());
                    }
                }
            }
        }
        
        None
    }

    /// Enhanced config discovery - parse process arguments to get actual runtime config files
    async fn collect_configs_enhanced_discovery(&self, ps_output: &str) -> Result<HashMap<String, (String, String)>> {
        let mut configs = HashMap::new();
        
        // Parse the process command line like enhanced log discovery does
        let line = ps_output.lines().next().ok_or_else(|| anyhow::anyhow!("No process output"))?;
        let parts: Vec<&str> = line.split_whitespace().collect();
        
        // Extract server.properties path from command line arguments
        for part in parts.iter() {
            // Look for server.properties file arguments
            if part.ends_with("server.properties") {
                if let Ok(content) = self.run_on_broker(&format!("sudo cat '{}' 2>/dev/null", part)) {
                    if !content.is_empty() && !content.contains("No such file") {
                        configs.insert("server.properties".to_string(), (content, part.to_string()));
                    }
                }
                break;
            }
        }
        
        // Extract log4j configuration path from JVM arguments
        for part in parts {
            if part.contains("log4j.configuration=") {
                if let Some(eq_pos) = part.find('=') {
                    let log4j_path = &part[eq_pos + 1..];
                    let clean_path = log4j_path.strip_prefix("file:").unwrap_or(log4j_path);
                    
                    if let Ok(content) = self.run_on_broker(&format!("sudo cat '{}' 2>/dev/null", clean_path)) {
                        if !content.is_empty() && !content.contains("No such file") {
                            configs.insert("log4j.properties".to_string(), (content, clean_path.to_string()));
                        }
                    }
                }
            }
            if part.contains("log4j2.configurationFile=") {
                if let Some(eq_pos) = part.find('=') {
                    let log4j_path = &part[eq_pos + 1..];
                    
                    if let Ok(content) = self.run_on_broker(&format!("sudo cat '{}' 2>/dev/null", log4j_path)) {
                        if !content.is_empty() && !content.contains("No such file") {
                            configs.insert("log4j2.xml".to_string(), (content, log4j_path.to_string()));
                        }
                    }
                }
            }
        }
        
        // Get systemd service information - extract PID first
        if let Some(pid_str) = self.extract_pid_from_ps_output(ps_output) {
            if let Ok(pid) = pid_str.parse::<u32>() {
                // Get systemd service name using the PID
                if let Ok(systemctl_output) = self.run_on_broker(&format!("systemctl status {} 2>/dev/null", pid)) {
                    if let Some(service_name) = self.extract_service_name_from_systemctl(&systemctl_output) {
                        // Get the full service configuration
                        if let Ok(service_content) = self.run_on_broker(&format!("systemctl cat {} 2>/dev/null", service_name)) {
                            configs.insert("kafka.service".to_string(), (service_content, service_name.clone()));
                        }
                        
                        // Try to get environment file if mentioned in service
                        if let Ok(service_cat_output) = self.run_on_broker(&format!("systemctl cat {} 2>/dev/null", service_name)) {
                            if let Some(env_file_path) = self.extract_environment_file_from_service(&service_cat_output) {
                                if let Ok(env_content) = self.run_on_broker(&format!("sudo cat '{}' 2>/dev/null", env_file_path)) {
                                    if !env_content.is_empty() && !env_content.contains("No such file") {
                                        configs.insert("kafka.env".to_string(), (env_content, env_file_path));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Ok(configs)
    }

    /// Extract PID from ps aux output
    fn extract_pid_from_ps_output(&self, ps_output: &str) -> Option<String> {
        let line = ps_output.lines().next()?;
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            Some(parts[1].to_string()) // PID is second column in ps aux
        } else {
            None
        }
    }

    /// Extract systemd service name from systemctl status output
    fn extract_service_name_from_systemctl(&self, systemctl_output: &str) -> Option<String> {
        for line in systemctl_output.lines() {
            if line.contains("Loaded:") {
                // Look for service file path in Loaded line
                if let Some(start) = line.find('/') {
                    if let Some(end) = line[start..].find(';') {
                        let service_path = &line[start..start + end];
                        if let Some(filename) = service_path.split('/').last() {
                            return Some(filename.to_string());
                        }
                    }
                }
            }
        }
        None
    }

    /// Extract environment file path from systemd service configuration
    fn extract_environment_file_from_service(&self, service_content: &str) -> Option<String> {
        for line in service_content.lines() {
            if line.trim().starts_with("EnvironmentFile=") {
                let env_file = line.trim().strip_prefix("EnvironmentFile=")?;
                return Some(env_file.trim_matches('"').to_string());
            }
        }
        None
    }
    
    /// Discover Kafka installation path using systemctl and ps aux methods
    async fn discover_kafka_installation_path(&self) -> Option<String> {
        println!("  🔍 Discovering Kafka installation path...");
        
        // Method 1: Try systemctl approach first
        if let Ok(ps_output) = self.run_on_broker("ps aux | grep -E 'kafka\\.Kafka[^a-zA-Z]' | grep -v grep") {
            if let Some(pid_str) = self.extract_pid_from_ps_output(&ps_output) {
                if let Ok(pid) = pid_str.parse::<u32>() {
                    if let Ok(systemctl_output) = self.run_on_broker(&format!("systemctl status {} 2>/dev/null", pid)) {
                        if let Some(service_name) = self.extract_service_name_from_systemctl(&systemctl_output) {
                            if let Ok(service_content) = self.run_on_broker(&format!("systemctl cat {} 2>/dev/null", service_name)) {
                                // Look for ExecStart line to extract kafka installation path
                                for line in service_content.lines() {
                                    if line.trim().starts_with("ExecStart=") {
                                        let exec_start = line.trim().strip_prefix("ExecStart=").unwrap_or("");
                                        // Look for kafka-server-start.sh pattern
                                        if exec_start.contains("kafka-server-start.sh") {
                                            if let Some(start_pos) = exec_start.find("kafka-server-start.sh") {
                                                let before_script = &exec_start[..start_pos];
                                                if let Some(bin_pos) = before_script.rfind("/bin/") {
                                                    let kafka_path = &before_script[..bin_pos + 4];
                                                    println!("    ✅ Found Kafka path via systemctl: {}", kafka_path);
                                                    return Some(kafka_path.to_string());
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // Method 2: Parse ps aux output directly for installation path
        if let Ok(ps_output) = self.run_on_broker("ps aux | grep kafka.Kafka | grep -v grep") {
            let line = ps_output.lines().next().unwrap_or("");
            
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
                                let kafka_path = format!("{}/bin", kafka_base);
                                println!("    ✅ Found Kafka path via ps aux classpath: {}", kafka_path);
                                return Some(kafka_path);
                            }
                        }
                    }
                }
            }
            
            // Alternative: Look for kafka-server-start.sh or similar script in the command line
            if line.contains("kafka-server-start") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                for part in parts {
                    if part.contains("kafka-server-start") && part.contains("/bin/") {
                        if let Some(bin_pos) = part.rfind("/bin/") {
                            let kafka_path = &part[..bin_pos + 4];
                            println!("    ✅ Found Kafka path via ps aux script: {}", kafka_path);
                            return Some(kafka_path.to_string());
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
                            println!("    ✅ Found Kafka path via logs directory: {}", kafka_path);
                            return Some(kafka_path);
                        }
                        // Alternative: /opt/kafka/logs -> /opt/kafka/bin
                        if logs_dir.contains("/logs") {
                            let base_path = logs_dir.trim_end_matches("/logs");
                            let kafka_path = format!("{}/bin", base_path);
                            println!("    ✅ Found Kafka path via logs directory (alt): {}", kafka_path);
                            return Some(kafka_path);
                        }
                    }
                }
            }
        }
        
        println!("    ⚠️  Could not discover Kafka installation path");
        None
    }
    
    /// Collect all data from this broker
    pub async fn collect_all(&self) -> Result<BrokerData> {
        let broker_dir = self.output_dir.join("brokers").join(format!("broker_{}", self.broker.id));
        
        // Create broker directories
        fs::create_dir_all(&broker_dir)?;
        fs::create_dir_all(broker_dir.join("configs"))?;
        fs::create_dir_all(broker_dir.join("logs"))?;
        fs::create_dir_all(broker_dir.join("metrics"))?;
        fs::create_dir_all(broker_dir.join("system"))?;
        fs::create_dir_all(broker_dir.join("data"))?;
        
        // Save broker info
        let broker_info_json = serde_json::json!({
            "id": self.broker.id,
            "hostname": self.broker.hostname,
            "datacenter": self.broker.datacenter,
            "collection_timestamp": chrono::Utc::now().to_rfc3339(),
        });
        fs::write(
            broker_dir.join("broker_info.json"),
            serde_json::to_string_pretty(&broker_info_json)?,
        )?;
        
        // 1. Discover Kafka installation path first (needed for broker discovery later)
        let kafka_installation_path = self.discover_kafka_installation_path().await;
        
        // Store the discovered path for later use
        if let Some(ref path) = kafka_installation_path {
            fs::write(
                broker_dir.join("system").join("kafka_installation_path.txt"),
                path,
            )?;
        }

        // 2. Collect system information
        print!("  📊 System info... ");
        let mut system_info = HashMap::new();
        
        // Add kafka installation path to system info if discovered
        if let Some(ref path) = kafka_installation_path {
            system_info.insert("kafka_installation_path".to_string(), path.clone());
        }
        
        let system_commands = vec![
            ("hostname", "hostname -f"),
            ("uptime", "uptime"),
            ("memory", "free -h"),
            ("disk", "df -h"),
            ("cpu", "cat /proc/cpuinfo | grep -E 'processor|model name' | head -20"),
            ("lscpu", "lscpu 2>/dev/null"),
            ("cpuinfo", "cat /proc/cpuinfo 2>/dev/null"),
        ];
        
        for (name, cmd) in system_commands {
            if let Ok(output) = self.run_on_broker(cmd) {
                fs::write(broker_dir.join("system").join(format!("{}.txt", name)), &output)?;
                system_info.insert(name.to_string(), output);
            }
        }
        println!("✓");
        
        // 3. Java/JVM information
        print!("  ☕ Java/JVM info... ");
        if let Ok(java_version) = self.run_on_broker("java -version 2>&1") {
            fs::write(broker_dir.join("system").join("java_version.txt"), &java_version)?;
            system_info.insert("java_version".to_string(), java_version);
        }
        
        if let Ok(kafka_process) = self.run_on_broker("ps aux | grep -E 'kafka\\.Kafka' | grep -v grep") {
            fs::write(broker_dir.join("system").join("kafka_process.txt"), &kafka_process)?;
            system_info.insert("kafka_process".to_string(), kafka_process);
        }
        
        // Try to get JVM stats
        if let Ok(kafka_pid) = self.run_on_broker("pgrep -f 'kafka\\.Kafka' | head -1") {
            let pid = kafka_pid.trim();
            if !pid.is_empty() {
                if let Ok(jstat) = self.run_on_broker(&format!("jstat -gc {} 2>/dev/null", pid)) {
                    fs::write(broker_dir.join("metrics").join("jstat_gc.txt"), &jstat)?;
                }
            }
        }
        println!("✓");
        
        // 4. Configuration files - Using enhanced discovery first, fallback to find
        print!("  📝 Configuration files (enhanced discovery)... ");
        let mut configs = HashMap::new();
        
        // Try enhanced discovery first - parse Kafka process for actual runtime config paths
        let mut server_props_found = false;
        let mut log4j_found = false;
        let mut service_found = false;
        
        if let Ok(ps_output) = self.run_on_broker("ps aux | grep -E 'kafka\\.Kafka[^a-zA-Z]' | grep -v grep") {
            let enhanced_configs = self.collect_configs_enhanced_discovery(&ps_output).await;
            
            if let Ok(enhanced_configs) = enhanced_configs {
                for (filename, (content, source)) in enhanced_configs {
                    fs::write(broker_dir.join("configs").join(&filename), &content)?;
                    configs.insert(filename.clone(), content);
                    configs.insert(format!("{}_source", filename.replace('.', "_")), format!("enhanced:{}", source));
                    
                    match filename.as_str() {
                        "server.properties" => server_props_found = true,
                        "log4j.properties" => log4j_found = true,
                        "kafka.service" => service_found = true,
                        _ => {}
                    }
                }
                
                if configs.len() > 0 {
                    println!("✅ Enhanced discovery found {} config files", configs.len());
                }
            }
        }
        
        // Fallback: Use find command for any missing config files
        if !server_props_found {
            println!("⚠️  Enhanced discovery failed for server.properties, falling back to find");
            let find_commands = vec![
                "find /etc -name 'server.properties' 2>/dev/null | head -1",
                "find /opt -name 'server.properties' 2>/dev/null | head -1", 
                "find /usr -name 'server.properties' 2>/dev/null | head -1",
                "find /home -name 'server.properties' 2>/dev/null | head -1",
            ];
            
            for find_cmd in find_commands {
                if let Ok(output) = self.run_on_broker(find_cmd) {
                    let config_path = output.trim();
                    if !config_path.is_empty() {
                        if let Ok(content) = self.run_on_broker(&format!("sudo cat '{}' 2>/dev/null", config_path)) {
                            if !content.is_empty() && !content.contains("No such file") {
                                fs::write(broker_dir.join("configs").join("server.properties"), &content)?;
                                configs.insert("server.properties".to_string(), content);
                                configs.insert("server_properties_source".to_string(), format!("fallback_find:{}", config_path));
                                server_props_found = true;
                                println!("✅ Found server.properties via find fallback: {}", config_path);
                                break;
                            }
                        }
                    }
                }
            }
        }
        
        // Method 3: Fallback to standard locations
        if !server_props_found {
            let standard_paths = vec![
                "/etc/kafka/server.properties",
                "/opt/kafka/config/server.properties",
                "/usr/local/kafka/config/server.properties",
                "/usr/hdp/current/kafka-broker/conf/server.properties",
                "/opt/confluent/etc/kafka/server.properties",
            ];
            
            for path in standard_paths {
                if let Ok(content) = self.run_on_broker(&format!("cat {} 2>/dev/null", path)) {
                    if !content.is_empty() && !content.contains("No such file") {
                        fs::write(broker_dir.join("configs").join("server.properties"), &content)?;
                        configs.insert("server.properties".to_string(), content);
                        configs.insert("config_source".to_string(), format!("standard:{}", path));
                        break;
                    }
                }
            }
        }
        
        // Fallback for log4j configuration if enhanced discovery missed it
        if !log4j_found {
            println!("⚠️  Enhanced discovery failed for log4j.properties, falling back to find");
            let log4j_locations = vec![
                "find /etc -name 'log4j*.properties' 2>/dev/null | head -1",
                "find /opt -name 'log4j*.properties' 2>/dev/null | head -1",
            ];
            
            for cmd in log4j_locations {
                if let Ok(output) = self.run_on_broker(cmd) {
                    let log4j_path = output.trim();
                    if !log4j_path.is_empty() && log4j_path != "log4j*.properties" {
                        if let Ok(content) = self.run_on_broker(&format!("sudo cat '{}' 2>/dev/null", log4j_path)) {
                            if !content.is_empty() && !content.contains("No such file") {
                                fs::write(broker_dir.join("configs").join("log4j.properties"), &content)?;
                                configs.insert("log4j.properties".to_string(), content);
                                configs.insert("log4j_properties_source".to_string(), format!("fallback_find:{}", log4j_path));
                                log4j_found = true;
                                println!("✅ Found log4j.properties via find fallback: {}", log4j_path);
                                break;
                            }
                        }
                    }
                }
            }
        }
        
        // Fallback for systemd service file if enhanced discovery missed it
        if !service_found {
            println!("⚠️  Enhanced discovery failed for kafka.service, falling back to standard paths");
            let service_names = vec!["kafka", "kafka.service", "confluent-kafka", "apache-kafka"];
            for service_name in service_names {
                let service_paths = vec![
                    format!("/etc/systemd/system/{}", service_name),
                    format!("/lib/systemd/system/{}", service_name),  
                    format!("/usr/lib/systemd/system/{}", service_name),
                ];
                
                for service_path in service_paths {
                    if let Ok(content) = self.run_on_broker(&format!("sudo cat '{}' 2>/dev/null", service_path)) {
                        if !content.is_empty() && !content.contains("No such file") {
                            fs::write(broker_dir.join("configs").join("kafka.service"), &content)?;
                            configs.insert("kafka.service".to_string(), content);
                            configs.insert("kafka_service_source".to_string(), format!("fallback_standard:{}", service_path));
                            service_found = true;
                            println!("✅ Found kafka.service via standard paths: {}", service_path);
                            break;
                        }
                    }
                }
                if service_found {
                    break;
                }
            }
        }
        
        let enhanced_count = configs.values().filter(|v| v.contains("enhanced:")).count();
        let fallback_count = configs.len() - enhanced_count;
        
        if enhanced_count > 0 && fallback_count > 0 {
            println!("✓ (found {} config files: {} enhanced, {} fallback)", configs.len(), enhanced_count, fallback_count);
        } else if enhanced_count > 0 {
            println!("✓ (found {} config files via enhanced discovery)", configs.len());
        } else {
            println!("✓ (found {} config files via fallback methods)", configs.len());
        }
        
        // 5. Log files - Using enhanced discovery (process → systemd → config → logs)
        print!("  📜 Log files (enhanced discovery)... ");
        let mut logs = HashMap::new();
        
        // Initialize enhanced log discovery with the current SSH setup
        let ssh_target = if let Some(bastion) = &self.bastion_alias {
            Some(format!("{} ssh -o StrictHostKeyChecking=no {}", bastion, self.broker.hostname))
        } else {
            Some(self.broker.hostname.clone())
        };
        
        let enhanced_discovery = EnhancedLogDiscovery::new(ssh_target.clone());
        
        // Run the enhanced discovery chain
        match enhanced_discovery.discover_logs().await {
            Ok(discovery_result) => {
                // Save comprehensive discovery metadata
                let discovery_json = serde_json::to_string_pretty(&discovery_result)?;
                fs::write(broker_dir.join("logs").join("enhanced_discovery_metadata.json"), discovery_json)?;
                
                // Use discovered logs
                logs = discovery_result.discovered_logs;
                
                // Write individual log files
                for (log_name, log_content) in &logs {
                    if !log_content.trim().is_empty() {
                        // Create safe filename
                        let safe_name = log_name
                            .replace('/', "_")
                            .replace(' ', "_")
                            .trim_start_matches('_')
                            .to_string();
                        let final_name = if safe_name.is_empty() {
                            "unknown.log".to_string()
                        } else if safe_name.len() > 100 {
                            format!("{}.log", &safe_name[..97])
                        } else {
                            safe_name
                        };
                        
                        fs::write(broker_dir.join("logs").join(&final_name), log_content)?;
                    }
                }
                
                if logs.is_empty() {
                    // Ultimate fallback to basic journald
                    if let Ok(content) = self.run_on_broker("journalctl -n 500 --no-pager 2>/dev/null | grep -i kafka") {
                        if !content.is_empty() {
                            fs::write(broker_dir.join("logs").join("system_journald.log"), &content)?;
                            logs.insert("system_journald_fallback".to_string(), content);
                        }
                    }
                }
                
                // Report discovery success
                let steps = discovery_result.discovery_steps.len();
                let warnings = discovery_result.warnings.len();
                if warnings > 0 {
                    println!("✓ ({} logs, {} steps, {} warnings)", logs.len(), steps, warnings);
                } else {
                    println!("✓ ({} logs, {} discovery steps)", logs.len(), steps);
                }
            }
            Err(e) => {
                println!("⚠️ (enhanced discovery failed: {}, using legacy fallback)", e);
                
                // Fallback to the previous dynamic discovery method
                let fallback_discovery = LogDiscovery::new(ssh_target);
                match fallback_discovery.discover_logs().await {
                    Ok(discovered_logs) => {
                        match fallback_discovery.collect_log_contents(&discovered_logs, 500).await {
                            Ok(log_contents) => {
                                for (log_path, content) in log_contents {
                                    if !content.trim().is_empty() {
                                        let safe_name = log_path.replace('/', "_").trim_start_matches('_').to_string();
                                        fs::write(broker_dir.join("logs").join(&safe_name), &content)?;
                                        logs.insert(log_path, content);
                                    }
                                }
                            }
                            Err(_) => {
                                // Final fallback to hardcoded paths
                                let hardcoded_commands = vec![
                                    ("server.log", "tail -500 /var/log/kafka/server.log 2>/dev/null"),
                                    ("controller.log", "tail -500 /var/log/kafka/controller.log 2>/dev/null"),
                                    ("journald.log", "journalctl -u kafka -n 500 --no-pager 2>/dev/null"),
                                ];
                                
                                for (name, cmd) in hardcoded_commands {
                                    if let Ok(content) = self.run_on_broker(cmd) {
                                        if !content.is_empty() {
                                            fs::write(broker_dir.join("logs").join(name), &content)?;
                                            logs.insert(format!("hardcoded_{}", name), content);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(_) => {
                        // Final system-wide journald fallback
                        if let Ok(content) = self.run_on_broker("journalctl -n 500 --no-pager 2>/dev/null | grep -i kafka") {
                            if !content.is_empty() {
                                fs::write(broker_dir.join("logs").join("system_kafka_logs.log"), &content)?;
                                logs.insert("system_kafka_logs".to_string(), content);
                            }
                        }
                    }
                }
            }
        }
        
        // 6. Data directories
        print!("  💾 Data directories... ");
        let mut data_dirs = Vec::new();
        
        if let Ok(log_dirs_config) = self.run_on_broker(
            "grep '^log.dirs' /etc/kafka/server.properties /opt/kafka/config/server.properties 2>/dev/null | cut -d= -f2 | head -1"
        ) {
            let log_dirs = log_dirs_config.trim();
            if !log_dirs.is_empty() {
                fs::write(broker_dir.join("data").join("log_dirs.txt"), log_dirs)?;
                
                // Get size of each directory
                let mut dir_sizes = String::new();
                for dir in log_dirs.split(',') {
                    let dir = dir.trim();
                    if let Ok(size) = self.run_on_broker(&format!("du -sh {} 2>/dev/null", dir)) {
                        dir_sizes.push_str(&size);
                        data_dirs.push(dir.to_string());
                    }
                }
                
                if !dir_sizes.is_empty() {
                    fs::write(broker_dir.join("data").join("directory_sizes.txt"), dir_sizes)?;
                }
            }
        }
        println!("✓");
        
        // 7. Network information
        print!("  🌐 Network info... ");
        if let Ok(network) = self.run_on_broker(
            "netstat -tuln 2>/dev/null | grep -E '9092|9093|9094' || ss -tuln | grep -E '9092|9093|9094'"
        ) {
            fs::write(broker_dir.join("system").join("network.txt"), &network)?;
            system_info.insert("network".to_string(), network);
        }
        println!("✓");
        
        Ok(BrokerData {
            broker_id: self.broker.id,
            hostname: self.broker.hostname.clone(),
            datacenter: self.broker.datacenter.clone(),
            accessible: true,
            system_info,
            configs,
            logs,
            data_dirs,
        })
    }
}