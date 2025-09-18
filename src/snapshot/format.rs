use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Current snapshot format version
pub const SNAPSHOT_VERSION: &str = "1.0.0";

/// Main snapshot structure containing all collected data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub version: String,
    pub timestamp: DateTime<Utc>,
    pub metadata: SnapshotMetadata,
    pub cluster: ClusterSnapshot,
    pub collectors: CollectorOutputs,
    pub findings: Vec<Finding>,
}

impl Snapshot {
    pub fn new(metadata: SnapshotMetadata) -> Self {
        Self {
            version: SNAPSHOT_VERSION.to_string(),
            timestamp: Utc::now(),
            metadata,
            cluster: ClusterSnapshot::default(),
            collectors: CollectorOutputs::default(),
            findings: Vec::new(),
        }
    }
}

/// Metadata about the snapshot collection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    pub tool_version: String,
    pub collection_id: String,
    pub collection_duration_ms: u64,
    pub collectors_used: Vec<String>,
    pub redaction_applied: bool,
    pub environment: Option<String>,
    pub tags: HashMap<String, String>,
}

impl SnapshotMetadata {
    pub fn new(tool_version: String) -> Self {
        Self {
            tool_version,
            collection_id: uuid::Uuid::new_v4().to_string(),
            collection_duration_ms: 0,
            collectors_used: Vec::new(),
            redaction_applied: false,
            environment: None,
            tags: HashMap::new(),
        }
    }
}

/// Cluster-level snapshot data
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClusterSnapshot {
    pub id: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    pub mode: ClusterMode,
    pub cloud_provider: Option<String>,
    pub region: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ClusterMode {
    #[serde(rename = "kraft")]
    Kraft,
    #[serde(rename = "zookeeper")]
    Zookeeper,
    #[serde(rename = "unknown")]
    Unknown,
}

impl Default for ClusterMode {
    fn default() -> Self {
        Self::Unknown
    }
}

/// Container for all collector outputs
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CollectorOutputs {
    pub admin: Option<serde_json::Value>,
    pub logs: Option<serde_json::Value>,
    pub config: Option<serde_json::Value>,
    pub metrics: Option<serde_json::Value>,
    pub cloud: Option<serde_json::Value>,
    pub custom: HashMap<String, serde_json::Value>,
}

/// Finding from analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub id: String,
    pub severity: Severity,
    pub category: Category,
    pub title: String,
    pub description: String,
    pub impact: String,
    pub evidence: Evidence,
    pub root_cause: Option<String>,
    pub remediation: Remediation,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    #[serde(rename = "critical")]
    Critical,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "info")]
    Info,
}

impl Severity {
    pub fn color(&self) -> &'static str {
        match self {
            Severity::Critical => "red",
            Severity::High => "bright red",
            Severity::Medium => "yellow",
            Severity::Low => "bright yellow",
            Severity::Info => "blue",
        }
    }
    
    pub fn icon(&self) -> &'static str {
        match self {
            Severity::Critical => "🔴",
            Severity::High => "🟠",
            Severity::Medium => "🟡",
            Severity::Low => "🟢",
            Severity::Info => "ℹ️",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Category {
    #[serde(rename = "cluster_hygiene")]
    ClusterHygiene,
    #[serde(rename = "performance")]
    Performance,
    #[serde(rename = "configuration")]
    Configuration,
    #[serde(rename = "security")]
    Security,
    #[serde(rename = "availability")]
    Availability,
    #[serde(rename = "client")]
    Client,
    #[serde(rename = "capacity")]
    Capacity,
    #[serde(rename = "other")]
    Other,
}

/// Evidence supporting a finding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub metrics: Vec<MetricEvidence>,
    pub logs: Vec<LogEvidence>,
    pub configs: Vec<ConfigEvidence>,
    pub raw_data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricEvidence {
    pub name: String,
    pub value: f64,
    pub threshold: Option<f64>,
    pub unit: Option<String>,
    pub source: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEvidence {
    pub level: String,
    pub message: String,
    pub source_file: String,
    pub line_number: Option<usize>,
    pub timestamp: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigEvidence {
    pub resource_type: String,
    pub resource_name: String,
    pub config_key: String,
    pub current_value: String,
    pub recommended_value: Option<String>,
    pub reason: String,
    pub source_files: Vec<String>,  // Files where this config issue was found
}

/// Remediation information for a finding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Remediation {
    pub steps: Vec<RemediationStep>,
    pub script: Option<String>,
    pub risk_level: RiskLevel,
    pub requires_downtime: bool,
    pub estimated_duration_minutes: Option<u32>,
    pub rollback_plan: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemediationStep {
    pub order: u32,
    pub description: String,
    pub command: Option<String>,
    pub verification: Option<String>,
    pub can_automate: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum RiskLevel {
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
}

impl Default for Finding {
    fn default() -> Self {
        Self {
            id: String::new(),
            severity: Severity::Info,
            category: Category::Other,
            title: String::new(),
            description: String::new(),
            impact: String::new(),
            evidence: Evidence {
                metrics: Vec::new(),
                logs: Vec::new(),
                configs: Vec::new(),
                raw_data: None,
            },
            root_cause: None,
            remediation: Remediation {
                steps: Vec::new(),
                script: None,
                risk_level: RiskLevel::Low,
                requires_downtime: false,
                estimated_duration_minutes: None,
                rollback_plan: None,
            },
            metadata: HashMap::new(),
        }
    }
}
