pub mod terminal;
pub mod markdown;

use crate::snapshot::format::Snapshot;
use std::path::Path;

/// Result type for report operations
pub type ReportResult<T> = Result<T, ReportError>;

/// Errors that can occur during report generation
#[derive(Debug, thiserror::Error)]
pub enum ReportError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Template error: {0}")]
    TemplateError(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    
    #[error("Other error: {0}")]
    Other(String),
}

/// Trait for report generators
pub trait ReportGenerator {
    /// Generate a report from a snapshot
    fn generate(&self, snapshot: &Snapshot, output_path: &Path) -> ReportResult<()>;
    
    /// Get generator name
    fn name(&self) -> &'static str;
}
