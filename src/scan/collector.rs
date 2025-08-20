use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use super::{BrokerData, BrokerInfo, ClusterData};

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
        
        // 3. Configuration files
        print!("  📝 Configuration files... ");
        let mut configs = HashMap::new();
        
        let config_paths = vec![
            "/etc/kafka/server.properties",
            "/opt/kafka/config/server.properties",
            "/usr/hdp/current/kafka-broker/conf/server.properties",
        ];
        
        for path in config_paths {
            if let Ok(content) = self.run_on_broker(&format!("cat {} 2>/dev/null", path)) {
                if !content.is_empty() && !content.contains("No such file") {
                    fs::write(broker_dir.join("configs").join("server.properties"), &content)?;
                    configs.insert("server.properties".to_string(), content);
                    break;
                }
            }
        }
        
        // Collect other config files
        if let Ok(log4j) = self.run_on_broker("cat /etc/kafka/log4j.properties 2>/dev/null") {
            if !log4j.is_empty() && !log4j.contains("No such file") {
                fs::write(broker_dir.join("configs").join("log4j.properties"), &log4j)?;
                configs.insert("log4j.properties".to_string(), log4j);
            }
        }
        
        if let Ok(service) = self.run_on_broker("cat /etc/systemd/system/kafka.service 2>/dev/null") {
            if !service.is_empty() && !service.contains("No such file") {
                fs::write(broker_dir.join("configs").join("kafka.service"), &service)?;
                configs.insert("kafka.service".to_string(), service);
            }
        }
        println!("✓");
        
        // 4. Log files
        print!("  📜 Log files... ");
        let mut logs = HashMap::new();
        
        let log_commands = vec![
            ("server.log", "tail -500 /var/log/kafka/server.log 2>/dev/null"),
            ("controller.log", "tail -500 /var/log/kafka/controller.log 2>/dev/null"),
            ("journald.log", "journalctl -u kafka -n 500 --no-pager 2>/dev/null"),
        ];
        
        for (name, cmd) in log_commands {
            if let Ok(content) = self.run_on_broker(cmd) {
                if !content.is_empty() {
                    fs::write(broker_dir.join("logs").join(name), &content)?;
                    logs.insert(name.to_string(), content);
                }
            }
        }
        println!("✓");
        
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