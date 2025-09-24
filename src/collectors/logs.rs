use super::{Collector, CollectorError, CollectorResult, KafkaConfig};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Log collector for gathering Kafka broker and application logs
pub struct LogCollector {
    log_paths: Vec<PathBuf>,
    max_lines_per_file: usize,
    patterns: Vec<String>,
}

impl Default for LogCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl LogCollector {
    pub fn new() -> Self {
        Self {
            log_paths: Self::default_log_paths(),
            max_lines_per_file: 1000,
            patterns: Self::default_patterns(),
        }
    }

    pub fn with_paths(mut self, paths: Vec<PathBuf>) -> Self {
        self.log_paths = paths;
        self
    }

    pub fn with_max_lines(mut self, max_lines: usize) -> Self {
        self.max_lines_per_file = max_lines;
        self
    }

    pub fn with_patterns(mut self, patterns: Vec<String>) -> Self {
        self.patterns = patterns;
        self
    }

    fn default_log_paths() -> Vec<PathBuf> {
        // This method is now deprecated in favor of dynamic discovery
        // Keeping for backward compatibility only
        vec![
            PathBuf::from("/var/log/kafka/server.log"),
            PathBuf::from("/var/log/kafka/controller.log"),
            PathBuf::from("/var/log/kafka/state-change.log"),
            PathBuf::from("/var/log/kafka/kafka-request.log"),
            PathBuf::from("/opt/kafka/logs/server.log"),
            PathBuf::from("./logs/kafka.log"),
        ]
    }

    /// Create a new LogCollector with dynamic log discovery
    /// This method is preferred over new() as it discovers logs dynamically
    pub fn with_dynamic_discovery() -> Self {
        Self {
            log_paths: Vec::new(), // Will be discovered dynamically
            max_lines_per_file: 1000,
            patterns: Self::default_patterns(),
        }
    }

    fn default_patterns() -> Vec<String> {
        vec![
            "ERROR".to_string(),
            "WARN".to_string(),
            "FATAL".to_string(),
            "Exception".to_string(),
            "Failed".to_string(),
            "Timeout".to_string(),
            "Disconnected".to_string(),
            "UnderReplicated".to_string(),
            "ISR".to_string(),
            "Leader".to_string(),
            "Election".to_string(),
            "Rebalance".to_string(),
        ]
    }

    async fn collect_file_logs(&self, path: &Path) -> CollectorResult<Vec<LogEntry>> {
        if !path.exists() {
            debug!("Log file does not exist: {:?}", path);
            return Ok(vec![]);
        }

        let file = File::open(path)
            .map_err(CollectorError::IoError)?;
        let reader = BufReader::new(file);
        
        let mut entries = Vec::new();
        let mut line_count = 0;
        
        for line_result in reader.lines() {
            if line_count >= self.max_lines_per_file {
                break;
            }
            
            let line = line_result.map_err(CollectorError::IoError)?;
            
            // Check if line matches any pattern
            let matches_pattern = self.patterns.iter().any(|pattern| {
                line.contains(pattern)
            });
            
            if matches_pattern {
                let entry = self.parse_log_line(&line);
                entries.push(entry);
                line_count += 1;
            }
        }
        
        Ok(entries)
    }

    fn parse_log_line(&self, line: &str) -> LogEntry {
        // Basic log parsing - can be enhanced based on log format
        let parts: Vec<&str> = line.splitn(4, ' ').collect();
        
        let (timestamp, level, source, message) = if parts.len() >= 4 {
            (
                parts[0].to_string(),
                self.parse_log_level(parts[1]),
                parts[2].to_string(),
                parts[3].to_string(),
            )
        } else {
            (
                String::new(),
                LogLevel::Info,
                String::new(),
                line.to_string(),
            )
        };
        
        LogEntry {
            timestamp,
            level,
            source,
            message,
            raw: line.to_string(),
        }
    }

