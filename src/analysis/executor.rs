use super::task::{AnalysisTask, TaskLoader};
use crate::llm::LlmService;
use crate::snapshot::format::{
    Finding, Snapshot, Evidence, Category, Severity, 
    Remediation, RemediationStep, RiskLevel
};
use anyhow::Result;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fmt::Write;
use tracing::{debug, info, warn};

pub struct AiExecutor {
    llm_service: LlmService,
    task_loader: TaskLoader,
}

impl AiExecutor {
    pub fn new(llm_service: LlmService) -> Self {
        Self {
            llm_service,
            task_loader: TaskLoader::default_tasks_dir(),
        }
    }
    
    pub fn with_tasks_dir(llm_service: LlmService, tasks_dir: impl AsRef<std::path::Path>) -> Self {
        Self {
            llm_service,
            task_loader: TaskLoader::new(tasks_dir),
        }
    }
    
    /// Run all enabled tasks on a snapshot
    pub async fn analyze_all(&mut self, snapshot: &Snapshot) -> Result<Vec<Finding>> {
        info!("Starting AI-only analysis");
        
        // Load all tasks
        let tasks = self.task_loader.load_all()?;
        info!("Loaded {} analysis tasks", tasks.len());
        
        let mut all_findings = Vec::new();
        
        // Execute each task (with cluster type filtering)
        for task in tasks {
            // Check if task is compatible with current cluster type
            if !self.is_task_compatible(&task, snapshot) {
                debug!("Skipping task '{}' - not compatible with cluster type {:?}", task.name, snapshot.cluster.mode);
                continue;
            }
            
            info!("Running task: {}", task.name);
            
            match self.execute_task(&task, snapshot).await {
                Ok(mut findings) => {
                    info!("Task '{}' found {} issues", task.name, findings.len());
                    all_findings.append(&mut findings);
                }
                Err(e) => {
                    warn!("Task '{}' failed: {}", task.name, e);
                }
            }
        }
        
        info!("Analysis complete. Total findings: {}", all_findings.len());
        Ok(all_findings)
    }
    
    /// Execute a single task
    pub async fn execute_task(&self, task: &AnalysisTask, snapshot: &Snapshot) -> Result<Vec<Finding>> {
        debug!("Executing task: {} ({})", task.name, task.id);
        
        // Build the prompt with available data
        let prompt = self.build_prompt(task, snapshot)?;
        
        // Add examples if provided
        let full_prompt = if let Some(examples) = &task.examples {
            format!("{}\n\nExamples:\n{}", prompt, examples)
        } else {
            prompt
        };
        
        // Call the LLM
        debug!("Sending prompt to LLM (length: {} chars)", full_prompt.len());
        
        let response = self.llm_service.chat(vec![
            crate::llm::service::ChatMessage::system(
                "You are KafkaPilot, an expert Kafka administrator analyzing cluster health. \
                 Always respond with valid JSON containing a 'findings' array."
            ),
            crate::llm::service::ChatMessage::user(&full_prompt),
        ]).await?;
        
        debug!("Received LLM response (length: {} chars)", response.len());
        
        // Parse the response into findings
        self.parse_findings(&response, task)
    }
    
    /// Build the prompt with actual data
    fn build_prompt(&self, task: &AnalysisTask, snapshot: &Snapshot) -> Result<String> {
        let mut prompt = task.prompt.clone();
        let data_map = self.prepare_data(task, snapshot)?;
        
        // Replace placeholders with actual data
        for (key, value) in data_map {
            let placeholder = format!("{{{}}}", key);
            prompt = prompt.replace(&placeholder, &value);
        }
        
        Ok(prompt)
    }
    
