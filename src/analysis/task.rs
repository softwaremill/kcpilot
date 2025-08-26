use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use tracing::{debug, info, warn};

/// A simple analysis task that can be defined in YAML
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisTask {
    /// Unique ID for the task
    pub id: String,
    
    /// Human-readable name
    pub name: String,
    
    /// Description of what this analyzes
    pub description: String,
    
    /// The prompt to send to the AI
    /// Can use placeholders like {logs}, {config}, {metrics}, {admin}, {topics}
    pub prompt: String,
    
    /// Which data to include (if not specified, includes everything available)
    #[serde(default)]
    pub include_data: Vec<String>,
    
    /// Keywords that indicate severity levels
    #[serde(default)]
    pub severity_keywords: HashMap<String, String>,
    
    /// Default severity if no keywords match
    #[serde(default = "default_severity")]
    pub default_severity: String,
    
    /// Category of the finding (performance, security, availability, etc.)
    #[serde(default = "default_category")]
    pub category: String,
    
    /// Whether this task is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    
    /// Optional examples to help the AI understand the expected output
    #[serde(default)]
    pub examples: Option<String>,
}

fn default_severity() -> String {
    "medium".to_string()
}

fn default_category() -> String {
    "cluster_hygiene".to_string()
}

fn default_enabled() -> bool {
    true
}

/// Loads tasks from YAML files
pub struct TaskLoader {
    tasks_dir: PathBuf,
}

impl TaskLoader {
    pub fn new(tasks_dir: impl AsRef<Path>) -> Self {
        Self {
            tasks_dir: tasks_dir.as_ref().to_path_buf(),
        }
    }
    
    pub fn default() -> Self {
        Self::new("analysis_tasks")
    }
    
    /// Load all tasks from the directory
    pub fn load_all(&self) -> Result<Vec<AnalysisTask>> {
        let mut tasks = Vec::new();
        
        // Add built-in tasks first
        tasks.extend(self.builtin_tasks());
        
        // Then load from directory if it exists
        if self.tasks_dir.exists() {
            info!("Loading tasks from {}", self.tasks_dir.display());
            
            for entry in fs::read_dir(&self.tasks_dir)? {
                let entry = entry?;
                let path = entry.path();
                
                if path.extension().and_then(|s| s.to_str()) == Some("yaml") ||
                   path.extension().and_then(|s| s.to_str()) == Some("yml") {
                    match self.load_task_file(&path) {
                        Ok(task) => {
                            if task.enabled {
                                info!("Loaded task: {} ({})", task.name, task.id);
                                tasks.push(task);
                            } else {
                                debug!("Skipping disabled task: {}", task.id);
                            }
                        }
                        Err(e) => {
                            warn!("Failed to load task from {}: {}", path.display(), e);
                        }
                    }
                }
            }
        } else {
            info!("Tasks directory {} not found, using built-in tasks only", 
                  self.tasks_dir.display());
        }
        
        Ok(tasks)
    }
    
    fn load_task_file(&self, path: &Path) -> Result<AnalysisTask> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        
        let task: AnalysisTask = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse {}", path.display()))?;
        
