use anyhow::Result;
use chrono::Utc;
use serde_json;
use std::fs;
use std::path::PathBuf;
use tracing::{error, info};
use crate::scan::collector::{BastionCollector, BrokerCollector, DiscoveryMethod};
use crate::scan::types::{
    ScanConfig, BrokerInfo, ScanMetadata, ScanResult, 
    ClusterData, BrokerData, CollectionStats
};
use crate::scan::cluster_detection::{parse_server_properties, is_kraft_mode, is_zookeeper_mode};
use crate::scan::broker_discovery::{
    discover_brokers_from_kafkactl, discover_brokers_from_single_local,
    discover_brokers_using_installation_path, discover_brokers_with_metadata_shell,
    discover_brokers_with_api_versions, discover_brokers_from_configs
};
use crate::scan::bastion::{
    check_ssh_agent, run_ssh_diagnostics, test_broker_access, 
    check_kafkactl_availability, discover_brokers_with_bastion_admin_client
};

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
        self.detected_cluster_mode = Some(mode);
        mode
    }
    
    /// Get the currently detected cluster mode
    pub fn get_cluster_mode(&self) -> Option<crate::snapshot::format::ClusterMode> {
        self.detected_cluster_mode
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
        let kafkactl_available = check_kafkactl_availability(self.config.bastion_alias.as_ref());
        
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
        
        let discovered_brokers = discover_brokers_from_kafkactl(self.config.bastion_alias.as_ref()).await?;
        self.config.brokers = discovered_brokers;
        
        // Set discovery method for topic collection
        self.discovery_method = Some(DiscoveryMethod::Kafkactl);
        Ok(self)
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
        let discovered_brokers = discover_brokers_from_single_local(broker_address).await?;
        self.config.brokers = discovered_brokers;
        Ok(self)
    }

    /// Discover brokers via SSH using installation path-based discovery
    async fn discover_brokers_via_ssh(mut self, broker_address: &str, bastion_alias: &str) -> Result<Self> {
        info!("Attempting broker discovery on bastion: {}", bastion_alias);
        info!("Note: kafkactl will be collected separately as data source, not used for broker discovery");
        
        // New Method: SSH to the broker, discover Kafka installation path, then discover all brokers
        if let Ok(brokers) = discover_brokers_using_installation_path(
            self.config.bastion_alias.as_ref(), 
            broker_address, 
            &self.config.output_dir
        ).await {
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
        if let Ok(brokers) = discover_brokers_with_bastion_admin_client(
            self.config.bastion_alias.as_ref(), 
            broker_address
        ).await {
            if !brokers.is_empty() {
                info!("Successfully discovered {} brokers using admin client on bastion", brokers.len());
                self.config.brokers = brokers;
                return Ok(self);
            }
        }
        
        // Method 2: Use kafka-metadata-shell.sh if available
        if let Ok(brokers) = discover_brokers_with_metadata_shell(
            self.config.bastion_alias.as_ref(), 
            broker_address
        ).await {
            if !brokers.is_empty() {
                info!("Successfully discovered {} brokers using kafka-metadata-shell", brokers.len());
                self.config.brokers = brokers;
                return Ok(self);
            }
        }
        
        // Method 3: Use kafka-broker-api-versions.sh with better parsing
        if let Ok(brokers) = discover_brokers_with_api_versions(
            self.config.bastion_alias.as_ref(), 
            broker_address
        ).await {
            if !brokers.is_empty() {
                info!("Successfully discovered {} brokers using kafka-broker-api-versions", brokers.len());
                self.config.brokers = brokers;
                return Ok(self);
            }
        }
        
        // Method 4: Parse server.properties files on brokers to find other brokers
        if let Ok(brokers) = discover_brokers_from_configs(
            self.config.bastion_alias.as_ref(), 
            broker_address
        ).await {
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
        }];
        
        self.config.brokers = fallback_brokers;
        Ok(self)
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
            cluster_mode: self.detected_cluster_mode,
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
                check_ssh_agent(self.config.bastion_alias.as_ref())?;
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
            run_ssh_diagnostics(alias, &self.config.brokers[0]).await;
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
            print!("  • Broker {}... ", broker.id);
            
            if test_broker_access(self.config.bastion_alias.as_ref(), broker).await {
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
                println!("🔍 Processing Broker {} ({})...", 
                    broker.id, broker.hostname);
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
                                self.detected_cluster_mode = Some(detected_mode);
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
                cluster_mode: self.detected_cluster_mode,
            },
            cluster_data,
            broker_data,
            collection_stats: stats,
            detected_cluster_mode: self.detected_cluster_mode,
        })
    }
    
    /// Generate summary report
    fn generate_summary_report(&self, _cluster_data: &ClusterData, broker_data: &[BrokerData]) -> Result<()> {
        let mut report = String::new();
        
        report.push_str("# Kafka Cluster Data Collection Summary\n\n");
        report.push_str("## Collection Details\n");
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
        for entry in walkdir::WalkDir::new(&self.config.output_dir).into_iter().flatten() {
            if entry.file_type().is_file() {
                total_files += 1;
                if let Ok(metadata) = entry.metadata() {
                    total_size += metadata.len();
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