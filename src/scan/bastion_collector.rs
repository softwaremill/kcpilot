use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use super::ClusterData;

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
        
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
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