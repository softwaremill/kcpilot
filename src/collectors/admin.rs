use async_trait::async_trait;
use rdkafka::admin::AdminClient;
use rdkafka::client::DefaultClientContext;
use rdkafka::config::ClientConfig;
use rdkafka::metadata::Metadata;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{error, info};

use super::{Collector, CollectorError, CollectorResult, KafkaConfig};

/// Conversion constant for seconds to milliseconds
const MS_PER_SEC: u64 = 1000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminCollectorOutput {
    pub cluster: ClusterInfo,
    pub brokers: Vec<BrokerInfo>,
    pub topics: Vec<TopicInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterInfo {
    pub cluster_id: Option<String>,
    pub controller_id: Option<i32>,
    pub broker_count: usize,
    pub topic_count: usize,
    pub partition_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerInfo {
    pub id: i32,
    pub host: String,
    pub port: i32,
    pub rack: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicInfo {
    pub name: String,
    pub partitions: Vec<PartitionInfo>,
    pub replication_factor: i16,
    pub is_internal: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionInfo {
    pub id: i32,
    pub leader: Option<i32>,
    pub replicas: Vec<i32>,
    pub isr: Vec<i32>,
    pub offline_replicas: Vec<i32>,
}

pub struct AdminCollector {
    redact_sensitive: bool,
}

impl Default for AdminCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl AdminCollector {
    pub fn new() -> Self {
        Self {
            redact_sensitive: false,
        }
    }

    pub fn with_redaction(mut self, redact: bool) -> Self {
        self.redact_sensitive = redact;
        self
    }

    fn create_client_config(config: &KafkaConfig) -> ClientConfig {
        let mut client_config = ClientConfig::new();
        
        // Bootstrap servers
        client_config.set(
            "bootstrap.servers",
            config.bootstrap_servers.join(","),
        );
        
        // Security settings
        if config.security_protocol != "plaintext" {
            client_config.set("security.protocol", &config.security_protocol);
        }
        
        // SASL settings
        if let Some(ref mechanism) = config.sasl_mechanism {
            client_config.set("sasl.mechanism", mechanism);
            
            if let Some(ref username) = config.sasl_username {
                client_config.set("sasl.username", username);
            }
            
            if let Some(ref password) = config.sasl_password {
                client_config.set("sasl.password", password);
            }
        }
        
        // SSL settings
        if let Some(ref ca_cert) = config.ssl_ca_cert {
            client_config.set("ssl.ca.location", ca_cert);
        }
        
        if let Some(ref cert) = config.ssl_cert {
            client_config.set("ssl.certificate.location", cert);
        }
        
        if let Some(ref key) = config.ssl_key {
            client_config.set("ssl.key.location", key);
        }
        
        // Timeout settings
        client_config.set(
            "socket.timeout.ms",
            (config.timeout_secs * MS_PER_SEC).to_string(),
        );
        
        client_config
    }

    async fn collect_real(&self, config: &KafkaConfig) -> CollectorResult<AdminCollectorOutput> {
        let client_config = Self::create_client_config(config);
        
        // Create admin client
        let admin_client: AdminClient<DefaultClientContext> = client_config
            .create()
            .map_err(|e| CollectorError::ConnectionFailed(e.to_string()))?;
        
        // Fetch metadata
        let metadata = admin_client
            .inner()
            .fetch_metadata(None, Duration::from_secs(config.timeout_secs))
            .map_err(|e| CollectorError::ConnectionFailed(e.to_string()))?;
        
        // Parse metadata
        let output = self.parse_metadata(&metadata)?;
        
        Ok(output)
    }

    fn parse_metadata(&self, metadata: &Metadata) -> CollectorResult<AdminCollectorOutput> {
        // Parse brokers
        let brokers: Vec<BrokerInfo> = metadata
            .brokers()
            .iter()
            .map(|b| BrokerInfo {
                id: b.id(),
                host: b.host().to_string(),
                port: b.port(),
                rack: None, // Not available from metadata
            })
            .collect();
        
        // Parse topics
        let mut topics = Vec::new();
        let mut total_partitions = 0;
        
        for topic in metadata.topics() {
            let mut partitions = Vec::new();
            
            for p in topic.partitions() {
                total_partitions += 1;
                
                partitions.push(PartitionInfo {
                    id: p.id(),
                    leader: if p.leader() >= 0 { Some(p.leader()) } else { None },
                    replicas: p.replicas().to_vec(),
                    isr: p.isr().to_vec(),
                    offline_replicas: Vec::new(), // Would need to calculate from replica state
                });
            }
            
            topics.push(TopicInfo {
                name: topic.name().to_string(),
                partitions,
                replication_factor: topic.partitions().first()
                    .map(|p| p.replicas().len() as i16)
                    .unwrap_or(0),
                is_internal: topic.name().starts_with("__"),
            });
        }
        
        // Create cluster info
        let cluster = ClusterInfo {
            cluster_id: None, // Not directly available from metadata
            controller_id: None, // Would need to query controller endpoint
            broker_count: brokers.len(),
            topic_count: topics.len(),
            partition_count: total_partitions,
        };
        
        Ok(AdminCollectorOutput {
            cluster,
            brokers,
            topics,
        })
    }
}

#[async_trait]
impl Collector for AdminCollector {
    type Config = KafkaConfig;
    type Output = AdminCollectorOutput;

    async fn collect(&self, config: &Self::Config) -> CollectorResult<Self::Output> {
        info!("Starting admin data collection");
        
        if config.bootstrap_servers.is_empty() {
            return Err(CollectorError::ConfigurationError(
                "No bootstrap servers configured".to_string()
            ));
        }
        
        // Try to connect to the Kafka cluster with timeout
        let result = tokio::time::timeout(
            Duration::from_secs(config.timeout_secs),
            self.collect_real(config)
        ).await;

        match result {
            Ok(Ok(output)) => {
                info!("Successfully collected admin data from Kafka cluster");
                Ok(output)
            }
            Ok(Err(e)) => {
                error!("Failed to connect to Kafka cluster: {}", e);
                Err(e)
            }
            Err(_) => {
                error!("Connection timeout after {} seconds", config.timeout_secs);
                Err(CollectorError::ConnectionFailed(
                    format!("Connection timeout after {} seconds", config.timeout_secs)
                ))
            }
        }
    }

    fn redact(&self, mut output: Self::Output) -> Self::Output {
        if self.redact_sensitive {
            // Redact sensitive topic names if needed
            for topic in &mut output.topics {
                if topic.name.contains("password") || topic.name.contains("secret") {
                    topic.name = format!("REDACTED_{}", topic.name.len());
                }
            }
        }
        output
    }
    
    fn name(&self) -> &'static str {
        "AdminCollector"
    }
    
    fn validate_config(&self, config: &Self::Config) -> CollectorResult<()> {
        if config.bootstrap_servers.is_empty() {
            return Err(CollectorError::ConfigurationError(
                "Bootstrap servers cannot be empty".to_string()
            ));
        }
        
        // Validate timeout
        if config.timeout_secs == 0 {
            return Err(CollectorError::ConfigurationError(
                "Timeout must be greater than 0".to_string()
            ));
        }
        
        Ok(())
    }
}