        Ok(task)
    }
    
    /// Built-in tasks (converted from existing analyzers)
    fn builtin_tasks(&self) -> Vec<AnalysisTask> {
        vec![
            AnalysisTask {
                id: "general_health".to_string(),
                name: "General Cluster Health Check".to_string(),
                description: "Comprehensive analysis of Kafka cluster health".to_string(),
                prompt: r#"You are an expert Kafka administrator. Analyze this Kafka cluster for any issues.

Cluster Data:
{admin}

Logs (if available):
{logs}

Configuration:
{config}

Please provide a comprehensive analysis including:
1. Critical issues that need immediate attention
2. Performance problems or bottlenecks  
3. Configuration issues or misconfigurations
4. Security concerns
5. Capacity or scaling issues
6. Any anomalies or unusual patterns

For each issue found, provide:
- Clear description of the problem
- Impact on the cluster
- Root cause if identifiable
- Specific remediation steps

Format your response as JSON:
{
  "findings": [
    {
      "title": "Brief title",
      "severity": "critical|high|medium|low|info",
      "description": "Detailed description",
      "impact": "Impact description",
      "root_cause": "Root cause if known",
      "remediation": "Steps to fix"
    }
  ]
}"#.to_string(),
                include_data: vec![],
                severity_keywords: HashMap::from([
                    ("critical".to_string(), "critical".to_string()),
                    ("high".to_string(), "high".to_string()),
                    ("medium".to_string(), "medium".to_string()),
                    ("low".to_string(), "low".to_string()),
                    ("offline".to_string(), "critical".to_string()),
                    ("under-replicated".to_string(), "high".to_string()),
                    ("degraded".to_string(), "high".to_string()),
                ]),
                default_severity: "medium".to_string(),
                category: "cluster_hygiene".to_string(),
                enabled: true,
                examples: None,
            },
            
            AnalysisTask {
                id: "log_analysis".to_string(),
                name: "Log Error Analysis".to_string(),
                description: "Analyze logs for errors and issues".to_string(),
                prompt: r#"Analyze these Kafka logs for errors, warnings, and issues:

{logs}

Focus on:
1. Critical errors and their patterns
2. Repeated warnings
3. Performance issues (GC pauses, timeouts)
4. Security issues
5. Unusual patterns

Group similar errors together and provide actionable insights.

Format as JSON with findings array."#.to_string(),
                include_data: vec!["logs".to_string()],
                severity_keywords: HashMap::from([
                    ("fatal".to_string(), "critical".to_string()),
                    ("error".to_string(), "high".to_string()),
                    ("warn".to_string(), "medium".to_string()),
                ]),
                default_severity: "medium".to_string(),
                category: "cluster_hygiene".to_string(),
                enabled: true,
                examples: None,
            },
            
            AnalysisTask {
                id: "partition_health".to_string(),
                name: "Partition Health Check".to_string(),
                description: "Check for offline and under-replicated partitions".to_string(),
                prompt: r#"Analyze the Kafka cluster partition health:

Admin/Topic Data:
{admin}

Check for:
1. Offline partitions (no leader)
2. Under-replicated partitions (ISR < replicas)
3. Leader imbalance across brokers
4. ISR shrinkage issues

For any issues, explain the impact and how to fix them.

Respond with JSON format including findings array."#.to_string(),
                include_data: vec!["admin".to_string()],
                severity_keywords: HashMap::from([
                    ("offline".to_string(), "critical".to_string()),
                    ("no leader".to_string(), "critical".to_string()),
                    ("under-replicated".to_string(), "high".to_string()),
                    ("imbalance".to_string(), "medium".to_string()),
                ]),
                default_severity: "high".to_string(),
                category: "availability".to_string(),
                enabled: true,
                examples: None,
            },
            
            AnalysisTask {
                id: "config_review".to_string(),
                name: "Configuration Review".to_string(),
                description: "Review Kafka configuration for issues and optimization opportunities".to_string(),
                prompt: r#"Review this Kafka cluster configuration:

{config}

Analyze for:
1. Security issues or missing security settings
2. Performance tuning opportunities
3. High availability concerns
4. Resource allocation issues
5. Common misconfigurations

Provide specific recommendations with expected benefits.

Format as JSON with findings array."#.to_string(),
                include_data: vec!["config".to_string()],
                severity_keywords: HashMap::from([
                    ("security".to_string(), "high".to_string()),
                    ("misconfiguration".to_string(), "medium".to_string()),
                    ("optimization".to_string(), "low".to_string()),
                ]),
                default_severity: "medium".to_string(),
                category: "configuration".to_string(),
                enabled: true,
                examples: None,
            }
        ]
    }
}
