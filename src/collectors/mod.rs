pub mod admin;
pub mod logs;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Result type for collector operations
pub type CollectorResult<T> = Result<T, CollectorError>;

/// Errors that can occur during collection
#[derive(Debug, thiserror::Error)]
pub enum CollectorError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    
    #[error("Collection timeout after {0} seconds")]
    Timeout(u64),
    
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),
    
    #[error("Kafka error: {0}")]
    KafkaError(#[from] rdkafka::error::KafkaError),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
    
    #[error("Other error: {0}")]
    Other(String),
}

/// Base trait for all collectors
#[async_trait]
pub trait Collector: Send + Sync {
    type Config: Send + Sync;
    type Output: Serialize + for<'de> Deserialize<'de> + Send + Sync;
    
    /// Collect data from the source
    async fn collect(&self, config: &Self::Config) -> CollectorResult<Self::Output>;
    
    /// Redact sensitive information from the output
    fn redact(&self, output: Self::Output) -> Self::Output;
    
    /// Get collector name
    fn name(&self) -> &'static str;
    
    /// Validate the configuration
    fn validate_config(&self, config: &Self::Config) -> CollectorResult<()>;
}

/// Common configuration for Kafka connections
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaConfig {
    pub bootstrap_servers: Vec<String>,
    pub security_protocol: String,
    pub sasl_mechanism: Option<String>,
    pub sasl_username: Option<String>,
    pub sasl_password: Option<String>,
    pub ssl_ca_cert: Option<String>,
    pub ssl_cert: Option<String>,
    pub ssl_key: Option<String>,
    pub timeout_secs: u64,
    pub client_id: String,
}

impl Default for KafkaConfig {
    fn default() -> Self {
        Self {
            bootstrap_servers: vec!["localhost:9092".to_string()],
            security_protocol: "PLAINTEXT".to_string(),
            sasl_mechanism: None,
            sasl_username: None,
            sasl_password: None,
            ssl_ca_cert: None,
            ssl_cert: None,
            ssl_key: None,
            timeout_secs: 30,
            client_id: "kcpilot".to_string(),
        }
    }
}

/// Context for collection operations
#[derive(Debug, Clone)]
pub struct CollectionContext {
    pub kafka_config: KafkaConfig,
    pub metadata: HashMap<String, String>,
    pub dry_run: bool,
}

/// Type alias for boxed collectors to simplify type signatures
pub type BoxedCollector = Box<dyn Collector<Config = KafkaConfig, Output = serde_json::Value>>;

/// Registry for collectors
pub struct CollectorRegistry {
    collectors: HashMap<String, BoxedCollector>,
}

impl Default for CollectorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl CollectorRegistry {
    pub fn new() -> Self {
        Self {
            collectors: HashMap::new(),
        }
    }
    
    pub fn register(&mut self, name: String, collector: BoxedCollector) {
        self.collectors.insert(name, collector);
    }
    
    pub fn get(&self, name: &str) -> Option<&BoxedCollector> {
        self.collectors.get(name)
    }
    
    pub fn list(&self) -> Vec<String> {
        self.collectors.keys().cloned().collect()
    }
}
