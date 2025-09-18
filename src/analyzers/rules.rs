use super::{Analyzer, AnalyzerError, AnalyzerResult};
use crate::collectors::admin::AdminCollectorOutput;
use crate::snapshot::format::{
    Category, Evidence, Finding, LogEvidence, MetricEvidence, 
    Remediation, RemediationStep, RiskLevel, Severity, Snapshot
};
use async_trait::async_trait;
use tracing::{debug, info};

/// Rule-based analyzer for deterministic health checks
pub struct RuleAnalyzer {
    enabled_rules: Vec<Box<dyn Rule>>,
}

impl Default for RuleAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl RuleAnalyzer {
    pub fn new() -> Self {
        Self {
            enabled_rules: Self::default_rules(),
        }
    }
    
    fn default_rules() -> Vec<Box<dyn Rule>> {
        vec![
            Box::new(UnderReplicatedPartitionsRule),
            Box::new(OfflinePartitionsRule),
            Box::new(LeaderImbalanceRule),
            Box::new(IsrShrinkageRule),
            Box::new(HighErrorRateRule),
        ]
    }
}

#[async_trait]
impl Analyzer for RuleAnalyzer {
    async fn analyze(&self, snapshot: &Snapshot) -> AnalyzerResult<Vec<Finding>> {
        info!("Running rule-based analysis");
        
        let mut findings = Vec::new();
        
        for rule in &self.enabled_rules {
            match rule.evaluate(snapshot) {
                Ok(Some(finding)) => {
                    debug!("Rule {} produced finding: {}", rule.name(), finding.title);
                    findings.push(finding);
                }
                Ok(None) => {
                    debug!("Rule {} found no issues", rule.name());
                }
                Err(e) => {
                    tracing::warn!("Rule {} failed: {}", rule.name(), e);
                }
            }
        }
        
        info!("Rule analysis completed with {} findings", findings.len());
        Ok(findings)
    }
    
    fn name(&self) -> &'static str {
        "rule_analyzer"
    }
    
    fn description(&self) -> &'static str {
        "Deterministic rule-based health checks"
    }
}

/// Trait for individual rules
trait Rule: Send + Sync {
    fn evaluate(&self, snapshot: &Snapshot) -> AnalyzerResult<Option<Finding>>;
    fn name(&self) -> &'static str;
}

/// Rule: Check for under-replicated partitions
struct UnderReplicatedPartitionsRule;

impl Rule for UnderReplicatedPartitionsRule {
    fn evaluate(&self, snapshot: &Snapshot) -> AnalyzerResult<Option<Finding>> {
        // Extract admin data
        let admin_data = snapshot.collectors.admin.as_ref()
            .ok_or_else(|| AnalyzerError::InvalidData("No admin data available".to_string()))?;
        
        let admin: AdminCollectorOutput = serde_json::from_value(admin_data.clone())
            .map_err(|e| AnalyzerError::InvalidData(format!("Failed to parse admin data: {}", e)))?;
        
        let mut under_replicated = Vec::new();
        
        for topic in &admin.topics {
            for partition in &topic.partitions {
                if partition.isr.len() < partition.replicas.len() {
                    under_replicated.push((topic.name.clone(), partition.id));
                }
            }
        }
        
        if under_replicated.is_empty() {
            return Ok(None);
        }
        
        let finding = Finding {
            id: format!("FND-001-{}", uuid::Uuid::new_v4()),
            severity: Severity::High,
            category: Category::Availability,
            title: format!("Under-replicated partitions detected: {} affected", under_replicated.len()),
            description: format!(
                "Found {} partitions with ISR count less than replica count. This indicates potential data loss risk.",
                under_replicated.len()
            ),
            impact: "Reduced fault tolerance. If additional brokers fail, data loss may occur.".to_string(),
            evidence: Evidence {
                metrics: vec![MetricEvidence {
                    name: "under_replicated_partitions".to_string(),
                    value: under_replicated.len() as f64,
                    threshold: Some(0.0),
                    unit: Some("partitions".to_string()),
                    source: "admin".to_string(),
                    timestamp: snapshot.timestamp,
                }],
                logs: Vec::new(),
                configs: Vec::new(),
                raw_data: Some(serde_json::json!({
                    "affected_partitions": under_replicated
                })),
            },
            root_cause: Some("Possible causes: broker failures, network issues, disk problems, or high load".to_string()),
            remediation: Remediation {
                steps: vec![
                    RemediationStep {
                        order: 1,
                        description: "Check broker health and logs for errors".to_string(),
                        command: Some("kafka-broker-api-versions.sh --bootstrap-server localhost:9092".to_string()),
                        verification: Some("All brokers should respond".to_string()),
                        can_automate: true,
                    },
                    RemediationStep {
                        order: 2,
                        description: "Review and fix any disk space issues".to_string(),
                        command: Some("df -h /var/kafka-logs".to_string()),
                        verification: Some("Disk usage should be below 85%".to_string()),
                        can_automate: false,
                    },
                    RemediationStep {
                        order: 3,
                        description: "Force re-election if needed".to_string(),
                        command: Some("kafka-leader-election.sh --bootstrap-server localhost:9092 --election-type preferred --all-topic-partitions".to_string()),
                        verification: Some("Check ISR list again".to_string()),
                        can_automate: true,
                    },
                ],
                script: None,
                risk_level: RiskLevel::Medium,
                requires_downtime: false,
                estimated_duration_minutes: Some(15),
                rollback_plan: None,
            },
            metadata: std::collections::HashMap::new(),
        };
        
        Ok(Some(finding))
    }
    
