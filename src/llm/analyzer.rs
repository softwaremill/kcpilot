use crate::analyzers::{Analyzer, AnalyzerError, AnalyzerResult};
use crate::llm::{LlmService, LlmServiceError};
use crate::snapshot::format::{
    Category, ConfigEvidence, Evidence, Finding, LogEvidence,
    Remediation, RemediationStep, RiskLevel, Severity, Snapshot,
};
use async_trait::async_trait;
use serde_json::json;
use std::collections::HashMap;

/// LLM-powered analyzer for advanced Kafka cluster analysis
pub struct LlmAnalyzer {
    service: LlmService,
    enabled_analyses: Vec<AnalysisType>,
}

#[derive(Debug, Clone, Copy)]
pub enum AnalysisType {
    LogIntelligence,
    RootCauseAnalysis,
    PerformanceAnalysis,
    ConfigurationOptimization,
    CapacityPlanning,
    SecurityAudit,
}

impl LlmAnalyzer {
    /// Create a new LLM analyzer
    pub fn new(service: LlmService) -> Self {
        Self {
            service,
            enabled_analyses: vec![
                AnalysisType::LogIntelligence,
                AnalysisType::RootCauseAnalysis,
                AnalysisType::PerformanceAnalysis,
                AnalysisType::ConfigurationOptimization,
            ],
        }
    }
    
    /// Create from environment configuration
    pub fn from_env() -> Result<Self, LlmServiceError> {
        let service = LlmService::from_env()?;
        Ok(Self::new(service))
    }
    
    /// Create from environment configuration with debug logging
    pub fn from_env_with_debug(enable_debug: bool) -> Result<Self, LlmServiceError> {
        let service = LlmService::from_env_with_debug(enable_debug)?;
        Ok(Self::new(service))
    }
    
    /// Create from environment configuration with options
    pub fn from_env_with_options(enable_debug: bool, timeout_secs: u64) -> Result<Self, LlmServiceError> {
        let service = LlmService::from_env_with_options(enable_debug, timeout_secs)?;
        Ok(Self::new(service))
    }
    
    /// Enable specific analysis types
    pub fn with_analyses(mut self, analyses: Vec<AnalysisType>) -> Self {
        self.enabled_analyses = analyses;
        self
    }
    
    /// Analyze logs using LLM
    async fn analyze_logs(&self, snapshot: &Snapshot) -> AnalyzerResult<Vec<Finding>> {
        let mut findings = Vec::new();
        
        // Extract log data from snapshot
        if let Some(logs_data) = &snapshot.collectors.logs {
            let logs_str = serde_json::to_string_pretty(logs_data)
                .map_err(|e| AnalyzerError::InvalidData(e.to_string()))?;
            
            // Limit log size for API calls
            let truncated_logs = if logs_str.len() > 10000 {
                format!("{}... [truncated]", &logs_str[..10000])
            } else {
                logs_str
            };
            
            // Analyze logs with LLM
            match self.service.analyze_logs(&truncated_logs, Some("Kafka broker logs")).await {
                Ok(analysis) => {
                    // Convert analysis to findings
                    for (i, issue) in analysis.issues.iter().enumerate() {
                        let finding = Finding {
                            id: format!("LLM-LOG-{:03}", i + 1),
                            severity: parse_severity(&analysis.severity),
                            category: Category::Performance,
                            title: issue.clone(),
                            description: analysis.summary.clone(),
                            impact: "Potential cluster instability".to_string(),
                            evidence: Evidence {
                                logs: vec![LogEvidence {
                                    level: "ERROR".to_string(),
                                    message: issue.clone(),
                                    source_file: "broker.log".to_string(),
                                    line_number: None,
                                    timestamp: chrono::Utc::now().to_string(),
                                    count: 1,
                                }],
                                metrics: vec![],
                                configs: vec![],
                                raw_data: Some(json!({"llm_analysis": analysis})),
                            },
                            root_cause: Some("Identified by LLM analysis".to_string()),
                            remediation: build_remediation(&analysis.recommendations),
                            metadata: HashMap::new(),
                        };
                        findings.push(finding);
                    }
                }
                Err(e) => {
                    tracing::warn!("LLM log analysis failed: {}", e);
                }
            }
        }
        
        Ok(findings)
    }
    
    /// Perform root cause analysis
    async fn perform_root_cause_analysis(&self, snapshot: &Snapshot) -> AnalyzerResult<Vec<Finding>> {
        let mut findings = Vec::new();
        
        // Gather symptoms from existing findings
        let symptoms: Vec<String> = snapshot.findings.iter()
            .filter(|f| matches!(f.severity, Severity::Critical | Severity::High))
            .map(|f| f.title.clone())
            .collect();
        
        if !symptoms.is_empty() {
            // Prepare evidence
            let evidence = json!({
                "metrics": snapshot.collectors.metrics,
                "cluster_info": snapshot.cluster,
                "finding_count": snapshot.findings.len(),
            });
            
            match self.service.root_cause_analysis(&symptoms, &evidence).await {
                Ok(analysis) => {
                    let finding = Finding {
                        id: "LLM-RCA-001".to_string(),
                        severity: Severity::High,
                        category: Category::Other,
                        title: "Root Cause Analysis".to_string(),
                        description: analysis.root_cause.clone(),
                        impact: format!("Confidence: {:.0}%", analysis.confidence * 100.0),
                        evidence: Evidence {
                            logs: vec![],
                            metrics: vec![],
                            configs: vec![],
                            raw_data: Some(json!({
                                "root_cause": analysis.root_cause,
                                "confidence": analysis.confidence,
                                "factors": analysis.contributing_factors,
                            })),
                        },
                        root_cause: Some(analysis.root_cause),
                        remediation: Remediation {
                            steps: vec![],
                            script: None,
                            risk_level: RiskLevel::Medium,
                            requires_downtime: false,
                            estimated_duration_minutes: None,
                            rollback_plan: None,
                        },
                        metadata: HashMap::new(),
                    };
                    findings.push(finding);
                }
                Err(e) => {
                    tracing::warn!("LLM root cause analysis failed: {}", e);
                }
            }
        }
        
        Ok(findings)
    }
    
