use crate::analyzers::{Analyzer, AnalyzerResult};
use crate::snapshot::format::{
    Category, ConfigEvidence, Evidence, Finding, 
    Remediation, RemediationStep, RiskLevel, Severity, Snapshot
};
use async_trait::async_trait;
use std::collections::HashMap;

/// Configuration validator that checks for common Kafka configuration issues
pub struct ConfigValidator;

impl ConfigValidator {
    pub fn new() -> Self {
        Self
    }
    
    /// Check for broker ID configuration issues
    fn check_broker_id_issues(&self, snapshot: &Snapshot) -> Vec<Finding> {
        let mut findings = Vec::new();
        let mut config_broker_ids: HashMap<String, Vec<String>> = HashMap::new();
        let mut actual_broker_ids: HashMap<String, String> = HashMap::new();
        
        // Parse configuration files to find broker.id values
        if let Some(config_data) = &snapshot.collectors.config {
            if let Some(config_obj) = config_data.as_object() {
                for (file_path, content) in config_obj {
                    if file_path.contains("server.properties") {
                        if let Some(content_str) = content.as_str() {
                            // Find broker.id in the configuration
                            for line in content_str.lines() {
                                let trimmed = line.trim();
                                if trimmed.starts_with("broker.id=") {
                                    let broker_id = trimmed.strip_prefix("broker.id=").unwrap_or("").trim();
                                    config_broker_ids.entry(broker_id.to_string())
                                        .or_insert_with(Vec::new)
                                        .push(format!("brokers/{}", file_path));
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // Get actual broker IDs from broker_info.json files
        if let Some(brokers_data) = snapshot.collectors.custom.get("brokers") {
            if let Some(brokers_obj) = brokers_data.as_object() {
                for (broker_name, broker_data) in brokers_obj {
                    if let Some(broker_obj) = broker_data.as_object() {
                        if let Some(broker_info) = broker_obj.get("broker_info.json") {
                            if let Some(id) = broker_info.get("id") {
                                if let Some(id_num) = id.as_u64() {
                                    actual_broker_ids.insert(
                                        broker_name.clone(),
                                        id_num.to_string()
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // Check for configuration issues
        for (broker_id, files) in config_broker_ids.iter() {
            if files.len() > 1 || broker_id == "0" {
                // Find actual IDs for these brokers
                let mut actual_ids: Vec<(String, String)> = Vec::new();
                for file_path in files {
                    // Extract broker name from path like "brokers/broker_11/server.properties"
                    if let Some(broker_part) = file_path.split('/').nth(1) {
                        if let Some(actual_id) = actual_broker_ids.get(broker_part) {
                            actual_ids.push((broker_part.to_string(), actual_id.clone()));
                        }
                    }
                }
                
                let has_actual_ids = !actual_ids.is_empty();
                let severity = if has_actual_ids {
                    Severity::Medium  // Config mismatch but brokers are running OK
                } else {
                    Severity::Critical  // Real duplicate IDs
                };
                
                let title = if has_actual_ids {
                    format!("Configuration mismatch: server.properties shows broker.id={} but brokers running with correct IDs", broker_id)
                } else if files.len() > 1 {
                    format!("Duplicate broker.id={} found across {} brokers", broker_id, files.len())
                } else {
                    format!("Default broker.id={} not updated in configuration", broker_id)
                };
                
                let description = if has_actual_ids {
                    format!(
                        "The server.properties files contain broker.id={}, but the brokers are actually \
                        running with different IDs ({}). This indicates that broker IDs are being \
                        overridden at runtime (likely via systemd service files or environment variables). \
                        While the cluster is functioning correctly now, this configuration mismatch could cause issues:\n\
                        • If brokers are restarted using only the config files\n\
                        • During disaster recovery or migration\n\
                        • When troubleshooting (confusing to have wrong values in config)",
                        broker_id,
                        actual_ids.iter()
                            .map(|(_, id)| id.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                } else {
                    format!(
                        "Multiple brokers are configured with the same broker.id={}. \
                        Each broker in a Kafka cluster MUST have a unique broker.id. \
                        This misconfiguration will cause severe cluster issues including:\n\
                        • Brokers failing to start or join the cluster\n\
                        • Data loss and replication failures\n\
                        • Unpredictable cluster behavior\n\
                        • Client connection issues",
                        broker_id
                    )
                };
                
                let finding = Finding {
                    id: format!("CONFIG-BROKER-ID-{:03}", findings.len() + 1),
                    severity,
                    category: Category::Configuration,
                    title,
                    description,
                    impact: if has_actual_ids {
                        "MEDIUM: Configuration files don't reflect actual runtime configuration. \
                        Risk of misconfiguration if brokers are restarted without proper overrides.".to_string()
                    } else {
                        "CRITICAL: Cluster will not function correctly. \
                        Brokers may fail to start, data may be lost, \
                        and the cluster will be unstable.".to_string()
                    },
                    evidence: Evidence {
                        configs: vec![ConfigEvidence {
                            resource_type: "broker".to_string(),
                            resource_name: if files.len() > 1 { "multiple".to_string() } else { "single".to_string() },
                            config_key: "broker.id".to_string(),
                            current_value: if has_actual_ids {
                                format!("{} (in config files)", broker_id)
                            } else {
                                broker_id.clone()
                            },
                            recommended_value: Some(if has_actual_ids {
                                format!("Update to actual IDs: {}", 
                                    actual_ids.iter()
                                        .map(|(name, id)| format!("{} → {}", name, id))
                                        .collect::<Vec<_>>()
                                        .join(", "))
                            } else {
                                "Unique integer for each broker (e.g., 11, 12, 13, ...)".to_string()
                            }),
                            reason: if has_actual_ids {
                                format!("Config files show broker.id={}, but brokers actually running with IDs: {}", 
                                    broker_id,
                                    actual_ids.iter()
                                        .map(|(_, id)| id.as_str())
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                )
                            } else {
                                "Each broker must have a unique identifier".to_string()
                            },
                            source_files: files.clone(),
                        }],
                        logs: vec![],
                        metrics: vec![],
                        raw_data: None,
                    },
                    root_cause: Some(if has_actual_ids {
                        format!(
                            "Configuration management issue: Config files contain default/template value (broker.id={}), \
                            while actual broker IDs are: {}. IDs likely overridden in systemd service files.",
                            broker_id,
                            actual_ids.iter()
                                .map(|(name, id)| format!("{}: {}", name, id))
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    } else {
                        format!(
                            "Configuration error: {} brokers are using the same broker.id={}",
                            files.len(), broker_id
                        )
                    }),
                    remediation: Remediation {
                        steps: if has_actual_ids {
                            vec![
                                RemediationStep {
                                    order: 1,
                                    description: format!("Update server.properties files to reflect actual broker IDs:\n{}", 
                                        actual_ids.iter()
                                            .zip(files.iter())
                                            .map(|((_name, id), file)| format!("  • {} → broker.id={}", file, id))
                                            .collect::<Vec<_>>()
                                            .join("\n")
                                    ),
                                    command: None,
                                    verification: Some("grep broker.id /etc/kafka/server.properties".to_string()),
                                    can_automate: false,
                                },
                                RemediationStep {
                                    order: 2,
                                    description: "Check if systemd service files are overriding broker.id (they might be, which is why cluster works)".to_string(),
                                    command: Some("grep -r 'broker.id' /etc/systemd/system/ /etc/default/".to_string()),
                                    verification: None,
                                    can_automate: false,
                                },
                                RemediationStep {
                                    order: 3,
                                    description: "Document the actual broker ID configuration method for future reference".to_string(),
                                    command: None,
                                    verification: None,
                                    can_automate: false,
                                },
                            ]
                        } else {
                            vec![
                                RemediationStep {
                                    order: 1,
                                    description: "Stop all affected Kafka brokers".to_string(),
                                    command: Some("systemctl stop kafka".to_string()),
                                    verification: Some("systemctl status kafka".to_string()),
                                    can_automate: true,
                                },
                                RemediationStep {
                                    order: 2,
                                    description: format!("Assign unique broker.id values to each broker:\n{}", 
                                        files.iter().enumerate()
                                            .map(|(i, f)| format!("  • {} → broker.id={}", f, 
                                                if actual_broker_ids.is_empty() { i.to_string() } 
                                                else { (11 + i).to_string() }))
                                            .collect::<Vec<_>>()
                                            .join("\n")
                                    ),
                                    command: None,
                                    verification: Some("grep broker.id /etc/kafka/server.properties".to_string()),
                                    can_automate: false,
                                },
                                RemediationStep {
                                    order: 3,
                                    description: "Start Kafka brokers one by one".to_string(),
                                    command: Some("systemctl start kafka".to_string()),
                                    verification: Some("kafka-broker-api-versions --bootstrap-server localhost:9092".to_string()),
                                    can_automate: true,
                                },
                            ]
                        },
                        script: None,
                        risk_level: if has_actual_ids {
                            RiskLevel::Low  // Just updating config files to match reality
                        } else {
                            RiskLevel::High  // Need to actually fix duplicate IDs
                        },
                        requires_downtime: !has_actual_ids,
                        estimated_duration_minutes: Some(if has_actual_ids { 5 } else { 30 }),
                        rollback_plan: Some("Keep backup of original server.properties files".to_string()),
                    },
                    metadata: HashMap::new(),
                };
                findings.push(finding);
            }
        }
        
        findings
    }
    
    /// Check for other common configuration issues
    fn check_common_issues(&self, snapshot: &Snapshot) -> Vec<Finding> {
        let mut findings = Vec::new();
        let mut replication_issues = HashMap::new();
        let mut log_dir_issues = Vec::new();
        
        if let Some(config_data) = &snapshot.collectors.config {
            if let Some(config_obj) = config_data.as_object() {
                for (file_path, content) in config_obj {
                    if file_path.contains("server.properties") {
                        if let Some(content_str) = content.as_str() {
                            for line in content_str.lines() {
                                let trimmed = line.trim();
                                
                                // Check replication factors
                                if trimmed.starts_with("offsets.topic.replication.factor=1") ||
                                   trimmed.starts_with("transaction.state.log.replication.factor=1") {
                                    replication_issues.entry(trimmed.to_string())
                                        .or_insert_with(Vec::new)
                                        .push(format!("brokers/{}", file_path));
                                }
                                
                                // Check log directories
                                if trimmed.starts_with("log.dirs=/tmp/") {
                                    log_dir_issues.push(format!("brokers/{}", file_path));
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // Report replication factor issues
        for (config, files) in replication_issues {
            let param = config.split('=').next().unwrap_or("");
            findings.push(Finding {
                id: format!("CONFIG-REPLICATION-{}", findings.len() + 1),
                severity: Severity::High,
                category: Category::Availability,
                title: format!("Low replication factor: {}", param),
                description: format!(
                    "The {} is set to 1, which provides no fault tolerance. \
                    If a broker fails, data may be lost and the cluster may become unavailable.",
                    param
                ),
                impact: "High risk of data loss and service unavailability".to_string(),
                evidence: Evidence {
                    configs: vec![ConfigEvidence {
                        resource_type: "broker".to_string(),
                        resource_name: "all".to_string(),
                        config_key: param.to_string(),
                        current_value: "1".to_string(),
                        recommended_value: Some("3".to_string()),
                        reason: "Minimum of 3 replicas recommended for production".to_string(),
                        source_files: files,
                    }],
                    logs: vec![],
                    metrics: vec![],
                    raw_data: None,
                },
                root_cause: Some("Insufficient replication configuration".to_string()),
                remediation: Remediation {
                    steps: vec![RemediationStep {
                        order: 1,
                        description: format!("Update {} to 3 in all broker configurations", param),
                        command: Some(format!("sed -i 's/{}=1/{}=3/g' /etc/kafka/server.properties", param, param)),
                        verification: Some(format!("grep {} /etc/kafka/server.properties", param)),
                        can_automate: true,
                    }],
                    script: None,
                    risk_level: RiskLevel::Medium,
                    requires_downtime: true,
                    estimated_duration_minutes: Some(15),
                    rollback_plan: Some("Revert to replication factor of 1".to_string()),
                },
                metadata: HashMap::new(),
            });
        }
        
        // Report log directory issues
        if !log_dir_issues.is_empty() {
            findings.push(Finding {
                id: "CONFIG-LOGDIR-001".to_string(),
                severity: Severity::Medium,
                category: Category::Configuration,
                title: "Logs stored in temporary directory".to_string(),
                description: "Kafka logs are configured to be stored in /tmp, which is typically \
                            cleared on system restart and may have limited space. This can lead to \
                            data loss and disk space issues.".to_string(),
                impact: "Risk of data loss on system restart and potential disk space issues".to_string(),
                evidence: Evidence {
                    configs: vec![ConfigEvidence {
                        resource_type: "broker".to_string(),
                        resource_name: "multiple".to_string(),
                        config_key: "log.dirs".to_string(),
                        current_value: "/tmp/kafka-logs".to_string(),
                        recommended_value: Some("/var/kafka/logs".to_string()),
                        reason: "Use dedicated persistent storage for Kafka logs".to_string(),
                        source_files: log_dir_issues,
                    }],
                    logs: vec![],
                    metrics: vec![],
                    raw_data: None,
                },
                root_cause: Some("Inappropriate storage location for Kafka logs".to_string()),
                remediation: Remediation {
                    steps: vec![
                        RemediationStep {
                            order: 1,
                            description: "Create dedicated log directory".to_string(),
                            command: Some("mkdir -p /var/kafka/logs && chown kafka:kafka /var/kafka/logs".to_string()),
                            verification: Some("ls -la /var/kafka/".to_string()),
                            can_automate: true,
                        },
                        RemediationStep {
                            order: 2,
                            description: "Update log.dirs in server.properties".to_string(),
                            command: Some("sed -i 's|log.dirs=/tmp/kafka-logs|log.dirs=/var/kafka/logs|g' /etc/kafka/server.properties".to_string()),
                            verification: Some("grep log.dirs /etc/kafka/server.properties".to_string()),
                            can_automate: true,
                        },
                    ],
                    script: None,
                    risk_level: RiskLevel::Medium,
                    requires_downtime: true,
                    estimated_duration_minutes: Some(20),
                    rollback_plan: Some("Move logs back to /tmp/kafka-logs".to_string()),
                },
                metadata: HashMap::new(),
            });
        }
        
        findings
    }
}

#[async_trait]
impl Analyzer for ConfigValidator {
    async fn analyze(&self, snapshot: &Snapshot) -> AnalyzerResult<Vec<Finding>> {
        let mut findings = Vec::new();
        
        // Check for broker ID issues
        findings.extend(self.check_broker_id_issues(snapshot));
        
        // Check for other common issues
        findings.extend(self.check_common_issues(snapshot));
        
        Ok(findings)
    }
    
    fn name(&self) -> &'static str {
        "Configuration Validator"
    }
    
    fn description(&self) -> &'static str {
        "Validates Kafka configuration for common issues and best practices"
    }
}