    fn name(&self) -> &'static str {
        "under_replicated_partitions"
    }
}

/// Rule: Check for offline partitions
struct OfflinePartitionsRule;

impl Rule for OfflinePartitionsRule {
    fn evaluate(&self, snapshot: &Snapshot) -> AnalyzerResult<Option<Finding>> {
        let admin_data = snapshot.collectors.admin.as_ref()
            .ok_or_else(|| AnalyzerError::InvalidData("No admin data available".to_string()))?;
        
        let admin: AdminCollectorOutput = serde_json::from_value(admin_data.clone())
            .map_err(|e| AnalyzerError::InvalidData(format!("Failed to parse admin data: {}", e)))?;
        
        let mut offline = Vec::new();
        
        for topic in &admin.topics {
            for partition in &topic.partitions {
                if partition.leader.is_none() {
                    offline.push((topic.name.clone(), partition.id));
                }
            }
        }
        
        if offline.is_empty() {
            return Ok(None);
        }
        
        let finding = Finding {
            id: format!("FND-002-{}", uuid::Uuid::new_v4()),
            severity: Severity::Critical,
            category: Category::Availability,
            title: format!("Offline partitions detected: {} affected", offline.len()),
            description: format!(
                "Found {} partitions without a leader. These partitions are unavailable for reads and writes.",
                offline.len()
            ),
            impact: "Complete unavailability of affected partitions. Applications cannot produce or consume from these partitions.".to_string(),
            evidence: Evidence {
                metrics: vec![MetricEvidence {
                    name: "offline_partitions".to_string(),
                    value: offline.len() as f64,
                    threshold: Some(0.0),
                    unit: Some("partitions".to_string()),
                    source: "admin".to_string(),
                    timestamp: snapshot.timestamp,
                }],
                logs: Vec::new(),
                configs: Vec::new(),
                raw_data: Some(serde_json::json!({
                    "offline_partitions": offline
                })),
            },
            root_cause: Some("All replicas for these partitions are down or unreachable".to_string()),
            remediation: Remediation {
                steps: vec![
                    RemediationStep {
                        order: 1,
                        description: "Identify and restart failed brokers".to_string(),
                        command: Some("systemctl status kafka".to_string()),
                        verification: Some("All brokers should be running".to_string()),
                        can_automate: false,
                    },
                    RemediationStep {
                        order: 2,
                        description: "Check for corrupted log segments".to_string(),
                        command: Some("kafka-log-dirs.sh --describe --bootstrap-server localhost:9092".to_string()),
                        verification: Some("No corrupted segments".to_string()),
                        can_automate: true,
                    },
                    RemediationStep {
                        order: 3,
                        description: "Use unclean leader election as last resort".to_string(),
                        command: Some("kafka-leader-election.sh --bootstrap-server localhost:9092 --election-type unclean --topic <topic> --partition <partition>".to_string()),
                        verification: Some("Partition has a leader".to_string()),
                        can_automate: false,
                    },
                ],
                script: None,
                risk_level: RiskLevel::High,
                requires_downtime: false,
                estimated_duration_minutes: Some(30),
                rollback_plan: Some("If unclean election was used, verify data consistency".to_string()),
            },
            metadata: std::collections::HashMap::new(),
        };
        
        Ok(Some(finding))
    }
    
    fn name(&self) -> &'static str {
        "offline_partitions"
    }
}

/// Rule: Check for leader imbalance
struct LeaderImbalanceRule;

