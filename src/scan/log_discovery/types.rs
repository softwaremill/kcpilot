use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaProcessInfo {
    pub pid: u32,
    pub command_line: String,
    pub service_name: Option<String>,
    pub config_paths: Vec<PathBuf>,
    pub log4j_path: Option<PathBuf>,
    pub environment_vars: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemdServiceInfo {
    pub service_name: String,
    pub service_content: String,
    pub environment_file: Option<PathBuf>,
    pub environment_vars: HashMap<String, String>,
    pub exec_start: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogOutputInfo {
    pub log_files: Vec<LogFileLocation>,
    pub uses_stdout: bool,
    pub uses_journald: bool,
    pub log4j_analysis: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogFileLocation {
    pub path: PathBuf,
    pub log_type: String,
    pub appender_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedLogResult {
    pub process_info: Option<KafkaProcessInfo>,
    pub systemd_info: Option<SystemdServiceInfo>,
    pub log_output_info: Option<LogOutputInfo>,
    pub discovered_logs: HashMap<String, String>,
    pub discovery_steps: Vec<String>,
    pub warnings: Vec<String>,
}