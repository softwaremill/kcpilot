// Module declarations
pub mod collector;
pub mod bastion_collector;
pub mod broker_collector;
pub mod log_discovery;
pub mod enhanced_log_discovery;
pub mod types;
pub mod scanner;
pub mod cluster_detection;
pub mod broker_discovery;
pub mod bastion;

// Re-export types for convenience
pub use types::{
    ScanConfig, BrokerInfo, ScanMetadata, ScanResult, 
    ClusterData, BrokerData, CollectionStats
};
pub use scanner::Scanner;
pub use cluster_detection::detect_cluster_mode;