impl Rule for LeaderImbalanceRule {
    fn evaluate(&self, snapshot: &Snapshot) -> AnalyzerResult<Option<Finding>> {
        let admin_data = snapshot.collectors.admin.as_ref()
            .ok_or_else(|| AnalyzerError::InvalidData("No admin data available".to_string()))?;
        
        let admin: AdminCollectorOutput = serde_json::from_value(admin_data.clone())
            .map_err(|e| AnalyzerError::InvalidData(format!("Failed to parse admin data: {}", e)))?;
        
        // Count leaders per broker
        let mut leader_count: std::collections::HashMap<i32, usize> = std::collections::HashMap::new();
        
        for topic in &admin.topics {
            for partition in &topic.partitions {
                if let Some(leader) = partition.leader {
                    *leader_count.entry(leader).or_insert(0) += 1;
                }
            }
        }
        
        if leader_count.is_empty() {
            return Ok(None);
        }
        
        let max_leaders = leader_count.values().max().copied().unwrap_or(0);
        let min_leaders = leader_count.values().min().copied().unwrap_or(0);
        let avg_leaders = leader_count.values().sum::<usize>() / leader_count.len().max(1);
        
        // Check for imbalance (more than 20% difference from average)
        let imbalance_threshold = 0.2;
        let is_imbalanced = max_leaders as f64 > avg_leaders as f64 * (1.0 + imbalance_threshold);
        
        if !is_imbalanced {
            return Ok(None);
        }
        
        let finding = Finding {
            id: format!("FND-003-{}", uuid::Uuid::new_v4()),
            severity: Severity::Medium,
            category: Category::Performance,
            title: "Leader distribution imbalance detected".to_string(),
            description: format!(
                "Leader distribution is uneven across brokers. Max: {}, Min: {}, Avg: {}",
                max_leaders, min_leaders, avg_leaders
            ),
            impact: "Uneven load distribution may cause performance bottlenecks and increased latency on overloaded brokers".to_string(),
            evidence: Evidence {
                metrics: vec![
                    MetricEvidence {
                        name: "max_leaders_per_broker".to_string(),
                        value: max_leaders as f64,
                        threshold: Some(avg_leaders as f64 * 1.2),
                        unit: Some("partitions".to_string()),
                        source: "admin".to_string(),
                        timestamp: snapshot.timestamp,
                    },
                    MetricEvidence {
                        name: "min_leaders_per_broker".to_string(),
                        value: min_leaders as f64,
                        threshold: None,
                        unit: Some("partitions".to_string()),
                        source: "admin".to_string(),
                        timestamp: snapshot.timestamp,
                    },
                ],
                logs: Vec::new(),
                configs: Vec::new(),
                raw_data: Some(serde_json::json!({
                    "leader_distribution": leader_count
                })),
            },
            root_cause: Some("Preferred leader election not running or broker failures causing imbalance".to_string()),
            remediation: Remediation {
                steps: vec![
                    RemediationStep {
                        order: 1,
                        description: "Run preferred leader election".to_string(),
                        command: Some("kafka-leader-election.sh --bootstrap-server localhost:9092 --election-type preferred --all-topic-partitions".to_string()),
                        verification: Some("Check leader distribution again".to_string()),
                        can_automate: true,
                    },
                    RemediationStep {
                        order: 2,
                        description: "Enable automatic leader rebalancing".to_string(),
                        command: Some("Set auto.leader.rebalance.enable=true in server.properties".to_string()),
                        verification: Some("Config is applied".to_string()),
                        can_automate: false,
                    },
                ],
                script: None,
                risk_level: RiskLevel::Low,
                requires_downtime: false,
                estimated_duration_minutes: Some(5),
                rollback_plan: None,
            },
            metadata: std::collections::HashMap::new(),
        };
        
        Ok(Some(finding))
    }
    
    fn name(&self) -> &'static str {
        "leader_imbalance"
    }
}

/// Rule: Check for ISR shrinkage
struct IsrShrinkageRule;

