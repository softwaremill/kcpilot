use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Configuration for scanning operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanConfig {
    pub bastion_alias: Option<String>,  // None means running locally on bastion
    pub output_dir: PathBuf,
    pub brokers: Vec<BrokerInfo>,
}

/// Information about a Kafka broker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerInfo {
    pub id: i32,
    pub hostname: String,
    pub datacenter: String,
}

/// Metadata about a scan operation
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

/// Result of a complete scan operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub metadata: ScanMetadata,
    pub cluster_data: ClusterData,
    pub broker_data: Vec<BrokerData>,
    pub collection_stats: CollectionStats,
    pub detected_cluster_mode: Option<crate::snapshot::format::ClusterMode>,
}

/// Cluster-level data collected during scan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterData {
    pub kafkactl_data: HashMap<String, String>,
    pub metrics: Option<String>,
    pub bastion_info: HashMap<String, String>,
}

/// Data collected from a specific broker
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

/// Statistics about the data collection process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionStats {
    pub total_files: usize,
    pub total_size_bytes: u64,
    pub duration_secs: u64,
}