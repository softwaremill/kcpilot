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