impl Rule for IsrShrinkageRule {
    fn evaluate(&self, snapshot: &Snapshot) -> AnalyzerResult<Option<Finding>> {
        let admin_data = snapshot.collectors.admin.as_ref()
            .ok_or_else(|| AnalyzerError::InvalidData("No admin data available".to_string()))?;
        
        let admin: AdminCollectorOutput = serde_json::from_value(admin_data.clone())
            .map_err(|e| AnalyzerError::InvalidData(format!("Failed to parse admin data: {}", e)))?;
        
        let mut shrunk_isr = Vec::new();
        
        for topic in &admin.topics {
            for partition in &topic.partitions {
                // Check if ISR is less than 2 (risky for durability)
                if partition.isr.len() < 2 && partition.replicas.len() >= 2 {
                    shrunk_isr.push((topic.name.clone(), partition.id, partition.isr.len()));
                }
            }
        }
        
        if shrunk_isr.is_empty() {
            return Ok(None);
        }
        
        let finding = Finding {
            id: format!("FND-004-{}", uuid::Uuid::new_v4()),
            severity: Severity::High,
            category: Category::Availability,
            title: format!("ISR shrinkage detected: {} partitions at risk", shrunk_isr.len()),
            description: format!(
                "Found {} partitions with ISR count less than 2. This reduces durability guarantees.",
                shrunk_isr.len()
            ),
            impact: "Reduced durability. If the single ISR broker fails, data loss will occur with acks=all".to_string(),
            evidence: Evidence {
                metrics: vec![MetricEvidence {
                    name: "partitions_with_shrunk_isr".to_string(),
                    value: shrunk_isr.len() as f64,
                    threshold: Some(0.0),
                    unit: Some("partitions".to_string()),
                    source: "admin".to_string(),
                    timestamp: snapshot.timestamp,
                }],
                logs: Vec::new(),
                configs: Vec::new(),
                raw_data: Some(serde_json::json!({
                    "affected_partitions": shrunk_isr
                })),
            },
            root_cause: Some("Brokers falling behind on replication due to network, disk, or load issues".to_string()),
            remediation: Remediation {
                steps: vec![
                    RemediationStep {
                        order: 1,
                        description: "Check replica lag".to_string(),
                        command: Some("kafka-replica-verification.sh --broker-list localhost:9092 --topic-white-list '.*'".to_string()),
                        verification: Some("Identify lagging replicas".to_string()),
                        can_automate: true,
                    },
                    RemediationStep {
                        order: 2,
                        description: "Increase replica fetch settings if needed".to_string(),
                        command: Some("Update replica.fetch.max.bytes and replica.fetch.wait.max.ms".to_string()),
                        verification: Some("Replicas catching up".to_string()),
                        can_automate: false,
                    },
                    RemediationStep {
                        order: 3,
                        description: "Check network connectivity between brokers".to_string(),
                        command: Some("ping and network tests between brokers".to_string()),
                        verification: Some("Low latency and no packet loss".to_string()),
                        can_automate: false,
                    },
                ],
                script: None,
                risk_level: RiskLevel::Medium,
                requires_downtime: false,
                estimated_duration_minutes: Some(20),
                rollback_plan: None,
            },
            metadata: std::collections::HashMap::new(),
        };
        
        Ok(Some(finding))
    }
    
    fn name(&self) -> &'static str {
        "isr_shrinkage"
    }
}

/// Rule: Check for high error rate in logs
struct HighErrorRateRule;

impl Rule for HighErrorRateRule {
    fn evaluate(&self, snapshot: &Snapshot) -> AnalyzerResult<Option<Finding>> {
        let logs_data = snapshot.collectors.logs.as_ref();
        
        if logs_data.is_none() {
            return Ok(None);
        }
        
        let logs: crate::collectors::logs::LogCollectorOutput = serde_json::from_value(logs_data.unwrap().clone())
            .map_err(|e| AnalyzerError::InvalidData(format!("Failed to parse logs data: {}", e)))?;
        
        let error_rate_threshold = 10; // errors per minute threshold
        let total_errors = logs.summary.error_count;
        
        if total_errors < error_rate_threshold {
            return Ok(None);
        }
        
        let finding = Finding {
            id: format!("FND-005-{}", uuid::Uuid::new_v4()),
            severity: Severity::High,
            category: Category::Performance,
            title: format!("High error rate in logs: {} errors found", total_errors),
            description: format!(
                "Detected {} errors in recent logs, which exceeds the threshold of {} errors",
                total_errors, error_rate_threshold
            ),
            impact: "High error rate indicates system instability and potential service degradation".to_string(),
            evidence: Evidence {
                metrics: vec![MetricEvidence {
                    name: "error_count".to_string(),
                    value: total_errors as f64,
                    threshold: Some(error_rate_threshold as f64),
                    unit: Some("errors".to_string()),
                    source: "logs".to_string(),
                    timestamp: snapshot.timestamp,
                }],
                logs: logs.summary.top_errors.iter().take(3).map(|e| LogEvidence {
                    level: "ERROR".to_string(),
                    message: e.sample_message.clone(),
                    source_file: "server.log".to_string(),
                    line_number: None,
                    timestamp: e.last_occurrence.clone(),
                    count: e.count,
                }).collect(),
                configs: Vec::new(),
                raw_data: None,
            },
            root_cause: Some("Review top error patterns to identify specific issues".to_string()),
            remediation: Remediation {
                steps: vec![
                    RemediationStep {
                        order: 1,
                        description: "Analyze top error patterns".to_string(),
                        command: Some("grep ERROR /var/log/kafka/server.log | head -50".to_string()),
                        verification: Some("Identify root cause".to_string()),
                        can_automate: true,
                    },
                    RemediationStep {
                        order: 2,
                        description: "Address specific errors based on patterns".to_string(),
                        command: None,
                        verification: Some("Error rate decreasing".to_string()),
                        can_automate: false,
                    },
                ],
                script: None,
                risk_level: RiskLevel::Medium,
                requires_downtime: false,
                estimated_duration_minutes: Some(30),
                rollback_plan: None,
            },
            metadata: std::collections::HashMap::new(),
        };
        
        Ok(Some(finding))
    }
    
    fn name(&self) -> &'static str {
        "high_error_rate"
    }
}
