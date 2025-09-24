use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use tracing::{debug, info, warn};

use super::log_discovery::{
    EnhancedLogResult, LogOutputInfo,
    process_parser::ProcessParser,
    systemd_parser::SystemdParser,
    llm_log_analyzer::LlmLogAnalyzer,
};

/// Enhanced log discovery following the process → systemd → config → logs chain
pub struct EnhancedLogDiscovery {
    bastion_alias: Option<String>,
    broker_hostname: Option<String>,
}

impl EnhancedLogDiscovery {
    pub fn new(ssh_executor: Option<String>) -> Self {
        // Parse the SSH executor string to extract bastion and broker info
        match ssh_executor {
            Some(ssh_str) if ssh_str.contains("ssh") => {
                // Format: "bastion ssh -o StrictHostKeyChecking=no broker"
                let parts: Vec<&str> = ssh_str.split_whitespace().collect();
                if parts.len() >= 4 && parts[1] == "ssh" {
                    Self {
                        bastion_alias: Some(parts[0].to_string()),
                        broker_hostname: Some(parts[parts.len() - 1].to_string()),
                    }
                } else {
                    Self {
                        bastion_alias: None,
                        broker_hostname: Some(ssh_str),
                    }
                }
            }
            Some(hostname) => {
                Self {
                    bastion_alias: None,
                    broker_hostname: Some(hostname),
                }
            }
            None => {
                Self {
                    bastion_alias: None,
                    broker_hostname: None,
                }
            }
        }
    }

