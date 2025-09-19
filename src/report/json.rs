use crate::snapshot::format::{Finding, Snapshot, Severity};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Write;
use std::path::Path;
use chrono::Utc;
use anyhow::Result;

/// JSON report structure with all relevant data
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonReport {
    /// Report metadata
    pub metadata: ReportMetadata,
    
    /// Cluster information
    pub cluster_info: ClusterInfo,
    
    /// Analysis findings
    pub findings: Vec<Finding>,
    
    /// Summary statistics
    pub summary: Summary,
    
    /// Health score (0-100)
    pub health_score: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReportMetadata {
    pub report_generated_at: String,
    pub tool_version: String,
    pub scan_timestamp: String,
    pub output_directory: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClusterInfo {
    pub cluster_id: Option<String>,
    pub cluster_mode: Option<String>,
    pub broker_count: usize,
    pub topic_count: Option<usize>,
    pub partition_count: Option<usize>,
    pub bastion_host: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Summary {
    pub total_findings: usize,
    pub critical_count: usize,
    pub high_count: usize,
    pub medium_count: usize,
    pub low_count: usize,
    pub info_count: usize,
}

/// JSON report generator
pub struct JsonReporter;

impl JsonReporter {
    pub fn new() -> Self {
        Self
    }
    
    /// Generate and save JSON report
    pub fn save_report(&self, snapshot: &Snapshot, findings: &[Finding], output_path: &Path) -> Result<()> {
        let report = self.generate_report(snapshot, findings);
        
        // Serialize to JSON
        let json = serde_json::to_string_pretty(&report)?;
        
        // If output_path is provided, save to file; otherwise print to stdout
        if output_path == Path::new("-") || output_path == Path::new("") {
            println!("{}", json);
        } else {
            let mut file = File::create(output_path)?;
            file.write_all(json.as_bytes())?;
        }
        
        Ok(())
    }
    
    /// Generate the JSON report structure
    fn generate_report(&self, snapshot: &Snapshot, findings: &[Finding]) -> JsonReport {
        // Count findings by severity
        let mut critical_count = 0;
        let mut high_count = 0;
        let mut medium_count = 0;
        let mut low_count = 0;
        let mut info_count = 0;
        
        for finding in findings {
            match finding.severity {
                Severity::Critical => critical_count += 1,
                Severity::High => high_count += 1,
                Severity::Medium => medium_count += 1,
                Severity::Low => low_count += 1,
                Severity::Info => info_count += 1,
            }
        }
        
        // Calculate health score (100 - penalties)
        let health_score = (100.0
            - (critical_count as f64 * 20.0)
            - (high_count as f64 * 10.0)
            - (medium_count as f64 * 5.0)
            - (low_count as f64 * 2.0)
            - (info_count as f64 * 0.5))
            .max(0.0);
        
        // Extract cluster info
        let cluster_info = ClusterInfo {
            cluster_id: snapshot.cluster.id.clone(),
            cluster_mode: Some(format!("{:?}", snapshot.cluster.mode).to_lowercase()),
            broker_count: snapshot.collectors.admin.as_ref()
                .and_then(|admin| admin.get("brokers"))
                .and_then(|brokers| brokers.as_array())
                .map(|arr| arr.len())
                .unwrap_or(0),
            topic_count: snapshot.collectors.admin.as_ref()
                .and_then(|admin| admin.get("topics"))
                .and_then(|topics| topics.as_array())
                .map(|arr| arr.len()),
            partition_count: snapshot.collectors.admin.as_ref()
                .and_then(|admin| admin.get("partitions"))
                .and_then(|partitions| partitions.as_array())
                .map(|arr| arr.len()),
            bastion_host: snapshot.metadata.environment.clone(),
        };
        
        JsonReport {
            metadata: ReportMetadata {
                report_generated_at: Utc::now().to_rfc3339(),
                tool_version: snapshot.metadata.tool_version.clone(),
                scan_timestamp: snapshot.timestamp.to_rfc3339(),
                output_directory: snapshot.metadata.collection_id.clone(),
            },
            cluster_info,
            findings: findings.to_vec(),
            summary: Summary {
                total_findings: findings.len(),
                critical_count,
                high_count,
                medium_count,
                low_count,
                info_count,
            },
            health_score,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::format::{ClusterMode, CollectorOutputs, SnapshotMetadata, ClusterSnapshot, Category, Evidence, Remediation, RiskLevel};
    use chrono::DateTime;
    use std::collections::HashMap;

    fn create_test_snapshot() -> Snapshot {
        Snapshot {
            version: "1.0.0".to_string(),
            timestamp: DateTime::parse_from_rfc3339("2023-01-01T12:00:00Z").unwrap().into(),
            metadata: SnapshotMetadata {
                tool_version: "1.0.0".to_string(),
                collection_id: "test-scan-123".to_string(),
                collection_duration_ms: 60000,
                collectors_used: vec!["admin".to_string()],
                redaction_applied: false,
                environment: Some("test-bastion".to_string()),
                tags: HashMap::new(),
            },
            cluster: ClusterSnapshot {
                id: Some("test-cluster".to_string()),
                name: None,
                version: None,
                mode: ClusterMode::Kraft,
                cloud_provider: None,
                region: None,
            },
            collectors: CollectorOutputs {
                admin: Some(serde_json::json!({
                    "brokers": [
                        {"id": 1, "host": "broker1"},
                        {"id": 2, "host": "broker2"}
                    ],
                    "topics": [
                        {"name": "topic1"},
                        {"name": "topic2"},
                        {"name": "topic3"}
                    ]
                })),
                logs: None,
                config: None,
                metrics: None,
                cloud: None,
                custom: HashMap::new(),
            },
            findings: Vec::new(),
        }
    }

    fn create_test_findings() -> Vec<Finding> {
        vec![
            Finding {
                id: "TEST-001".to_string(),
                title: "Critical Issue".to_string(),
                description: "A critical issue found".to_string(),
                severity: Severity::Critical,
                category: Category::Configuration,
                impact: "High impact on cluster".to_string(),
                evidence: Evidence {
                    metrics: Vec::new(),
                    logs: Vec::new(),
                    configs: Vec::new(),
                    raw_data: None,
                },
                root_cause: Some("Configuration mismatch".to_string()),
                remediation: Remediation {
                    steps: Vec::new(),
                    script: None,
                    risk_level: RiskLevel::Low,
                    requires_downtime: false,
                    estimated_duration_minutes: Some(30),
                    rollback_plan: None,
                },
                metadata: HashMap::new(),
            },
            Finding {
                id: "TEST-002".to_string(),
                title: "High Issue".to_string(),
                description: "A high severity issue".to_string(),
                severity: Severity::High,
                category: Category::Performance,
                impact: "Performance degradation".to_string(),
                evidence: Evidence {
                    metrics: Vec::new(),
                    logs: Vec::new(),
                    configs: Vec::new(),
                    raw_data: None,
                },
                root_cause: Some("Resource contention".to_string()),
                remediation: Remediation {
                    steps: Vec::new(),
                    script: None,
                    risk_level: RiskLevel::Medium,
                    requires_downtime: false,
                    estimated_duration_minutes: Some(60),
                    rollback_plan: None,
                },
                metadata: HashMap::new(),
            },
            Finding {
                id: "TEST-003".to_string(),
                title: "Medium Issue".to_string(),
                description: "A medium severity issue".to_string(),
                severity: Severity::Medium,
                category: Category::Configuration,
                impact: "Minor impact".to_string(),
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
            },
        ]
    }

    #[test]
    fn test_generate_report() {
        let reporter = JsonReporter::new();
        let snapshot = create_test_snapshot();
        let findings = create_test_findings();

        let report = reporter.generate_report(&snapshot, &findings);

        // Test metadata
        assert_eq!(report.metadata.tool_version, "1.0.0");
        assert_eq!(report.metadata.output_directory, "test-scan-123");

        // Test cluster info
        assert_eq!(report.cluster_info.cluster_id, Some("test-cluster".to_string()));
        assert_eq!(report.cluster_info.cluster_mode, Some("kraft".to_string()));
        assert_eq!(report.cluster_info.broker_count, 2);
        assert_eq!(report.cluster_info.topic_count, Some(3));
        assert_eq!(report.cluster_info.bastion_host, Some("test-bastion".to_string()));

        // Test findings
        assert_eq!(report.findings.len(), 3);
        assert_eq!(report.findings[0].severity, Severity::Critical);

        // Test summary
        assert_eq!(report.summary.total_findings, 3);
        assert_eq!(report.summary.critical_count, 1);
        assert_eq!(report.summary.high_count, 1);
        assert_eq!(report.summary.medium_count, 1);
        assert_eq!(report.summary.low_count, 0);
        assert_eq!(report.summary.info_count, 0);

        // Test health score calculation
        // 100 - (1*20 + 1*10 + 1*5) = 65
        assert_eq!(report.health_score, 65.0);
    }

    #[test]
    fn test_health_score_calculation() {
        let reporter = JsonReporter::new();
        let snapshot = create_test_snapshot();

        // Test with different severity combinations
        let critical_findings = vec![Finding {
            id: "C1".to_string(),
            title: "Critical".to_string(),
            description: "Critical".to_string(),
            severity: Severity::Critical,
            category: Category::Other,
            impact: "Test impact".to_string(),
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
        }];

        let report = reporter.generate_report(&snapshot, &critical_findings);
        assert_eq!(report.health_score, 80.0); // 100 - 20

        // Test minimum score
        let many_critical_findings: Vec<Finding> = (0..10).map(|i| Finding {
            id: format!("C{}", i),
            title: "Critical".to_string(),
            description: "Critical".to_string(),
            severity: Severity::Critical,
            category: Category::Other,
            impact: "Test impact".to_string(),
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
        }).collect();

        let report = reporter.generate_report(&snapshot, &many_critical_findings);
        assert_eq!(report.health_score, 0.0); // Can't go below 0
    }

    #[test]
    fn test_summary_counts() {
        let reporter = JsonReporter::new();
        let snapshot = create_test_snapshot();

        let mixed_findings = vec![
            Finding {
                id: "1".to_string(),
                title: "Test".to_string(),
                description: "Test".to_string(),
                severity: Severity::Critical,
                category: Category::Other,
                impact: "Test impact".to_string(),
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
            },
            Finding {
                id: "2".to_string(),
                title: "Test".to_string(),
                description: "Test".to_string(),
                severity: Severity::Critical,
                category: Category::Other,
                impact: "Test impact".to_string(),
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
            },
            Finding {
                id: "3".to_string(),
                title: "Test".to_string(),
                description: "Test".to_string(),
                severity: Severity::High,
                category: Category::Other,
                impact: "Test impact".to_string(),
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
            },
            Finding {
                id: "4".to_string(),
                title: "Test".to_string(),
                description: "Test".to_string(),
                severity: Severity::Info,
                category: Category::Other,
                impact: "Test impact".to_string(),
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
            },
        ];

        let report = reporter.generate_report(&snapshot, &mixed_findings);

        assert_eq!(report.summary.total_findings, 4);
        assert_eq!(report.summary.critical_count, 2);
        assert_eq!(report.summary.high_count, 1);
        assert_eq!(report.summary.medium_count, 0);
        assert_eq!(report.summary.low_count, 0);
        assert_eq!(report.summary.info_count, 1);
    }

    #[test]
    fn test_cluster_info_extraction() {
        let reporter = JsonReporter::new();
        let mut snapshot = create_test_snapshot();
        
        // Test with missing admin data
        snapshot.collectors.admin = None;
        let findings = vec![];
        let report = reporter.generate_report(&snapshot, &findings);
        
        assert_eq!(report.cluster_info.broker_count, 0);
        assert_eq!(report.cluster_info.topic_count, None);
        assert_eq!(report.cluster_info.partition_count, None);
    }
}