    /// Analyze configuration for optimization
    async fn analyze_configuration(&self, snapshot: &Snapshot) -> AnalyzerResult<Vec<Finding>> {
        let mut findings = Vec::new();
        
        if let Some(config_data) = &snapshot.collectors.config {
            let empty_json = json!({});
            let metrics = snapshot.collectors.metrics.as_ref()
                .unwrap_or(&empty_json);
            
            // Build a map of config parameters to their source files
            let mut config_file_map: HashMap<String, Vec<String>> = HashMap::new();
            if let Some(config_obj) = config_data.as_object() {
                for (file_path, content) in config_obj {
                    if let Some(content_str) = content.as_str() {
                        // Parse configuration lines to find parameters
                        for line in content_str.lines() {
                            if let Some(eq_pos) = line.find('=') {
                                let key = line[..eq_pos].trim();
                                if !key.starts_with('#') && !key.is_empty() {
                                    config_file_map.entry(key.to_string())
                                        .or_insert_with(Vec::new)
                                        .push(format!("brokers/{}", file_path));
                                }
                            }
                        }
                    }
                }
            }
            
            match self.service.optimize_configuration(
                config_data,
                metrics,
                "General Kafka workload"
            ).await {
                Ok(recommendations) => {
                    for (i, rec) in recommendations.iter().enumerate() {
                        // Find which files contain this parameter
                        let source_files = config_file_map.get(&rec.parameter)
                            .cloned()
                            .unwrap_or_else(Vec::new);
                        
                        let finding = Finding {
                            id: format!("LLM-CONFIG-{:03}", i + 1),
                            severity: Severity::Medium,
                            category: Category::Configuration,
                            title: format!("Configuration optimization: {}", rec.parameter),
                            description: rec.justification.clone(),
                            impact: rec.impact.clone(),
                            evidence: Evidence {
                                configs: vec![ConfigEvidence {
                                    resource_type: "broker".to_string(),
                                    resource_name: "all".to_string(),
                                    config_key: rec.parameter.clone(),
                                    current_value: rec.current_value.clone(),
                                    recommended_value: Some(rec.recommended_value.clone()),
                                    reason: rec.justification.clone(),
                                    source_files,
                                }],
                                logs: vec![],
                                metrics: vec![],
                                raw_data: None,
                            },
                            root_cause: None,
                            remediation: Remediation {
                                steps: vec![RemediationStep {
                                    order: 1,
                                    description: format!("Update {} from {} to {}", 
                                                       rec.parameter, 
                                                       rec.current_value, 
                                                       rec.recommended_value),
                                    command: Some(format!("kafka-configs --alter --add-config {}={}", 
                                                        rec.parameter, 
                                                        rec.recommended_value)),
                                    verification: Some("Verify configuration is applied".to_string()),
                                    can_automate: true,
                                }],
                                script: None,
                                risk_level: RiskLevel::Low,
                                requires_downtime: false,
                                estimated_duration_minutes: Some(5),
                                rollback_plan: Some(format!("Revert {} to {}", 
                                                          rec.parameter, 
                                                          rec.current_value)),
                            },
                            metadata: HashMap::new(),
                        };
                        findings.push(finding);
                    }
                }
                Err(e) => {
                    tracing::warn!("LLM configuration analysis failed: {}", e);
                }
            }
        }
        
        Ok(findings)
    }
}

#[async_trait]
impl Analyzer for LlmAnalyzer {
    async fn analyze(&self, snapshot: &Snapshot) -> AnalyzerResult<Vec<Finding>> {
        let mut all_findings = Vec::new();
        
        for analysis_type in &self.enabled_analyses {
            let findings = match analysis_type {
                AnalysisType::LogIntelligence => {
                    self.analyze_logs(snapshot).await?
                }
                AnalysisType::RootCauseAnalysis => {
                    self.perform_root_cause_analysis(snapshot).await?
                }
                AnalysisType::ConfigurationOptimization => {
                    self.analyze_configuration(snapshot).await?
                }
                _ => {
                    // Other analysis types can be implemented similarly
                    vec![]
                }
            };
            
            all_findings.extend(findings);
        }
        
        Ok(all_findings)
    }
    
    fn name(&self) -> &'static str {
        "LLM-Enhanced Analyzer"
    }
    
    fn description(&self) -> &'static str {
        "Advanced analysis using Large Language Models for intelligent log analysis, \
         root cause analysis, and optimization recommendations"
    }
}

/// Parse severity string to enum
fn parse_severity(severity: &str) -> Severity {
    match severity.to_lowercase().as_str() {
        "critical" => Severity::Critical,
        "high" => Severity::High,
        "medium" => Severity::Medium,
        "low" => Severity::Low,
        _ => Severity::Info,
    }
}

/// Build remediation from recommendations
fn build_remediation(recommendations: &[String]) -> Remediation {
    let steps = recommendations.iter().enumerate().map(|(i, rec)| {
        RemediationStep {
            order: (i + 1) as u32,
            description: rec.clone(),
            command: None,
            verification: None,
            can_automate: false,
        }
    }).collect();
    
    Remediation {
        steps,
        script: None,
        risk_level: RiskLevel::Medium,
        requires_downtime: false,
        estimated_duration_minutes: Some(30),
        rollback_plan: None,
    }
}