    /// Execute command either locally or via SSH
    fn execute(&self, command: &str) -> Result<String> {
        debug!("🔧 Executing command: {}", command);
        
        let output = match (&self.bastion_alias, &self.broker_hostname) {
            (Some(bastion), Some(broker)) => {
                debug!("   → via SSH chain: {} -> {}", bastion, broker);
                // SSH to bastion, then SSH to broker
                let ssh_chain_command = format!("ssh -o StrictHostKeyChecking=no {} '{}'", broker, command);
                Command::new("ssh")
                    .arg("-A") // Enable agent forwarding
                    .arg("-o")
                    .arg("StrictHostKeyChecking=no")
                    .arg(bastion)
                    .arg(&ssh_chain_command)
                    .output()
                    .context(format!("Failed to execute via SSH chain: {}", command))?
            }
            (None, Some(broker)) => {
                debug!("   → via direct SSH to: {}", broker);
                Command::new("ssh")
                    .arg("-o")
                    .arg("StrictHostKeyChecking=no")
                    .arg(broker)
                    .arg(command)
                    .output()
                    .context(format!("Failed to execute via SSH: {}", command))?
            }
            (None, None) => {
                debug!("   → locally");
                Command::new("sh")
                    .arg("-c")
                    .arg(command)
                    .output()
                    .context(format!("Failed to execute locally: {}", command))?
            }
            (Some(_), None) => {
                return Err(anyhow::anyhow!("Invalid SSH configuration: bastion specified but no broker hostname"));
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        
        if !output.status.success() {
            warn!("❌ Command failed with exit code: {}", output.status.code().unwrap_or(-1));
            warn!("   Command: {}", command);
            if !stderr.is_empty() {
                warn!("   Stderr: {}", stderr.trim());
            }
            if !stdout.is_empty() {
                warn!("   Stdout: {}", stdout.trim());
            }
            return Ok(String::new()); // Return empty instead of error for optional steps
        }

        debug!("✅ Command succeeded");
        if !stdout.is_empty() {
            debug!("   Output length: {} chars", stdout.len());
            debug!("   First 200 chars: {}", stdout.chars().take(200).collect::<String>());
        } else {
            debug!("   No output");
        }

        Ok(stdout)
    }

    /// Main discovery method following the process → systemd → config → logs chain
    pub async fn discover_logs(&self) -> Result<EnhancedLogResult> {
        info!("🔍 Starting enhanced log discovery chain...");
        
        let mut result = EnhancedLogResult {
            process_info: None,
            systemd_info: None,
            log_output_info: None,
            discovered_logs: HashMap::new(),
            discovery_steps: Vec::new(),
            warnings: Vec::new(),
        };

        // Create a closure that captures self for execution
        let executor = |cmd: &str| self.execute(cmd);

        // Step 1: Find Kafka Java process
        info!("🔍 Step 1: Finding Kafka Java process...");
        match ProcessParser::find_kafka_process(executor).await {
            Ok(process_info) => {
                info!("✅ Step 1: Found Kafka process (PID: {}, Service: {:?})", 
                      process_info.pid, process_info.service_name);
                info!("   Command: {}", process_info.command_line.chars().take(150).collect::<String>());
                info!("   Config paths: {:?}", process_info.config_paths);
                info!("   Log4j path: {:?}", process_info.log4j_path);
                result.discovery_steps.push("Found Kafka Java process via ps aux".to_string());
                
                // Step 2: Get systemd service name from PID
                info!("🔍 Step 2: Getting systemd service for PID {}...", process_info.pid);
                let executor = |cmd: &str| self.execute(cmd);
                match SystemdParser::get_service_from_pid(executor, process_info.pid).await {
                    Ok(systemd_info) => {
                        info!("✅ Step 2: Found systemd service: {}", systemd_info.service_name);
                        result.discovery_steps.push(format!("Retrieved systemd service: {}", systemd_info.service_name));
                        
                        // Step 3: Parse systemctl cat to get environment and config paths
                        info!("🔍 Step 3: Parsing systemd configuration...");
                        let executor = |cmd: &str| self.execute(cmd);
                        match SystemdParser::parse_service_config(executor, &systemd_info.service_name).await {
                            Ok(enhanced_systemd) => {
                                info!("✅ Step 3: Parsed systemd configuration");
                                info!("   Environment file: {:?}", enhanced_systemd.environment_file);
                                info!("   Environment vars: {} found", enhanced_systemd.environment_vars.len());
                                for (key, value) in enhanced_systemd.environment_vars.iter().take(5) {
                                    info!("     {}={}", key, value);
                                }
                                result.discovery_steps.push("Parsed systemd service configuration".to_string());
                                
                                // Step 4: Analyze log4j configuration
                                if let Some(log4j_path) = &process_info.log4j_path {
                                    info!("🔍 Step 4: Analyzing log4j configuration...");
                                    info!("   Log4j path: {}", log4j_path.display());
                                    match self.analyze_log4j(log4j_path, &enhanced_systemd.environment_vars).await {
                                        Ok(log_output_info) => {
                                            info!("✅ Step 4: Analyzed log4j configuration");
                                            info!("   Found {} log files", log_output_info.log_files.len());
                                            info!("   Uses stdout: {}, Uses journald: {}", log_output_info.uses_stdout, log_output_info.uses_journald);
                                            for log_file in &log_output_info.log_files {
                                                info!("     {} -> {} ({})", log_file.appender_name, log_file.path.display(), log_file.log_type);
                                            }
                                            result.discovery_steps.push("Analyzed log4j for log destinations".to_string());
                                            
                                            // Step 5: Collect actual log content
                                            info!("🔍 Step 5: Collecting logs from discovered locations...");
                                            match self.collect_logs(&log_output_info, &enhanced_systemd.service_name).await {
                                                Ok(logs) => {
                                                    info!("✅ Step 5: Collected {} log sources", logs.len());
                                                    for (name, content) in &logs {
                                                        info!("     {}: {} chars", name, content.len());
                                                    }
                                                    result.discovered_logs = logs;
                                                    result.log_output_info = Some(log_output_info);
                                                    result.discovery_steps.push(format!("Collected logs from {} sources", result.discovered_logs.len()));
                                                }
                                                Err(e) => {
                                                    warn!("❌ Step 5 failed: {}", e);
                                                    result.warnings.push(format!("Failed to collect logs: {}", e));
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            warn!("❌ Step 4 failed: {}", e);
                                            result.warnings.push(format!("Failed to analyze log4j configuration: {}", e));
                                        }
                                    }
                                } else {
                                    warn!("⚠️  No log4j configuration path found in process arguments");
                                    result.warnings.push("No log4j configuration path found".to_string());
                                }
                                
                                result.systemd_info = Some(enhanced_systemd);
                            }
                            Err(e) => {
                                warn!("❌ Step 3 failed: {}", e);
                                result.warnings.push(format!("Failed to parse systemd configuration: {}", e));
                            }
                        }
                    }
                    Err(e) => {
                        warn!("❌ Step 2 failed: {}", e);
                        result.warnings.push(format!("Failed to get systemd service information: {}", e));
                    }
                }
                
                result.process_info = Some(process_info);
            }
            Err(e) => {
                warn!("❌ Step 1 failed: {}", e);
                result.warnings.push(format!("Failed to find Kafka Java process: {}", e));
            }
        }

        if result.discovered_logs.is_empty() {
            warn!("Enhanced discovery failed, falling back to basic methods");
            result.warnings.push("Falling back to basic discovery methods".to_string());
        }

        info!("📊 Enhanced log discovery complete: {} steps, {} logs, {} warnings", 
              result.discovery_steps.len(), 
              result.discovered_logs.len(), 
              result.warnings.len());

        Ok(result)
    }

    /// Analyze log4j configuration to determine log destinations
    async fn analyze_log4j(&self, log4j_path: &Path, env_vars: &HashMap<String, String>) -> Result<LogOutputInfo> {
        debug!("Analyzing log4j configuration...");
        
        // Clean the log4j path - remove file: prefix if present
        let path_str = log4j_path.to_string_lossy();
        let clean_path = if path_str.starts_with("file:") {
            path_str.strip_prefix("file:").unwrap_or(&path_str).to_string()
        } else {
            path_str.to_string()
        };
        
        info!("   Reading log4j config from: {}", clean_path);
        
        // Read log4j configuration file using sudo
        let log4j_content = self.execute(&format!("sudo cat '{}' 2>/dev/null", clean_path))?;
        
        if log4j_content.trim().is_empty() {
            return Err(anyhow::anyhow!("Could not read log4j configuration file"));
        }

        // Try LLM analysis first, fall back to basic parsing
        LlmLogAnalyzer::analyze(&log4j_content, env_vars).await
    }

    /// Collect actual logs based on the analysis
    async fn collect_logs(&self, log_info: &LogOutputInfo, service_name: &str) -> Result<HashMap<String, String>> {
        info!("Collecting logs from discovered locations...");
        let mut discovered_logs = HashMap::new();
        let mut successful_collections = 0;
        let mut failed_collections = 0;
        
        // Collect file-based logs
        for log_file in &log_info.log_files {
            let log_name = format!("{}_{}", log_file.log_type, log_file.path.file_name().unwrap_or_default().to_string_lossy());
            
            info!("   Trying to collect: {} -> {}", log_file.appender_name, log_file.path.display());
            
            // Try to collect log file content
            match self.execute(&format!("sudo tail -500 '{}' 2>/dev/null", log_file.path.display())) {
                Ok(content) => {
                    if !content.trim().is_empty() {
                        let lines = content.lines().count();
                        info!("   ✅ Collected {}: {} lines", log_name, lines);
                        discovered_logs.insert(log_name, content);
                        successful_collections += 1;
                    } else {
                        info!("   ⚠️  File exists but is empty: {}", log_file.path.display());
                    }
                }
                Err(e) => {
                    info!("   ❌ Failed to collect {}: {}", log_file.path.display(), e);
                    failed_collections += 1;
                }
            }
        }
        
        // Collect journald logs if needed
        if log_info.uses_journald || log_info.uses_stdout {
            info!("   Trying to collect journald logs for service: {}", service_name);
            match self.execute(&format!("journalctl -u {} -n 500 --no-pager 2>/dev/null", service_name)) {
                Ok(journald_content) => {
                    if !journald_content.trim().is_empty() {
                        let lines = journald_content.lines().count();
                        info!("   ✅ Collected journald logs: {} lines", lines);
                        discovered_logs.insert("journald_service_logs".to_string(), journald_content);
                        successful_collections += 1;
                    } else {
                        info!("   ⚠️  No journald logs available");
                    }
                }
                Err(e) => {
                    info!("   ❌ Failed to collect journald logs: {}", e);
                    failed_collections += 1;
                }
            }
        }
        
        info!("✅ Collection summary - {} successful, {} failed", successful_collections, failed_collections);
        
        Ok(discovered_logs)
    }
}