    /// Prepare data based on task requirements
    fn prepare_data(&self, task: &AnalysisTask, snapshot: &Snapshot) -> Result<HashMap<String, String>> {
        let mut data = HashMap::new();
        
        // Determine what data to include
        let include_all = task.include_data.is_empty();
        let should_include = |name: &str| {
            if include_all {
                return true;
            }
            
            // Check for exact match first
            if task.include_data.contains(&name.to_string()) {
                return true;
            }
            
            // Check for collector:filename format
            for include_item in &task.include_data {
                if let Some((collector_name, _)) = include_item.split_once(':') {
                    if collector_name == name {
                        return true;
                    }
                }
            }
            
            false
        };
        
        // Add admin data (topics, brokers, etc.)
        if should_include("admin") {
            if let Some(admin_data) = &snapshot.collectors.admin {
                data.insert("admin".to_string(), 
                           serde_json::to_string_pretty(admin_data)?);
                
                // Also make topics available separately
                if let Some(topics) = admin_data.get("topics") {
                    data.insert("topics".to_string(), 
                               serde_json::to_string_pretty(topics)?);
                }
            } else {
                data.insert("admin".to_string(), 
                           "No admin data available".to_string());
            }
        }
        
        // Add logs
        if should_include("logs") {
            if let Some(logs_data) = &snapshot.collectors.logs {
                // Extract actual log content
                let logs_str = self.extract_logs(logs_data)?;
                data.insert("logs".to_string(), logs_str);
            } else {
                data.insert("logs".to_string(), 
                           "No log data available".to_string());
            }
        }
        
        // Add configuration
        if should_include("config") {
            if let Some(config_data) = &snapshot.collectors.config {
                data.insert("config".to_string(), 
                           serde_json::to_string_pretty(config_data)?);
            } else {
                data.insert("config".to_string(), 
                           "No configuration data available".to_string());
            }
        }
        
        // Add metrics
        if should_include("metrics") {
            if let Some(metrics_data) = &snapshot.collectors.metrics {
                data.insert("metrics".to_string(), 
                           serde_json::to_string_pretty(metrics_data)?);
            } else {
                data.insert("metrics".to_string(), 
                           "No metrics data available".to_string());
            }
        }
        
        // Add custom collectors
        for (name, custom_data) in &snapshot.collectors.custom {
            if should_include(name) {
                // Check if there's a specific file requested (format: "collector:filename")
                let mut specific_file_requested = false;
                let mut extracted_data = None;
                
                for include_item in &task.include_data {
                    if let Some((collector_name, file_name)) = include_item.split_once(':') {
                        if collector_name == name {
                            specific_file_requested = true;
                            
                            // Special handling for system:processes.txt - aggregate from all brokers
                            if name == "system" && file_name == "processes.txt" {
                                let mut all_processes = String::with_capacity(8192);
                                
                                // Look for processes.txt in brokers data
                                if let Some(brokers_data) = snapshot.collectors.custom.get("brokers") {
                                    if let Some(brokers_obj) = brokers_data.as_object() {
                                        for (broker_name, broker_value) in brokers_obj {
                                            if let Some(broker_obj) = broker_value.as_object() {
                                                if let Some(system_obj) = broker_obj.get("system") {
                                                    if let Some(system_map) = system_obj.as_object() {
                                                        if let Some(processes) = system_map.get("processes.txt") {
                                                            if let Some(content) = processes.as_str() {
                                                                writeln!(all_processes, "=== Broker {} ===", broker_name).unwrap();
                                                                all_processes.push_str(content);
                                                                writeln!(all_processes).unwrap();
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                
                                // Also check top-level system directory (bastion)
                                if let Some(obj) = custom_data.as_object() {
                                    if let Some(bastion_obj) = obj.get("bastion") {
                                        if let Some(bastion_map) = bastion_obj.as_object() {
                                            if let Some(processes) = bastion_map.get("processes.txt") {
                                                if let Some(content) = processes.as_str() {
                                                    writeln!(all_processes, "=== Bastion ===").unwrap();
                                                    all_processes.push_str(content);
                                                    writeln!(all_processes).unwrap();
                                                }
                                            }
                                        }
                                    }
                                }
                                
                                if !all_processes.is_empty() {
                                    extracted_data = Some(all_processes);
                                }
                            } else {
                                // Regular file extraction from collector data
                                if let Some(obj) = custom_data.as_object() {
                                    if let Some(file_data) = obj.get(file_name) {
                                        extracted_data = Some(serde_json::to_string_pretty(file_data)?);
                                    }
                                }
                            }
                            break;
                        }
                    }
                }
                
                if specific_file_requested {
                    if let Some(file_content) = extracted_data {
                        data.insert(name.clone(), file_content);
                    } else {
                        data.insert(name.clone(), format!("Requested file not found in {} data", name));
                    }
                } else {
                    data.insert(name.clone(), 
                               serde_json::to_string_pretty(custom_data)?);
                }
            }
        }
        
        Ok(data)
    }
    
    /// Extract log content from log data
    fn extract_logs(&self, logs_data: &Value) -> Result<String> {
        if let Some(logs_obj) = logs_data.as_object() {
            let mut all_logs = String::with_capacity(4096);
            
            // If it has a summary field, include that
            if let Some(summary) = logs_obj.get("summary") {
                writeln!(all_logs, "=== LOG SUMMARY ===").unwrap();
                all_logs.push_str(&serde_json::to_string_pretty(summary)?);
                writeln!(all_logs).unwrap();
            }
            
            // If it has raw logs, include those
            if let Some(raw) = logs_obj.get("raw_logs") {
                if let Some(raw_str) = raw.as_str() {
                    writeln!(all_logs, "=== RAW LOGS ===").unwrap();
                    all_logs.push_str(raw_str);
                }
            }
            
            // Otherwise just serialize the whole thing
            if all_logs.is_empty() {
                all_logs = serde_json::to_string_pretty(logs_data)?;
            }
            
            Ok(all_logs)
        } else {
            Ok(serde_json::to_string_pretty(logs_data)?)
        }
    }
    
    /// Check if task is compatible with current cluster type
    fn is_task_compatible(&self, task: &AnalysisTask, snapshot: &Snapshot) -> bool {
        // If no cluster type filter specified, task runs on all cluster types
        if task.cluster_type_filter.is_empty() {
            return true;
        }
        
        // Convert cluster mode to string for comparison
        let cluster_mode_str = match snapshot.cluster.mode {
            crate::snapshot::format::ClusterMode::Kraft => "kraft",
            crate::snapshot::format::ClusterMode::Zookeeper => "zookeeper", 
            crate::snapshot::format::ClusterMode::Unknown => "unknown",
        };
        
        // Check if current cluster mode is in the task's filter list
        task.cluster_type_filter.contains(&cluster_mode_str.to_string())
    }
    
    /// Parse LLM response into findings
    fn parse_findings(&self, response: &str, task: &AnalysisTask) -> Result<Vec<Finding>> {
        // Try to extract JSON from the response
        let json_str = if response.contains("```json") {
            let start = response.find("```json").unwrap() + 7;
            let end = response[start..].find("```")
                .unwrap_or(response.len() - start);
            &response[start..start + end]
        } else if response.trim().starts_with('{') {
            response.trim()
        } else {
            // Try to find JSON in the response
            if let Some(start) = response.find('{') {
                if let Some(end) = response.rfind('}') {
                    &response[start..=end]
                } else {
                    response
                }
            } else {
                // If no JSON found, create a single finding with the response
                return Ok(vec![self.create_text_finding(response, task)]);
            }
        };
        
        // Parse JSON
        let parsed: Value = match serde_json::from_str(json_str) {
            Ok(v) => v,
            Err(e) => {
                warn!("Failed to parse JSON response: {}", e);
                return Ok(vec![self.create_text_finding(response, task)]);
            }
        };
        
        let mut findings = Vec::new();
        
        // Extract findings array
        if let Some(findings_array) = parsed["findings"].as_array() {
            for (i, finding_json) in findings_array.iter().enumerate() {
                let finding = self.json_to_finding(finding_json, task, i)?;
                findings.push(finding);
            }
        } else if parsed.is_object() && parsed.get("title").is_some() {
            // Single finding as object
            findings.push(self.json_to_finding(&parsed, task, 0)?);
        } else {
            // Couldn't parse structured findings
            findings.push(self.create_text_finding(response, task));
        }
        
        Ok(findings)
    }
    
    /// Convert JSON to Finding
    fn json_to_finding(&self, json: &Value, task: &AnalysisTask, index: usize) -> Result<Finding> {
        // Determine severity first to create appropriate default title
        let severity = if let Some(sev_str) = json["severity"].as_str() {
            self.parse_severity(sev_str)
        } else {
            let temp_title = json["title"].as_str().unwrap_or(&task.name);
            let temp_description = json["description"].as_str().unwrap_or("");
            self.determine_severity(temp_title, temp_description, task)
        };
        
        // Create severity-appropriate default title
        let default_title = match severity {
            crate::snapshot::format::Severity::Critical | 
            crate::snapshot::format::Severity::High | 
            crate::snapshot::format::Severity::Medium => {
                format!("{} - Issue {}", task.name, index + 1)
            },
            crate::snapshot::format::Severity::Low => {
                format!("{} - Finding {}", task.name, index + 1)
            },
            crate::snapshot::format::Severity::Info => {
                format!("{} - Report {}", task.name, index + 1)
            }
        };
        
        let title = json["title"].as_str()
            .unwrap_or(&default_title)
            .to_string();
        
        let description = json["description"].as_str()
            .unwrap_or("No description provided")
            .to_string();
        
        let impact = json["impact"].as_str()
            .unwrap_or("Impact not specified")
            .to_string();
        
        let root_cause = json["root_cause"].as_str()
            .map(|s| s.to_string());
        
        let remediation_text = json["remediation"].as_str()
            .unwrap_or("No remediation provided");
        
        // We already calculated severity above for the title
        
        // Parse category
        let category = self.parse_category(&task.category);
        
        // Build remediation
        let remediation = Remediation {
            steps: vec![
                RemediationStep {
                    order: 1,
                    description: remediation_text.to_string(),
                    command: None,
                    verification: None,
                    can_automate: false,
                }
            ],
            script: None,
            risk_level: RiskLevel::Medium,
            requires_downtime: false,
            estimated_duration_minutes: None,
            rollback_plan: None,
        };
        
        Ok(Finding {
            id: format!("{}-{:03}", task.id, index + 1),
            severity,
            category,
            title,
            description,
            impact,
            evidence: Evidence {
                logs: vec![],
                metrics: vec![],
                configs: vec![],
                raw_data: Some(json.clone()),
            },
            root_cause,
            remediation,
            metadata: HashMap::new(),
        })
    }
    
    /// Create a finding from plain text response
    fn create_text_finding(&self, response: &str, task: &AnalysisTask) -> Finding {
        Finding {
            id: format!("{}-001", task.id),
            severity: self.parse_severity(&task.default_severity),
            category: self.parse_category(&task.category),
            title: format!("{} Analysis", task.name),
            description: response.to_string(),
            impact: "See description for details".to_string(),
            evidence: Evidence {
                logs: vec![],
                metrics: vec![],
                configs: vec![],
                raw_data: Some(json!({ "response": response })),
            },
            root_cause: None,
            remediation: Remediation {
                steps: vec![],
                script: None,
                risk_level: RiskLevel::Low,
                requires_downtime: false,
                estimated_duration_minutes: None,
                rollback_plan: None,
            },
            metadata: HashMap::new(),
        }
    }
    
    /// Parse severity string
    fn parse_severity(&self, severity: &str) -> Severity {
        match severity.to_lowercase().as_str() {
            "critical" => Severity::Critical,
            "high" => Severity::High,
            "medium" => Severity::Medium,
            "low" => Severity::Low,
            "info" => Severity::Info,
            _ => Severity::Medium,
        }
    }
    
    /// Determine severity based on keywords
    fn determine_severity(&self, title: &str, description: &str, task: &AnalysisTask) -> Severity {
        let combined = format!("{} {}", title.to_lowercase(), description.to_lowercase());
        
        // Check severity keywords
        for (keyword, severity) in &task.severity_keywords {
            if combined.contains(&keyword.to_lowercase()) {
                return self.parse_severity(severity);
            }
        }
        
        self.parse_severity(&task.default_severity)
    }
    
    /// Parse category string
    fn parse_category(&self, category: &str) -> Category {
        match category.to_lowercase().as_str() {
            "performance" => Category::Performance,
            "security" => Category::Security,
            "availability" => Category::Availability,
            "configuration" => Category::Configuration,
            "capacity" => Category::Capacity,
            "client" => Category::Client,
            _ => Category::ClusterHygiene,
        }
    }
}

// Implement Analyzer trait for compatibility
use crate::analyzers::{Analyzer, AnalyzerResult};
use async_trait::async_trait;

#[async_trait]
impl Analyzer for AiExecutor {
    async fn analyze(&self, snapshot: &Snapshot) -> AnalyzerResult<Vec<Finding>> {
        // Create a new executor with the same service
        // Note: This creates a new TaskLoader but reuses the LlmService
        // We can't clone LlmService due to Mutex, but we can create a new AiExecutor
        // that shares the same LlmService reference through from_env
        match crate::llm::LlmService::from_env() {
            Ok(service) => {
                let mut executor = AiExecutor::new(service);
                executor.analyze_all(snapshot).await
                    .map_err(|e| crate::analyzers::AnalyzerError::AnalysisFailed(e.to_string()))
            }
            Err(e) => {
                Err(crate::analyzers::AnalyzerError::AnalysisFailed(
                    format!("Failed to initialize LLM service: {}", e)
                ))
            }
        }
    }
    
    fn name(&self) -> &'static str {
        "AI Task Executor"
    }
    
    fn description(&self) -> &'static str {
        "Executes AI-powered analysis tasks defined in YAML files"
    }
}
