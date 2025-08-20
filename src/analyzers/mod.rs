pub mod rules;

use crate::snapshot::format::{Finding, Snapshot};
use async_trait::async_trait;

/// Result type for analyzer operations
pub type AnalyzerResult<T> = Result<T, AnalyzerError>;

/// Errors that can occur during analysis
#[derive(Debug, thiserror::Error)]
pub enum AnalyzerError {
    #[error("Analysis failed: {0}")]
    AnalysisFailed(String),
    
    #[error("Invalid data: {0}")]
    InvalidData(String),
    
    #[error("Rule evaluation error: {0}")]
    RuleError(String),
    
    #[error("Other error: {0}")]
    Other(String),
}

/// Base trait for analyzers
#[async_trait]
pub trait Analyzer: Send + Sync {
    /// Analyze a snapshot and produce findings
    async fn analyze(&self, snapshot: &Snapshot) -> AnalyzerResult<Vec<Finding>>;
    
    /// Get analyzer name
    fn name(&self) -> &'static str;
    
    /// Get analyzer description
    fn description(&self) -> &'static str;
}

/// Registry for analyzers
pub struct AnalyzerRegistry {
    analyzers: Vec<Box<dyn Analyzer>>,
}

impl AnalyzerRegistry {
    pub fn new() -> Self {
        Self {
            analyzers: Vec::new(),
        }
    }
    
    pub fn register(&mut self, analyzer: Box<dyn Analyzer>) {
        self.analyzers.push(analyzer);
    }
    
    pub async fn analyze_all(&self, snapshot: &Snapshot) -> AnalyzerResult<Vec<Finding>> {
        let mut all_findings = Vec::new();
        
        for analyzer in &self.analyzers {
            match analyzer.analyze(snapshot).await {
                Ok(findings) => {
                    all_findings.extend(findings);
                }
                Err(e) => {
                    tracing::warn!("Analyzer {} failed: {}", analyzer.name(), e);
                }
            }
        }
        
        // Sort findings by severity
        all_findings.sort_by(|a, b| a.severity.cmp(&b.severity));
        
        Ok(all_findings)
    }
    
    pub fn list(&self) -> Vec<String> {
        self.analyzers.iter().map(|a| a.name().to_string()).collect()
    }
}
