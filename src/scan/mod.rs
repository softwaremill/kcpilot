use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tracing::{error, info};

pub mod collector;

use collector::{BastionCollector, BrokerCollector};

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub metadata: ScanMetadata,
    pub cluster_data: ClusterData,
    pub broker_data: Vec<BrokerData>,
    pub collection_stats: CollectionStats,
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
    config: ScanConfig,
}

impl Scanner {
    pub fn new(bastion_alias: Option<String>) -> Result<Self> {
        // Create output directory with timestamp
        let timestamp = Utc::now().format("%Y%m%d-%H%M%S");
        let output_dir = PathBuf::from(format!("kafka-scan-{}", timestamp));
        
        // Default broker configuration - can be made configurable
        let brokers = vec![
            BrokerInfo { id: 11, hostname: "kafka-poligon-dc1-1.c.bartek-rekke-sandbox.internal".to_string(), datacenter: "dc1".to_string() },
            BrokerInfo { id: 12, hostname: "kafka-poligon-dc1-2.c.bartek-rekke-sandbox.internal".to_string(), datacenter: "dc1".to_string() },
            BrokerInfo { id: 13, hostname: "kafka-poligon-dc2-1.c.bartek-rekke-sandbox.internal".to_string(), datacenter: "dc2".to_string() },
            BrokerInfo { id: 14, hostname: "kafka-poligon-dc2-2.c.bartek-rekke-sandbox.internal".to_string(), datacenter: "dc2".to_string() },
            BrokerInfo { id: 15, hostname: "kafka-poligon-dc3-1.c.bartek-rekke-sandbox.internal".to_string(), datacenter: "dc3".to_string() },
            BrokerInfo { id: 16, hostname: "kafka-poligon-dc3-2.c.bartek-rekke-sandbox.internal".to_string(), datacenter: "dc3".to_string() },
        ];
        
        Ok(Self {
            config: ScanConfig {
                bastion_alias,
                output_dir,
                brokers,
            },
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
        };
        
        let json = serde_json::to_string_pretty(&metadata)?;
        fs::write(
            self.config.output_dir.join("scan_metadata.json"),
            json,
        )?;
        
        Ok(())
    }
    
    /// Main scan execution
    pub async fn scan(&self) -> Result<ScanResult> {
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
        
        let bastion_collector = BastionCollector::new(
            self.config.bastion_alias.clone(),
            self.config.output_dir.clone(),
        );
        
        let cluster_data = bastion_collector.collect_all().await?;
        
        // Phase 3: Test broker connectivity
        println!("\n═══════════════════════════════════════════════════════════════");
        println!("PHASE 2: Testing Broker Connectivity");
        println!("═══════════════════════════════════════════════════════════════");
        println!();
        
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
            },
            cluster_data,
            broker_data,
            collection_stats: stats,
        })
    }
    
    /// Test if a broker is accessible via SSH
    async fn test_broker_access(&self, broker: &BrokerInfo) -> bool {
        match &self.config.bastion_alias {
            Some(alias) => {
                // Remote bastion: test via SSH chain
                let output = Command::new("ssh")
                    .arg("-A")
                    .arg(alias)
                    .arg(format!(
                        "ssh -o ConnectTimeout=3 -o StrictHostKeyChecking=no {} 'true'",
                        broker.hostname
                    ))
                    .output();
                
                match output {
                    Ok(result) => result.status.success(),
                    Err(_) => false,
                }
            }
            None => {
                // Local bastion: test direct SSH to broker
                let output = Command::new("ssh")
                    .arg("-o")
                    .arg("ConnectTimeout=3")
                    .arg("-o")
                    .arg("StrictHostKeyChecking=no")
                    .arg(&broker.hostname)
                    .arg("true")
                    .output();
                
                match output {
                    Ok(result) => result.status.success(),
                    Err(_) => false,
                }
            }
        }
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