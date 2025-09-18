// Module declarations
pub mod collector;
pub mod log_discovery;
pub mod enhanced_log_discovery;
pub mod types;
pub mod utils;
pub mod scanner;
pub mod cluster_detection;
pub mod broker_discovery;
pub mod bastion;
#[cfg(test)]
mod test_log_discovery;

// Re-export types for convenience
pub use types::{
    ScanConfig, BrokerInfo, ScanMetadata, ScanResult, 
    ClusterData, BrokerData, CollectionStats
};
pub use utils::infer_datacenter;
pub use scanner::Scanner;
pub use cluster_detection::detect_cluster_mode;



