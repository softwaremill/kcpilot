use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use tracing::{debug, info, warn};
use crate::llm::{LlmService, service::ChatMessage};

/// Enhanced log discovery following the process → systemd → config → logs chain
pub struct EnhancedLogDiscovery {
    bastion_alias: Option<String>,
    broker_hostname: Option<String>,
}

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

        // Step 1: Find Kafka Java process using ps aux
        info!("🔍 Step 1: Finding Kafka Java process...");
        match self.step1_find_kafka_process().await {
            Ok(process_info) => {
                info!("✅ Step 1: Found Kafka process (PID: {}, Service: {:?})", 
                      process_info.pid, process_info.service_name);
                info!("   Command: {}", process_info.command_line.chars().take(150).collect::<String>());
                info!("   Config paths: {:?}", process_info.config_paths);
                info!("   Log4j path: {:?}", process_info.log4j_path);
                result.discovery_steps.push("Found Kafka Java process via ps aux".to_string());
                
                // Step 2: Get systemd service name from PID
                info!("🔍 Step 2: Getting systemd service for PID {}...", process_info.pid);
                match self.step2_get_systemd_service(&process_info.pid).await {
                    Ok(systemd_info) => {
                        info!("✅ Step 2: Found systemd service: {}", systemd_info.service_name);
                        result.discovery_steps.push(format!("Retrieved systemd service: {}", systemd_info.service_name));
                        
                        // Step 3: Parse systemctl cat to get environment and config paths
                        info!("🔍 Step 3: Parsing systemd configuration...");
                        match self.step3_parse_systemd_config(&systemd_info).await {
                            Ok(enhanced_systemd) => {
                                info!("✅ Step 3: Parsed systemd configuration");
                                info!("   Environment file: {:?}", enhanced_systemd.environment_file);
                                info!("   Environment vars: {} found", enhanced_systemd.environment_vars.len());
                                for (key, value) in enhanced_systemd.environment_vars.iter().take(5) {
                                    info!("     {}={}", key, value);
                                }
                                result.discovery_steps.push("Parsed systemd service configuration".to_string());
                                
                                // Step 4: Use LLM to parse log4j and determine log destinations
                                if let Some(log4j_path) = &process_info.log4j_path {
                                    info!("🔍 Step 4: Analyzing log4j configuration with LLM...");
                                    info!("   Log4j path: {}", log4j_path.display());
                                    match self.step4_analyze_log4j_with_llm(log4j_path, &enhanced_systemd.environment_vars).await {
                                        Ok(log_output_info) => {
                                            info!("✅ Step 4: Analyzed log4j configuration with LLM");
                                            info!("   Found {} log files", log_output_info.log_files.len());
                                            info!("   Uses stdout: {}, Uses journald: {}", log_output_info.uses_stdout, log_output_info.uses_journald);
                                            for log_file in &log_output_info.log_files {
                                                info!("     {} -> {} ({})", log_file.appender_name, log_file.path.display(), log_file.log_type);
                                            }
                                            result.discovery_steps.push("Analyzed log4j with LLM for log destinations".to_string());
                                            
                                            // Step 5: Collect actual log content
                                            info!("🔍 Step 5: Collecting logs from discovered locations...");
                                            match self.step5_collect_logs(&log_output_info, &enhanced_systemd.service_name).await {
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
                                            result.warnings.push(format!("Failed to analyze log4j configuration with LLM: {}", e));
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
            // You can add fallback logic here if needed
        }

        info!("📊 Enhanced log discovery complete: {} steps, {} logs, {} warnings", 
              result.discovery_steps.len(), 
              result.discovered_logs.len(), 
              result.warnings.len());

        Ok(result)
    }

    /// Step 1: Find Kafka Java process using ps aux
    async fn step1_find_kafka_process(&self) -> Result<KafkaProcessInfo> {
        debug!("Step 1: Finding Kafka Java process...");
        
        // Look for Kafka main class in Java processes
        info!("   Trying primary pattern: java.*kafka\\.Kafka");
        let ps_output = self.execute("ps aux | grep -E 'java.*kafka\\.Kafka[^a-zA-Z]' | grep -v grep")?;
        
        if ps_output.trim().is_empty() {
            info!("   Primary pattern failed, trying alternatives...");
            // Try alternative patterns
            let alternative_patterns = [
                ("org.apache.kafka.Kafka", "ps aux | grep -E 'java.*org\\.apache\\.kafka\\.Kafka' | grep -v grep"),
                ("KafkaServerStart", "ps aux | grep -E 'java.*KafkaServerStart' | grep -v grep"),  
                ("kafka server start", "ps aux | grep -E 'kafka.*server.*start' | grep -v grep"),
                ("any java kafka", "ps aux | grep -E 'java.*kafka' | grep -v grep"),
            ];
            
            for (name, pattern) in &alternative_patterns {
                info!("   Trying pattern: {}", name);
                let alt_output = self.execute(pattern)?;
                if !alt_output.trim().is_empty() {
                    info!("   ✓ Found match with pattern: {}", name);
                    return self.parse_kafka_process_info(&alt_output).await;
                } else {
                    info!("   ✗ No match with pattern: {}", name);
                }
            }
            
            warn!("   All patterns failed. Let's see what Java processes are running:");
            let all_java = self.execute("ps aux | grep java | grep -v grep")?;
            if all_java.trim().is_empty() {
                warn!("   No Java processes found at all!");
            } else {
                warn!("   Found Java processes:");
                for (i, line) in all_java.lines().enumerate() {
                    if i < 5 { // Show first 5 lines
                        warn!("     {}", line);
                    }
                }
                if all_java.lines().count() > 5 {
                    warn!("     ... and {} more", all_java.lines().count() - 5);
                }
            }
            
            return Err(anyhow::anyhow!("No Kafka Java process found after trying all patterns"));
        }

        info!("   ✓ Found match with primary pattern");
        self.parse_kafka_process_info(&ps_output).await
    }

    /// Parse ps aux output to extract Kafka process information
    async fn parse_kafka_process_info(&self, ps_output: &str) -> Result<KafkaProcessInfo> {
        info!("   Parsing ps output...");
        debug!("   Raw ps output: {}", ps_output);
        
        let line = ps_output.lines().next().ok_or_else(|| anyhow::anyhow!("Empty ps output"))?;
        let parts: Vec<&str> = line.split_whitespace().collect();
        
        info!("   Split into {} parts", parts.len());
        if parts.len() < 11 {
            warn!("   Invalid ps output format - expected at least 11 parts, got {}", parts.len());
            warn!("   First 10 parts: {:?}", &parts[..parts.len().min(10)]);
            return Err(anyhow::anyhow!("Invalid ps output format"));
        }

        let pid: u32 = parts[1].parse().context("Failed to parse PID")?;
        info!("   Extracted PID: {}", pid);
        
        // Extract the full command line (everything after the first 10 columns)
        let command_start_idx = parts[0..10].iter().map(|s| s.len() + 1).sum::<usize>() - 1;
        let command_line = if command_start_idx < line.len() {
            line[command_start_idx..].to_string()
        } else {
            line.to_string()
        };
        info!("   Command line length: {} chars", command_line.len());

        // Extract configuration file paths from command line
        let config_paths = self.extract_config_paths_from_cmdline(&command_line);
        info!("   Found {} config paths: {:?}", config_paths.len(), config_paths);
        
        // Extract log4j path from command line
        let log4j_path = self.extract_log4j_path_from_cmdline(&command_line);
        info!("   Log4j path: {:?}", log4j_path);

        Ok(KafkaProcessInfo {
            pid,
            command_line,
            service_name: None, // Will be filled in step 2
            config_paths,
            log4j_path,
            environment_vars: HashMap::new(), // Will be filled later
        })
    }

    /// Extract configuration file paths from command line
    fn extract_config_paths_from_cmdline(&self, cmdline: &str) -> Vec<PathBuf> {
        let mut config_paths = Vec::new();
        let parts: Vec<&str> = cmdline.split_whitespace().collect();
        
        for part in &parts {
            if part.ends_with(".properties") || part.ends_with(".conf") {
                config_paths.push(PathBuf::from(part));
            }
        }
        
        config_paths
    }

    /// Extract log4j configuration path from command line
    fn extract_log4j_path_from_cmdline(&self, cmdline: &str) -> Option<PathBuf> {
        // Look for -Dlog4j.configuration or -Dlog4j2.configurationFile
        for part in cmdline.split_whitespace() {
            if part.contains("log4j.configuration=") {
                if let Some(eq_pos) = part.find('=') {
                    let path = &part[eq_pos + 1..];
                    let clean_path = path.strip_prefix("file:").unwrap_or(path);
                    return Some(PathBuf::from(clean_path));
                }
            }
            if part.contains("log4j2.configurationFile=") {
                if let Some(eq_pos) = part.find('=') {
                    let path = &part[eq_pos + 1..];
                    return Some(PathBuf::from(path));
                }
            }
        }
        None
    }

    /// Step 2: Get systemd service name from PID
    async fn step2_get_systemd_service(&self, pid: &u32) -> Result<SystemdServiceInfo> {
        debug!("Step 2: Getting systemd service for PID {}...", pid);
        
        // Use systemctl status PID to get service name
        let status_output = self.execute(&format!("systemctl status {} 2>/dev/null", pid))?;
        
        if status_output.trim().is_empty() {
            return Err(anyhow::anyhow!("Process {} is not managed by systemd", pid));
        }

        // Extract service name from the first line of systemctl status output
        let service_name = self.extract_service_name_from_status(&status_output)?;
        
        Ok(SystemdServiceInfo {
            service_name,
            service_content: String::new(), // Will be filled in step 3
            environment_file: None,
            environment_vars: HashMap::new(),
            exec_start: None,
        })
    }

    /// Extract service name from systemctl status output
    fn extract_service_name_from_status(&self, status_output: &str) -> Result<String> {
        // First line typically looks like: "● kafka.service - Apache Kafka Server"
        let first_line = status_output.lines().next()
            .ok_or_else(|| anyhow::anyhow!("Empty systemctl status output"))?;
        
        // Extract service name (between ● and the dash)
        if let Some(service_start) = first_line.find(' ') {
            let after_bullet = &first_line[service_start + 1..];
            if let Some(dash_pos) = after_bullet.find(" - ") {
                return Ok(after_bullet[..dash_pos].trim().to_string());
            }
        }
        
        Err(anyhow::anyhow!("Could not parse service name from status output"))
    }

    /// Step 3: Parse systemctl cat output to get environment and config paths
    async fn step3_parse_systemd_config(&self, systemd_info: &SystemdServiceInfo) -> Result<SystemdServiceInfo> {
        debug!("Step 3: Parsing systemd configuration for {}...", systemd_info.service_name);
        
        // Get full service configuration
        let service_content = self.execute(&format!("systemctl cat {} 2>/dev/null", systemd_info.service_name))?;
        
        if service_content.trim().is_empty() {
            return Err(anyhow::anyhow!("Could not get service configuration for {}", systemd_info.service_name));
        }

        let mut enhanced_info = systemd_info.clone();
        enhanced_info.service_content = service_content.clone();
        
        // Parse environment variables and files from service content
        let (environment_file, environment_vars, exec_start) = self.parse_systemd_service_content(&service_content);
        
        enhanced_info.environment_file = environment_file;
        enhanced_info.environment_vars = environment_vars;
        enhanced_info.exec_start = exec_start;
        
        // If there's an environment file, read it
        if let Some(ref env_file_path) = enhanced_info.environment_file {
            if let Ok(env_file_content) = self.execute(&format!("sudo cat '{}' 2>/dev/null", env_file_path.display())) {
                let env_file_vars = self.parse_environment_file(&env_file_content);
                enhanced_info.environment_vars.extend(env_file_vars);
            }
        }

        Ok(enhanced_info)
    }

    /// Parse systemd service content for environment info
    fn parse_systemd_service_content(&self, content: &str) -> (Option<PathBuf>, HashMap<String, String>, Option<String>) {
        let mut environment_file = None;
        let mut environment_vars = HashMap::new();
        let mut exec_start = None;
        
        for line in content.lines() {
            let line = line.trim();
            
            if line.starts_with("EnvironmentFile=") {
                let path = line[16..].trim_matches('"');
                environment_file = Some(PathBuf::from(path));
            } else if line.starts_with("Environment=") {
                let env_part = &line[12..];
                if let Some(eq_pos) = env_part.find('=') {
                    let key = env_part[..eq_pos].trim();
                    let value = env_part[eq_pos + 1..].trim_matches('"');
                    environment_vars.insert(key.to_string(), value.to_string());
                }
            } else if line.starts_with("ExecStart=") {
                exec_start = Some(line[10..].to_string());
            }
        }
        
        (environment_file, environment_vars, exec_start)
    }

    /// Parse environment file content
    fn parse_environment_file(&self, content: &str) -> HashMap<String, String> {
        let mut vars = HashMap::new();
        
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            
            if let Some(eq_pos) = line.find('=') {
                let key = line[..eq_pos].trim();
                let value = line[eq_pos + 1..].trim_matches('"').trim_matches('\'');
                vars.insert(key.to_string(), value.to_string());
            }
        }
        
        vars
    }

    /// Step 4: Use LLM to parse log4j and determine log output destinations
    async fn step4_analyze_log4j_with_llm(&self, log4j_path: &PathBuf, env_vars: &HashMap<String, String>) -> Result<LogOutputInfo> {
        debug!("Step 4: Analyzing log4j configuration with LLM...");
        
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

        // Prepare LLM prompt for log4j analysis
        let analysis_prompt = self.create_log4j_analysis_prompt(&log4j_content, env_vars);
        
        // Use the existing LLM service to analyze the log4j configuration  
        match LlmService::from_env() {
            Ok(llm_service) => {
                let messages = vec![ChatMessage::user(&analysis_prompt)];
                match llm_service.chat(messages).await {
                    Ok(llm_response) => {
                        match self.parse_llm_log4j_response(&llm_response) {
                            Ok(result) if result.log_files.is_empty() => {
                                // If LLM parsing succeeded but found no files, try basic parsing
                                warn!("LLM found no log files, trying basic parsing as fallback");
                                self.basic_log4j_parsing(&log4j_content, env_vars)
                            }
                            Ok(result) => Ok(result),
                            Err(e) => {
                                warn!("LLM response parsing failed: {}, falling back to basic parsing", e);
                                self.basic_log4j_parsing(&log4j_content, env_vars)
                            }
                        }
                    }
                    Err(e) => {
                        warn!("LLM analysis failed: {}, falling back to basic parsing", e);
                        self.basic_log4j_parsing(&log4j_content, env_vars)
                    }
                }
            }
            Err(e) => {
                warn!("LLM service not available: {}, using basic parsing", e);
                self.basic_log4j_parsing(&log4j_content, env_vars)
            }
        }
    }

    /// Create LLM prompt for log4j analysis
    fn create_log4j_analysis_prompt(&self, log4j_content: &str, env_vars: &HashMap<String, String>) -> String {
        let env_vars_str = env_vars.iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"Analyze this Kafka log4j configuration and determine where logs are written.

Environment Variables:
{}

Log4j Configuration:
{}

IMPORTANT: Respond with ONLY valid JSON, no explanations or additional text.

JSON Structure Required:
{{
  "log_files": [
    {{
      "path": "/resolved/path/to/logfile.log",
      "log_type": "server|controller|state-change|request|gc|custom",
      "appender_name": "appender_name"
    }}
  ],
  "uses_stdout": true/false,
  "uses_journald": true/false,
  "analysis": "Brief explanation of the log configuration"
}}

Requirements:
1. Resolve variable substitutions like ${{kafka.logs.dir}}, ${{LOG_DIR}} using environment variables
2. Identify if logs go to stdout/console (appears in journald)
3. Classify log types by appender names/file names
4. Use full resolved file paths
5. Return ONLY the JSON object, no other text"#,
            env_vars_str, log4j_content
        )
    }

    /// Parse LLM response for log4j analysis
    fn parse_llm_log4j_response(&self, llm_response: &str) -> Result<LogOutputInfo> {
        // Try multiple strategies to extract JSON from the LLM response
        let json_candidates = vec![
            // Strategy 1: Look for JSON code block
            self.extract_json_from_code_block(llm_response),
            // Strategy 2: Find the largest JSON object in the response
            self.extract_largest_json_object(llm_response),
            // Strategy 3: Original simple extraction
            self.extract_simple_json(llm_response),
        ];
        
        for json_str in json_candidates.into_iter().flatten() {
            if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&json_str) {
                return self.parse_json_to_log_output_info(json_value);
            }
        }
        
        // If none worked, return error with the full response for debugging
        Err(anyhow::anyhow!("Could not extract valid JSON from LLM response. Response was: {}", llm_response))
    }
    
    /// Extract JSON from code block (```json ... ```)
    fn extract_json_from_code_block(&self, response: &str) -> Option<String> {
        if let Some(start) = response.find("```json") {
            let start = start + 7; // Skip "```json"
            if let Some(end) = response[start..].find("```") {
                return Some(response[start..start + end].trim().to_string());
            }
        }
        None
    }
    
    /// Extract the largest JSON object from the response
    fn extract_largest_json_object(&self, response: &str) -> Option<String> {
        let mut best_json = None;
        let mut best_len = 0;
        
        // Find all potential JSON objects
        for (i, _) in response.match_indices('{') {
            let mut brace_count = 0;
            let mut end_pos = i;
            
            for (j, c) in response[i..].char_indices() {
                match c {
                    '{' => brace_count += 1,
                    '}' => {
                        brace_count -= 1;
                        if brace_count == 0 {
                            end_pos = i + j + 1;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            
            if brace_count == 0 {
                let json_candidate = &response[i..end_pos];
                if json_candidate.len() > best_len {
                    // Quick validation that it looks like proper JSON
                    if json_candidate.contains("log_files") && json_candidate.contains("uses_stdout") {
                        best_json = Some(json_candidate.to_string());
                        best_len = json_candidate.len();
                    }
                }
            }
        }
        
        best_json
    }
    
    /// Simple extraction (original method)
    fn extract_simple_json(&self, response: &str) -> Option<String> {
        let json_start = response.find('{')?;
        let json_end = response.rfind('}').map(|pos| pos + 1)?;
        Some(response[json_start..json_end].to_string())
    }
    
    /// Parse validated JSON into LogOutputInfo
    fn parse_json_to_log_output_info(&self, json_value: serde_json::Value) -> Result<LogOutputInfo> {
        let mut log_files = Vec::new();
        
        if let Some(files_array) = json_value.get("log_files").and_then(|v| v.as_array()) {
            for file_obj in files_array {
                if let (Some(path), Some(log_type), Some(appender)) = (
                    file_obj.get("path").and_then(|v| v.as_str()),
                    file_obj.get("log_type").and_then(|v| v.as_str()),
                    file_obj.get("appender_name").and_then(|v| v.as_str()),
                ) {
                    log_files.push(LogFileLocation {
                        path: PathBuf::from(path),
                        log_type: log_type.to_string(),
                        appender_name: appender.to_string(),
                    });
                }
            }
        }
        
        let uses_stdout = json_value.get("uses_stdout").and_then(|v| v.as_bool()).unwrap_or(false);
        let uses_journald = json_value.get("uses_journald").and_then(|v| v.as_bool()).unwrap_or(false);
        let analysis = json_value.get("analysis").and_then(|v| v.as_str()).unwrap_or("LLM analysis").to_string();
        
        Ok(LogOutputInfo {
            log_files,
            uses_stdout,
            uses_journald,
            log4j_analysis: analysis,
        })
    }

    /// Basic log4j parsing fallback when LLM is not available
    fn basic_log4j_parsing(&self, log4j_content: &str, env_vars: &HashMap<String, String>) -> Result<LogOutputInfo> {
        let mut log_files = Vec::new();
        let mut uses_stdout = false;
        
        for line in log4j_content.lines() {
            let line = line.trim();
            
            // Look for file appenders
            if line.contains("appender") && line.contains(".File=") {
                if let Some(eq_pos) = line.find('=') {
                    let mut file_path = line[eq_pos + 1..].trim().to_string();
                    
                    // Resolve environment variables
                    for (key, value) in env_vars {
                        file_path = file_path.replace(&format!("${{{}}}", key), value);
                        file_path = file_path.replace(&format!("${}", key), value);
                    }
                    
                    // Common fallback substitutions - try multiple likely locations
                    if file_path.contains("${kafka.logs.dir}") || file_path.contains("${log.dir}") {
                        // Try common Kafka log directory locations
                        let possible_log_dirs = vec![
                            "/opt/kafka/logs",     // Common for manual installs
                            "/var/log/kafka",      // Common for package installs  
                            "/usr/local/kafka/logs", // Alternative install location
                            "/home/kafka/logs",    // User-based installs
                        ];
                        
                        for log_dir in possible_log_dirs {
                            file_path = file_path.replace("${kafka.logs.dir}", log_dir);
                            file_path = file_path.replace("${log.dir}", log_dir);
                            break; // Use the first one for now - we could check which exists
                        }
                    }
                    
                    let log_type = if file_path.contains("server") {
                        "server"
                    } else if file_path.contains("controller") {
                        "controller"
                    } else if file_path.contains("state") {
                        "state-change"
                    } else {
                        "custom"
                    };
                    
                    log_files.push(LogFileLocation {
                        path: PathBuf::from(file_path),
                        log_type: log_type.to_string(),
                        appender_name: "detected".to_string(),
                    });
                }
            }
            
            // Check for console/stdout appenders
            if line.contains("ConsoleAppender") || line.contains("console") {
                uses_stdout = true;
            }
        }
        
        Ok(LogOutputInfo {
            log_files,
            uses_stdout,
            uses_journald: uses_stdout, // If using stdout, it goes to journald
            log4j_analysis: "Basic parsing (no LLM)".to_string(),
        })
    }

    /// Step 5: Collect actual logs based on the analysis
    async fn step5_collect_logs(&self, log_info: &LogOutputInfo, service_name: &str) -> Result<HashMap<String, String>> {
        info!("🔍 Step 5: Collecting logs from discovered locations...");
        let mut discovered_logs = HashMap::new();
        let mut successful_collections = 0;
        let mut failed_collections = 0;
        
        // Collect file-based logs
        for log_file in &log_info.log_files {
            let log_name = format!("{}_{}", log_file.log_type, log_file.path.file_name().unwrap_or_default().to_string_lossy());
            
            info!("   Trying to collect: {} -> {}", log_file.appender_name, log_file.path.display());
            
            // First try with error suppression
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
                Err(_) => {
                    // Try again without error suppression to see what the actual error is
                    match self.execute(&format!("sudo tail -500 '{}'", log_file.path.display())) {
                        Ok(content) => {
                            if !content.trim().is_empty() {
                                let lines = content.lines().count();
                                info!("   ✅ Collected {} (second attempt): {} lines", log_name, lines);
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
        
        info!("✅ Step 5: Collection summary - {} successful, {} failed", successful_collections, failed_collections);
        
        Ok(discovered_logs)
    }
}