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
    
    /// Optional cluster type filter (kraft, zookeeper, or both if not specified)
    #[serde(default)]
    pub cluster_type_filter: Vec<String>,
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
    
    pub fn default_tasks_dir() -> Self {
        Self::new("analysis_tasks")
    }
    
    /// Load all tasks from the directory
    pub fn load_all(&self) -> Result<Vec<AnalysisTask>> {
        let mut tasks = Vec::new();
        
        // Load from directory if it exists
        if self.tasks_dir.exists() {
            info!("Loading tasks from {}", self.tasks_dir.display());
            self.load_tasks_recursive(&self.tasks_dir, &mut tasks)?;
            
            if tasks.is_empty() {
                warn!("No valid task files found in {}", self.tasks_dir.display());
                return Err(anyhow::anyhow!("No analysis tasks found. Please ensure YAML task files exist in the '{}' directory.", self.tasks_dir.display()));
            }
        } else {
            return Err(anyhow::anyhow!(
                "Tasks directory '{}' not found. Please create it and add YAML task definitions.", 
                self.tasks_dir.display()
            ));
        }
        
        Ok(tasks)
    }
    
    /// Recursively load tasks from a directory and its subdirectories
    fn load_tasks_recursive(&self, dir: &Path, tasks: &mut Vec<AnalysisTask>) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() {
                // Skip the legacy directory to avoid loading old tasks
                if path.file_name().and_then(|s| s.to_str()) == Some("legacy") {
                    debug!("Skipping legacy directory: {}", path.display());
                    continue;
                }
                
                // Recursively scan subdirectories
                debug!("Scanning subdirectory: {}", path.display());
                self.load_tasks_recursive(&path, tasks)?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("yaml") ||
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
        
        Ok(())
    }
    
    fn load_task_file(&self, path: &Path) -> Result<AnalysisTask> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        
        let task: AnalysisTask = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse {}", path.display()))?;
        
        Ok(task)
    }
}
