use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use tracing::{debug, info, warn};


/*
ps aux - znajdz jave  i tam komende do kafki i do zookipera, tam są np parametry do tego gdzie jest server.properties
zwykle jest to po kafka.Kafka

z tego mamy pid'a i potem
systemctl status pid
z jakiego serwisu systemd uruchomiona jest kafka, znamy nazwe serwisu

systemctl cat service_name
i tu mamy całąkonfituracje, jak jest startowana

jezeli mamy environment file lub environment to warto go przeanalizować

z tego czytamy wszystkie envy i wiem gdzie jest server.properties i gdzie jest log4j.properties

na tym log4j robimy cata i tam mamy dokladnie gdzie są logi

jak jest na stdout pisane to sobie wtedy po serwisie
journalctl -u nazwa_serwisu
 */
/// Dynamic log discovery system for Kafka brokers
pub struct LogDiscovery {
    ssh_executor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredLogs {
    /// Log files discovered and their sources
    pub log_files: HashMap<String, LogFileInfo>,
    /// Configuration files that define logging
    pub log_configs: HashMap<String, String>,
    /// Detection methods used successfully
    pub detection_methods: Vec<String>,
    /// Any warnings or issues encountered
    pub warnings: Vec<String>,
    /// Kafka process information
    pub process_info: Option<KafkaProcessInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogFileInfo {
    /// Full path to the log file
    pub path: PathBuf,
    /// How this log was discovered
    pub discovery_method: String,
    /// Log type (server, controller, etc.)
    pub log_type: LogType,
    /// Whether the file exists and is readable
    pub accessible: bool,
    /// File size in bytes (if accessible)
    pub size_bytes: Option<u64>,
    /// Last modified timestamp
    pub last_modified: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogType {
    Server,
    Controller,
    StateChange,
    Request,
    Gc,
    Custom(String),
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaProcessInfo {
    /// Process ID
    pub pid: u32,
    /// Command line arguments
    pub cmdline: Vec<String>,
    /// Environment variables
    pub env_vars: HashMap<String, String>,
    /// Working directory
    pub cwd: Option<PathBuf>,
    /// JVM arguments
    pub jvm_args: Vec<String>,
    /// Log4j configuration path
    pub log4j_config: Option<PathBuf>,
}

impl LogDiscovery {
    pub fn new(ssh_executor: Option<String>) -> Self {
        Self { ssh_executor }
    }

    /// Execute command either locally or via SSH
    fn execute(&self, command: &str) -> Result<String> {
        let output = match &self.ssh_executor {
            Some(ssh_host) => {
                Command::new("ssh")
                    .arg("-o")
                    .arg("StrictHostKeyChecking=no")
                    .arg(ssh_host)
                    .arg(command)
                    .output()
                    .context(format!("Failed to execute via SSH: {}", command))?
            }
            None => {
                Command::new("sh")
                    .arg("-c")
                    .arg(command)
                    .output()
                    .context(format!("Failed to execute locally: {}", command))?
            }
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            debug!("Command failed: {} - Error: {}", command, stderr);
            return Ok(String::new()); // Return empty instead of error for optional detection
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Discover all Kafka logs using multiple detection methods
    pub async fn discover_logs(&self) -> Result<DiscoveredLogs> {
        info!("🔍 Starting dynamic log discovery...");
        
        let mut discovered = DiscoveredLogs {
            log_files: HashMap::new(),
            log_configs: HashMap::new(),
            detection_methods: Vec::new(),
            warnings: Vec::new(),
            process_info: None,
        };

        // Method 1: Process-based detection
        if let Ok(process_info) = self.discover_from_process().await {
            info!("✓ Process-based detection successful");
            discovered.detection_methods.push("process_analysis".to_string());
            discovered.process_info = Some(process_info.clone());
            
            // Extract logs from process information
            self.extract_logs_from_process(&process_info, &mut discovered).await?;
        } else {
            discovered.warnings.push("Process-based detection failed".to_string());
        }

        // Method 2: Systemd service discovery
        if let Ok(()) = self.discover_from_systemd(&mut discovered).await {
            info!("✓ Systemd service detection successful");
            discovered.detection_methods.push("systemd_service".to_string());
        } else {
            discovered.warnings.push("Systemd service detection failed".to_string());
        }

        // Method 3: Configuration file parsing
        if let Ok(()) = self.discover_from_config_files(&mut discovered).await {
            info!("✓ Configuration file parsing successful");
            discovered.detection_methods.push("config_parsing".to_string());
        } else {
            discovered.warnings.push("Configuration file parsing had issues".to_string());
        }

        // Method 4: Filesystem search (fallback)
        if discovered.log_files.is_empty() {
            warn!("No logs found via primary methods, falling back to filesystem search");
            if let Ok(()) = self.discover_from_filesystem(&mut discovered).await {
                info!("✓ Filesystem search found logs");
                discovered.detection_methods.push("filesystem_search".to_string());
            } else {
                discovered.warnings.push("Filesystem search failed".to_string());
            }
        }

        // Method 5: Standard locations as last resort
        if discovered.log_files.is_empty() {
            warn!("All detection methods failed, trying standard locations");
            self.discover_standard_locations(&mut discovered).await?;
            discovered.detection_methods.push("standard_locations".to_string());
        }

        // Validate discovered log files
        self.validate_log_files(&mut discovered).await?;

        info!("📊 Log discovery complete:");
        info!("  • Found {} log files", discovered.log_files.len());
        info!("  • Used methods: {}", discovered.detection_methods.join(", "));
        if !discovered.warnings.is_empty() {
            info!("  • Warnings: {}", discovered.warnings.len());
        }

        Ok(discovered)
    }

    /// Discover Kafka process and extract information
    async fn discover_from_process(&self) -> Result<KafkaProcessInfo> {
        debug!("Discovering Kafka process...");

        // Find Kafka main process
        let ps_output = self.execute("ps aux | grep -E 'kafka\\.Kafka[^a-zA-Z]' | grep -v grep")?;
        if ps_output.trim().is_empty() {
            // Try alternative patterns
            let alt_patterns = [
                "ps aux | grep -E 'org\\.apache\\.kafka\\.Kafka' | grep -v grep",
                "ps aux | grep -E 'kafka.*server.*start' | grep -v grep",
                "ps aux | grep -E 'kafka.*broker' | grep -v grep",
            ];
            
            for pattern in &alt_patterns {
                let output = self.execute(pattern)?;
                if !output.trim().is_empty() {
                    return self.parse_process_info(&output).await;
                }
            }
            
            return Err(anyhow::anyhow!("No Kafka process found"));
        }

        self.parse_process_info(&ps_output).await
    }

    /// Parse process information from ps output
    async fn parse_process_info(&self, ps_output: &str) -> Result<KafkaProcessInfo> {
        let line = ps_output.lines().next().unwrap_or("").trim();
        let parts: Vec<&str> = line.split_whitespace().collect();
        
        if parts.len() < 11 {
            return Err(anyhow::anyhow!("Invalid ps output format"));
        }

        let pid: u32 = parts[1].parse().context("Failed to parse PID")?;
        debug!("Found Kafka process with PID: {}", pid);

        // Get detailed process information
        let cmdline = self.get_process_cmdline(pid).await?;
        let env_vars = self.get_process_env(pid).await?;
        let cwd = self.get_process_cwd(pid).await.ok();

        // Extract JVM arguments and log4j config
        let jvm_args = self.extract_jvm_args(&cmdline);
        let log4j_config = self.extract_log4j_config(&jvm_args, &env_vars);

        Ok(KafkaProcessInfo {
            pid,
            cmdline,
            env_vars,
            cwd,
            jvm_args,
            log4j_config,
        })
    }

    /// Get process command line from /proc/PID/cmdline
    async fn get_process_cmdline(&self, pid: u32) -> Result<Vec<String>> {
        let cmdline_output = self.execute(&format!("cat /proc/{}/cmdline 2>/dev/null | tr '\\0' '\\n'", pid))?;
        if cmdline_output.is_empty() {
            // Fallback to ps
            let ps_output = self.execute(&format!("ps -p {} -o args= 2>/dev/null", pid))?;
            return Ok(ps_output.split_whitespace().map(|s| s.to_string()).collect());
        }
        
        Ok(cmdline_output.lines().map(|s| s.to_string()).filter(|s| !s.is_empty()).collect())
    }

    /// Get process environment from /proc/PID/environ
    async fn get_process_env(&self, pid: u32) -> Result<HashMap<String, String>> {
        let env_output = self.execute(&format!("cat /proc/{}/environ 2>/dev/null | tr '\\0' '\\n'", pid))?;
        let mut env_vars = HashMap::new();
        
        for line in env_output.lines() {
            if let Some(eq_pos) = line.find('=') {
                let key = line[..eq_pos].to_string();
                let value = line[eq_pos + 1..].to_string();
                env_vars.insert(key, value);
            }
        }
        
        Ok(env_vars)
    }

    /// Get process working directory
    async fn get_process_cwd(&self, pid: u32) -> Result<PathBuf> {
        let cwd_output = self.execute(&format!("readlink /proc/{}/cwd 2>/dev/null", pid))?;
        Ok(PathBuf::from(cwd_output.trim()))
    }

    /// Extract JVM arguments from command line
    pub fn extract_jvm_args(&self, cmdline: &[String]) -> Vec<String> {
        cmdline
            .iter()
            .filter(|arg| arg.starts_with('-') && (arg.contains("log") || arg.starts_with("-D") || arg.starts_with("-X")))
            .cloned()
            .collect()
    }

    /// Extract log4j configuration path from JVM arguments
    pub fn extract_log4j_config(&self, jvm_args: &[String], env_vars: &HashMap<String, String>) -> Option<PathBuf> {
        // Look for log4j configuration in JVM args
        for arg in jvm_args {
            if arg.contains("log4j.configuration") {
                if let Some(eq_pos) = arg.find('=') {
                    let config_path = &arg[eq_pos + 1..];
                    // Remove file:// prefix if present
                    let path = config_path.strip_prefix("file://").unwrap_or(config_path);
                    return Some(PathBuf::from(path));
                }
            }
            if arg.contains("log4j2.configurationFile") {
                if let Some(eq_pos) = arg.find('=') {
                    let config_path = &arg[eq_pos + 1..];
                    return Some(PathBuf::from(config_path));
                }
            }
        }

        // Check environment variables
        if let Some(log4j_config) = env_vars.get("LOG4J_CONFIGURATION_FILE") {
            return Some(PathBuf::from(log4j_config));
        }
        
        None
    }

    /// Extract logs from process information
    async fn extract_logs_from_process(&self, process_info: &KafkaProcessInfo, discovered: &mut DiscoveredLogs) -> Result<()> {
        // Parse log4j configuration if available
        if let Some(log4j_path) = &process_info.log4j_config {
            if let Ok(()) = self.parse_log4j_config(log4j_path, discovered).await {
                debug!("Successfully parsed log4j config: {:?}", log4j_path);
            }
        }

        // Look for log directory in JVM args
        for arg in &process_info.jvm_args {
            if arg.contains("kafka.logs.dir") || arg.contains("log.dirs") {
                if let Some(eq_pos) = arg.find('=') {
                    let log_dir = PathBuf::from(&arg[eq_pos + 1..]);
                    self.discover_logs_in_directory(&log_dir, "jvm_args", discovered).await?;
                }
            }
        }

        // Check environment variables for log paths
        for (key, value) in &process_info.env_vars {
            if key.contains("LOG") && (key.contains("DIR") || key.contains("PATH")) {
                let log_path = PathBuf::from(value);
                if log_path.exists() || self.execute(&format!("test -e '{}'", log_path.display())).is_ok() {
                    if log_path.is_dir() || self.execute(&format!("test -d '{}'", log_path.display())).is_ok() {
                        self.discover_logs_in_directory(&log_path, "env_vars", discovered).await?;
                    } else {
                        self.add_log_file(log_path, "env_vars", LogType::Unknown, discovered).await?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Discover logs from systemd service
    async fn discover_from_systemd(&self, discovered: &mut DiscoveredLogs) -> Result<()> {
        debug!("Discovering from systemd service...");

        // Try different service names
        let service_names = ["kafka", "kafka.service", "confluent-kafka", "apache-kafka"];
        
        for service_name in &service_names {
            // Get service status and properties
            if let Ok(status_output) = self.execute(&format!("systemctl show {} 2>/dev/null", service_name)) {
                if status_output.contains("ActiveState=active") || status_output.contains("LoadState=loaded") {
                    debug!("Found active systemd service: {}", service_name);
                    
                    // Extract ExecStart to get command line
                    for line in status_output.lines() {
                        if line.starts_with("ExecStart=") {
                            let exec_start = &line[10..]; // Remove "ExecStart="
                            if let Ok(()) = self.parse_systemd_exec_start(exec_start, discovered).await {
                                return Ok(());
                            }
                        }
                    }
                    
                    // Also check service file directly
                    if let Ok(()) = self.parse_systemd_service_file(service_name, discovered).await {
                        return Ok(());
                    }
                }
            }
        }

        Err(anyhow::anyhow!("No systemd service found"))
    }

    /// Parse systemd ExecStart command
    async fn parse_systemd_exec_start(&self, exec_start: &str, discovered: &mut DiscoveredLogs) -> Result<()> {
        // ExecStart format: { path=/usr/bin/kafka-server-start ; argv[]=/usr/bin/kafka-server-start /etc/kafka/server.properties }
        if let Some(argv_start) = exec_start.find("argv[]=") {
            let argv_part = &exec_start[argv_start + 7..];
            let args: Vec<&str> = argv_part.split_whitespace().collect();
            
            // Look for config files in arguments
            for arg in args {
                if arg.ends_with(".properties") || arg.contains("log4j") {
                    let config_path = PathBuf::from(arg);
                    if let Ok(content) = self.execute(&format!("cat '{}' 2>/dev/null", config_path.display())) {
                        discovered.log_configs.insert(config_path.display().to_string(), content);
                        self.parse_kafka_config(&config_path, discovered).await?;
                    }
                }
            }
        }
        
        Ok(())
    }

    /// Parse systemd service file
    async fn parse_systemd_service_file(&self, service_name: &str, discovered: &mut DiscoveredLogs) -> Result<()> {
        let service_paths = [
            format!("/etc/systemd/system/{}", service_name),
            format!("/lib/systemd/system/{}", service_name),
            format!("/usr/lib/systemd/system/{}", service_name),
        ];

        for service_path in &service_paths {
            if let Ok(content) = self.execute(&format!("cat '{}' 2>/dev/null", service_path)) {
                discovered.log_configs.insert(service_path.clone(), content.clone());
                
                // Parse Environment and ExecStart lines
                for line in content.lines() {
                    let line = line.trim();
                    if line.starts_with("Environment=") {
                        let env_part = &line[12..];
                        if env_part.contains("LOG") {
                            // Extract log paths from environment
                            if let Some(eq_pos) = env_part.find('=') {
                                let log_path = PathBuf::from(&env_part[eq_pos + 1..]);
                                self.discover_logs_in_directory(&log_path, "systemd_env", discovered).await?;
                            }
                        }
                    } else if line.starts_with("ExecStart=") {
                        let exec_start = &line[10..];
                        self.parse_systemd_exec_start(exec_start, discovered).await?;
                    }
                }
                
                return Ok(());
            }
        }

        Err(anyhow::anyhow!("Service file not found"))
    }

    /// Discover from configuration files
    async fn discover_from_config_files(&self, discovered: &mut DiscoveredLogs) -> Result<()> {
        debug!("Discovering from configuration files...");

        // Common configuration paths to try
        let config_paths = [
            "/etc/kafka/server.properties",
            "/opt/kafka/config/server.properties",
            "/usr/local/kafka/config/server.properties",
            "/usr/hdp/current/kafka-broker/conf/server.properties",
            "/opt/confluent/etc/kafka/server.properties",
        ];

        for config_path in &config_paths {
            let path = PathBuf::from(config_path);
            if let Ok(()) = self.parse_kafka_config(&path, discovered).await {
                debug!("Successfully parsed config: {:?}", path);
            }
        }

        // Also try to find config files dynamically
        let find_configs = [
            "find /etc -name 'server.properties' 2>/dev/null",
            "find /opt -name 'server.properties' 2>/dev/null",
            "find /usr -name 'server.properties' 2>/dev/null",
        ];

        for find_cmd in &find_configs {
            if let Ok(output) = self.execute(find_cmd) {
                for line in output.lines() {
                    let config_path = PathBuf::from(line.trim());
                    if let Ok(()) = self.parse_kafka_config(&config_path, discovered).await {
                        debug!("Found and parsed config via find: {:?}", config_path);
                    }
                }
            }
        }

        Ok(())
    }

    /// Parse Kafka server.properties configuration
    async fn parse_kafka_config(&self, config_path: &PathBuf, discovered: &mut DiscoveredLogs) -> Result<()> {
        let content = self.execute(&format!("cat '{}' 2>/dev/null", config_path.display()))?;
        if content.is_empty() {
            return Ok(());
        }

        discovered.log_configs.insert(config_path.display().to_string(), content.clone());

        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.is_empty() {
                continue;
            }

            if let Some(eq_pos) = line.find('=') {
                let key = line[..eq_pos].trim();
                let value = line[eq_pos + 1..].trim();

                match key {
                    "log.dirs" => {
                        // Kafka data directories (not log files, but good to know)
                        for dir in value.split(',') {
                            let log_dir = PathBuf::from(dir.trim());
                            self.discover_logs_in_directory(&log_dir, "kafka_config", discovered).await?;
                        }
                    }
                    "log4j.rootLogger" | "log4j.appender.kafkaAppender.File" => {
                        // Direct log file reference
                        if key.contains("File") && !value.contains("${") {
                            let log_path = PathBuf::from(value);
                            self.add_log_file(log_path, "kafka_config", LogType::Server, discovered).await?;
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }

    /// Parse log4j configuration file
    async fn parse_log4j_config(&self, log4j_path: &PathBuf, discovered: &mut DiscoveredLogs) -> Result<()> {
        let content = self.execute(&format!("cat '{}' 2>/dev/null", log4j_path.display()))?;
        if content.is_empty() {
            return Ok(());
        }

        discovered.log_configs.insert(log4j_path.display().to_string(), content.clone());

        // Parse log4j.properties format
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.is_empty() {
                continue;
            }

            if let Some(eq_pos) = line.find('=') {
                let key = line[..eq_pos].trim();
                let value = line[eq_pos + 1..].trim();

                if key.contains("appender") && key.ends_with(".File") {
                    // Extract log file path, handling variable substitution
                    let log_path = self.resolve_log4j_path(value);
                    let log_type = self.determine_log_type_from_appender(key);
                    self.add_log_file(log_path, "log4j_config", log_type, discovered).await?;
                }
            }
        }

        Ok(())
    }

    /// Resolve log4j path with variable substitution
    pub fn resolve_log4j_path(&self, path: &str) -> PathBuf {
        let mut resolved = path.to_string();
        
        // Common variable substitutions
        resolved = resolved.replace("${kafka.logs.dir}", "/var/log/kafka");
        resolved = resolved.replace("${log.dir}", "/var/log/kafka");
        resolved = resolved.replace("${user.home}", "/home/kafka");
        
        PathBuf::from(resolved)
    }

    /// Determine log type from log4j appender name
    pub fn determine_log_type_from_appender(&self, appender_key: &str) -> LogType {
        let lower = appender_key.to_lowercase();
        if lower.contains("server") {
            LogType::Server
        } else if lower.contains("controller") {
            LogType::Controller
        } else if lower.contains("state") || lower.contains("change") {
            LogType::StateChange
        } else if lower.contains("request") {
            LogType::Request
        } else if lower.contains("gc") {
            LogType::Gc
        } else {
            LogType::Custom(appender_key.to_string())
        }
    }

    /// Discover logs via filesystem search (fallback method)
    async fn discover_from_filesystem(&self, discovered: &mut DiscoveredLogs) -> Result<()> {
        debug!("Discovering from filesystem search...");

        let search_commands = [
            "find /var/log -name '*kafka*' -type f 2>/dev/null",
            "find /opt -name '*kafka*.log' -type f 2>/dev/null",
            "find /usr/local -name '*kafka*.log' -type f 2>/dev/null",
            "find /home -name '*kafka*.log' -type f 2>/dev/null",
            "locate kafka.log 2>/dev/null | head -20",
            "locate server.log 2>/dev/null | grep kafka | head -10",
        ];

        for search_cmd in &search_commands {
            if let Ok(output) = self.execute(search_cmd) {
                for line in output.lines() {
                    let log_path = PathBuf::from(line.trim());
                    if !log_path.as_os_str().is_empty() {
                        let log_type = self.determine_log_type_from_path(&log_path);
                        self.add_log_file(log_path, "filesystem_search", log_type, discovered).await?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Discover in standard locations (last resort)
    pub async fn discover_standard_locations(&self, discovered: &mut DiscoveredLogs) -> Result<()> {
        debug!("Trying standard locations as last resort...");

        let standard_paths = [
            ("/var/log/kafka/server.log", LogType::Server),
            ("/var/log/kafka/controller.log", LogType::Controller),
            ("/var/log/kafka/state-change.log", LogType::StateChange),
            ("/opt/kafka/logs/server.log", LogType::Server),
            ("/usr/local/kafka/logs/server.log", LogType::Server),
        ];

        for (path_str, log_type) in &standard_paths {
            let log_path = PathBuf::from(path_str);
            self.add_log_file(log_path, "standard_locations", log_type.clone(), discovered).await?;
        }

        Ok(())
    }

    /// Discover logs in a directory
    async fn discover_logs_in_directory(&self, dir_path: &PathBuf, method: &str, discovered: &mut DiscoveredLogs) -> Result<()> {
        let find_output = self.execute(&format!("find '{}' -name '*.log' -type f 2>/dev/null", dir_path.display()))?;
        
        for line in find_output.lines() {
            let log_path = PathBuf::from(line.trim());
            let log_type = self.determine_log_type_from_path(&log_path);
            self.add_log_file(log_path, method, log_type, discovered).await?;
        }

        Ok(())
    }

    /// Add a log file to the discovered set
    pub async fn add_log_file(&self, log_path: PathBuf, method: &str, log_type: LogType, discovered: &mut DiscoveredLogs) -> Result<()> {
        let path_str = log_path.display().to_string();
        
        // Don't add duplicates
        if discovered.log_files.contains_key(&path_str) {
            return Ok(());
        }

        let log_info = LogFileInfo {
            path: log_path.clone(),
            discovery_method: method.to_string(),
            log_type,
            accessible: false, // Will be validated later
            size_bytes: None,
            last_modified: None,
        };

        discovered.log_files.insert(path_str, log_info);
        Ok(())
    }

    /// Determine log type from file path
    pub fn determine_log_type_from_path(&self, path: &PathBuf) -> LogType {
        let path_str = path.to_string_lossy().to_lowercase();
        let filename = path.file_name()
            .map(|f| f.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        if filename.contains("server") || path_str.contains("server") {
            LogType::Server
        } else if filename.contains("controller") || path_str.contains("controller") {
            LogType::Controller
        } else if filename.contains("state") || filename.contains("change") {
            LogType::StateChange
        } else if filename.contains("request") {
            LogType::Request
        } else if filename.contains("gc") {
            LogType::Gc
        } else if filename.contains("kafka") {
            LogType::Custom(filename)
        } else {
            LogType::Unknown
        }
    }

    /// Validate discovered log files (check accessibility, get metadata)
    pub async fn validate_log_files(&self, discovered: &mut DiscoveredLogs) -> Result<()> {
        debug!("Validating discovered log files...");

        for (path_str, log_info) in discovered.log_files.iter_mut() {
            // Test file accessibility
            if let Ok(_) = self.execute(&format!("test -r '{}'", path_str)) {
                log_info.accessible = true;

                // Get file size
                if let Ok(size_output) = self.execute(&format!("stat -c %s '{}' 2>/dev/null", path_str)) {
                    if let Ok(size) = size_output.trim().parse::<u64>() {
                        log_info.size_bytes = Some(size);
                    }
                }

                // Get last modified time
                if let Ok(mtime_output) = self.execute(&format!("stat -c %y '{}' 2>/dev/null", path_str)) {
                    log_info.last_modified = Some(mtime_output.trim().to_string());
                }
            } else {
                debug!("Log file not accessible: {}", path_str);
            }
        }

        Ok(())
    }

    /// Collect content from discovered log files
    pub async fn collect_log_contents(&self, discovered: &DiscoveredLogs, max_lines: usize) -> Result<HashMap<String, String>> {
        let mut log_contents = HashMap::new();

        for (path_str, log_info) in &discovered.log_files {
            if !log_info.accessible {
                continue;
            }

            // Collect last N lines from each log file
            let content = self.execute(&format!("tail -n {} '{}' 2>/dev/null", max_lines, path_str))?;
            if !content.trim().is_empty() {
                log_contents.insert(path_str.clone(), content);
            }
        }

        Ok(log_contents)
    }
}

impl std::fmt::Display for LogType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogType::Server => write!(f, "server"),
            LogType::Controller => write!(f, "controller"),
            LogType::StateChange => write!(f, "state-change"),
            LogType::Request => write!(f, "request"),
            LogType::Gc => write!(f, "gc"),
            LogType::Custom(name) => write!(f, "custom({})", name),
            LogType::Unknown => write!(f, "unknown"),
        }
    }
}