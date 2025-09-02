use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use super::{BrokerData, BrokerInfo, ClusterData};
use super::log_discovery::LogDiscovery;
use super::enhanced_log_discovery::EnhancedLogDiscovery;

/// Collector for bastion-level data (kafkactl, metrics, etc.)
pub struct BastionCollector {
    bastion_alias: Option<String>,  // None means we're running locally on the bastion
    output_dir: PathBuf,
}

impl BastionCollector {
    pub fn new(bastion_alias: Option<String>, output_dir: PathBuf) -> Self {
        Self {
            bastion_alias,
            output_dir,
        }
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
        
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
    
    /// Collect all bastion-level data
    pub async fn collect_all(&self) -> Result<ClusterData> {
        let location = match &self.bastion_alias {
            Some(alias) => format!("from bastion '{}'", alias),
            None => "locally (running on bastion)".to_string(),
        };
        println!("📊 Collecting kafkactl data {}...", location);
        
        let mut kafkactl_data = HashMap::new();
        let kafkactl_dir = self.output_dir.join("cluster").join("kafkactl");
        
        // Get broker list
        print!("  • Getting broker list... ");
        let brokers = self.run_on_bastion("kafkactl get brokers")?;
        fs::write(kafkactl_dir.join("brokers.txt"), &brokers)?;
        kafkactl_data.insert("brokers".to_string(), brokers);
        println!("✓");
        
        // Get topics
        print!("  • Getting topics... ");
        let topics = self.run_on_bastion("kafkactl get topics")?;
        fs::write(kafkactl_dir.join("topics.txt"), &topics)?;
        kafkactl_data.insert("topics".to_string(), topics.clone());
        println!("✓");
        
        // Get topic details
        print!("  • Getting topic details... ");
        let mut topics_detailed = String::new();
        for line in topics.lines() {
            let topic = line.split_whitespace().next().unwrap_or("");
            if !topic.is_empty() && topic != "TOPIC" {
                if let Ok(details) = self.run_on_bastion(&format!("kafkactl describe topic {}", topic)) {
                    topics_detailed.push_str(&details);
                    topics_detailed.push_str("\n\n");
                }
            }
        }
        fs::write(kafkactl_dir.join("topics_detailed.txt"), &topics_detailed)?;
        kafkactl_data.insert("topics_detailed".to_string(), topics_detailed);
        println!("✓");
        
        // Get consumer groups
        print!("  • Getting consumer groups... ");
        let consumer_groups = self.run_on_bastion("kafkactl get consumer-groups")?;
        fs::write(kafkactl_dir.join("consumer_groups.txt"), &consumer_groups)?;
        kafkactl_data.insert("consumer_groups".to_string(), consumer_groups);
        println!("✓");
        
        // Get individual broker configs
        println!("  • Getting broker configurations:");
        for broker_id in [11, 12, 13, 14, 15, 16] {
            print!("    - Broker {}... ", broker_id);
            if let Ok(config) = self.run_on_bastion(&format!("kafkactl describe broker {}", broker_id)) {
                fs::write(
                    kafkactl_dir.join(format!("broker_{}_config.txt", broker_id)),
                    &config,
                )?;
                kafkactl_data.insert(format!("broker_{}_config", broker_id), config);
            }
            println!("✓");
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
        
        // 1. Collect system information
        print!("  📊 System info... ");
        let mut system_info = HashMap::new();
        
        let system_commands = vec![
            ("hostname", "hostname -f"),
            ("uptime", "uptime"),
            ("memory", "free -h"),
            ("disk", "df -h"),
            ("cpu", "cat /proc/cpuinfo | grep -E 'processor|model name' | head -20"),
        ];
        
        for (name, cmd) in system_commands {
            if let Ok(output) = self.run_on_broker(cmd) {
                fs::write(broker_dir.join("system").join(format!("{}.txt", name)), &output)?;
                system_info.insert(name.to_string(), output);
            }
        }
        println!("✓");
        
        // 2. Java/JVM information
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
        
        // 3. Configuration files - Using enhanced discovery first, fallback to find
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
        
        // 4. Log files - Using enhanced discovery (process → systemd → config → logs)
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
        
        // 5. Data directories
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
        
        // 6. Network information
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