    fn parse_log_level(&self, level_str: &str) -> LogLevel {
        match level_str.to_uppercase().as_str() {
            "FATAL" => LogLevel::Fatal,
            "ERROR" => LogLevel::Error,
            "WARN" | "WARNING" => LogLevel::Warn,
            "INFO" => LogLevel::Info,
            "DEBUG" => LogLevel::Debug,
            "TRACE" => LogLevel::Trace,
            _ => LogLevel::Info,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogCollectorOutput {
    pub logs: HashMap<String, Vec<LogEntry>>,
    pub summary: LogSummary,
    pub collection_timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: LogLevel,
    pub source: String,
    pub message: String,
    pub raw: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum LogLevel {
    Fatal,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogSummary {
    pub total_entries: usize,
    pub error_count: usize,
    pub warn_count: usize,
    pub files_processed: usize,
    pub patterns_matched: HashMap<String, usize>,
    pub top_errors: Vec<ErrorSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorSummary {
    pub pattern: String,
    pub count: usize,
    pub first_occurrence: String,
    pub last_occurrence: String,
    pub sample_message: String,
}

/// Configuration specific to log collection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogConfig {
    pub kafka_config: KafkaConfig,
    pub log_paths: Vec<PathBuf>,
    pub remote_logs: Option<RemoteLogConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteLogConfig {
    pub source: LogSource,
    pub credentials: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogSource {
    CloudWatch,
    GcpLogging,
    Elasticsearch,
    Splunk,
    Local,
}

#[async_trait]
impl Collector for LogCollector {
    type Config = LogConfig;
    type Output = LogCollectorOutput;

    async fn collect(&self, config: &Self::Config) -> CollectorResult<Self::Output> {
        info!("Starting log collection with dynamic discovery");
        
        let mut all_logs = HashMap::new();
        let mut total_entries = 0;
        let mut error_count = 0;
        let mut warn_count = 0;
        let mut files_processed = 0;
        let mut pattern_counts = HashMap::new();
        
        // Determine which paths to use
        let default_paths;
        let paths = if !config.log_paths.is_empty() {
            // Use configured paths if provided
            &config.log_paths
        } else if !self.log_paths.is_empty() {
            // Use collector's default paths
            &self.log_paths  
        } else {
            // Use hardcoded fallback paths (LogDiscovery has been refactored)
            info!("Using fallback log paths");
            
            default_paths = Self::default_log_paths();
            &default_paths
        };
        
        for path in paths {
            match self.collect_file_logs(path).await {
                Ok(entries) => {
                    if !entries.is_empty() {
                        files_processed += 1;
                        
                        // Count log levels
                        for entry in &entries {
                            total_entries += 1;
                            match entry.level {
                                LogLevel::Error | LogLevel::Fatal => error_count += 1,
                                LogLevel::Warn => warn_count += 1,
                                _ => {}
                            }
                            
                            // Count pattern matches
                            for pattern in &self.patterns {
                                if entry.raw.contains(pattern) {
                                    *pattern_counts.entry(pattern.clone()).or_insert(0) += 1;
                                }
                            }
                        }
                        
                        all_logs.insert(
                            path.to_string_lossy().to_string(),
                            entries,
                        );
                    }
                }
                Err(e) => {
                    warn!("Failed to collect logs from {:?}: {}", path, e);
                }
            }
        }
        
        // TODO: Implement remote log collection (CloudWatch, GCP, etc.)
        if let Some(remote) = &config.remote_logs {
            match remote.source {
                LogSource::CloudWatch => {
                    // Implement CloudWatch log collection
                    warn!("CloudWatch log collection not yet implemented");
                }
                LogSource::GcpLogging => {
                    // Implement GCP log collection
                    warn!("GCP log collection not yet implemented");
                }
                _ => {
                    warn!("Remote log source {:?} not yet implemented", remote.source);
                }
            }
        }
        
        // Generate top errors summary
        let mut error_summaries = Vec::new();
        for (pattern, count) in &pattern_counts {
            if *count > 0 {
                // Find sample message for this pattern
                let mut sample = String::new();
                let mut first_occurrence = String::new();
                let mut last_occurrence = String::new();
                
                for logs in all_logs.values() {
                    for entry in logs {
                        if entry.raw.contains(pattern) {
                            if sample.is_empty() {
                                sample = entry.message.clone();
                                first_occurrence = entry.timestamp.clone();
                            }
                            last_occurrence = entry.timestamp.clone();
                        }
                    }
                }
                
                error_summaries.push(ErrorSummary {
                    pattern: pattern.clone(),
                    count: *count,
                    first_occurrence,
                    last_occurrence,
                    sample_message: sample,
                });
            }
        }
        
        // Sort by count descending
        error_summaries.sort_by(|a, b| b.count.cmp(&a.count));
        error_summaries.truncate(10); // Keep top 10
        
        let summary = LogSummary {
            total_entries,
            error_count,
            warn_count,
            files_processed,
            patterns_matched: pattern_counts,
            top_errors: error_summaries,
        };
        
        let output = LogCollectorOutput {
            logs: all_logs,
            summary,
            collection_timestamp: chrono::Utc::now(),
        };
        
        info!("Log collection completed. Processed {} files, found {} entries", 
              files_processed, total_entries);
        
        Ok(output)
    }

    fn redact(&self, mut output: Self::Output) -> Self::Output {
        // Redact sensitive information from log messages
        for logs in output.logs.values_mut() {
            for entry in logs {
                // Redact IP addresses (keep first 3 octets)
                entry.message = entry.message
                    .split_whitespace()
                    .map(|word| {
                        if word.parse::<std::net::IpAddr>().is_ok() {
                            let parts: Vec<&str> = word.split('.').collect();
                            if parts.len() == 4 {
                                format!("{}.{}.{}.xxx", parts[0], parts[1], parts[2])
                            } else {
                                word.to_string()
                            }
                        } else {
                            word.to_string()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                
                // Redact potential passwords or tokens
                if entry.message.to_lowercase().contains("password") {
                    entry.message = entry.message
                        .chars()
                        .map(|c| if c.is_alphanumeric() { '*' } else { c })
                        .collect();
                }
            }
        }
        
        output
    }

    fn name(&self) -> &'static str {
        "logs"
    }

    fn validate_config(&self, _config: &Self::Config) -> CollectorResult<()> {
        // Log collection can work without any specific configuration
        Ok(())
    }
}
