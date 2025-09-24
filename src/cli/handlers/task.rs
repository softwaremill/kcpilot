use anyhow::Result;
use crate::analysis::{TaskLoader, AiExecutor};
use crate::cli::utils::load_snapshot_from_directory;
use std::fs;

pub async fn handle_task_command(action: crate::cli::commands::TaskCommand) -> Result<()> {
    use crate::cli::commands::TaskCommand;

    match action {
        TaskCommand::List { detailed } => {
            let loader = TaskLoader::default_tasks_dir();
            let tasks = loader.load_all()?;

            println!("\n📋 Available Analysis Tasks\n");
            println!("{}", "─".repeat(60));

            for task in tasks {
                if detailed {
                    println!("\n🔍 {}", task.name);
                    println!("   ID: {}", task.id);
                    println!("   Description: {}", task.description);
                    println!("   Category: {}", task.category);
                    println!("   Default Severity: {}", task.default_severity);
                    println!("   Enabled: {}", task.enabled);
                    if !task.include_data.is_empty() {
                        println!("   Required Data: {}", task.include_data.join(", "));
                    }
                } else {
                    println!("• {} ({})", task.name, task.id);
                }
            }

            if !detailed {
                println!("\n💡 Use --detailed for more information");
            }
        }

        TaskCommand::Test { task_id, scanned_data, debug } => {
            // Load the specific task
            let loader = TaskLoader::default_tasks_dir();
            let tasks = loader.load_all()?;
            let task = tasks.into_iter()
                .find(|t| t.id == task_id)
                .ok_or_else(|| anyhow::anyhow!("Task '{}' not found in analysis_tasks directory", task_id))?;

            println!("🧪 Testing task: {}", task.name);

            // Load scanned data
            let snapshot_data = if scanned_data.is_dir() {
                load_snapshot_from_directory(&scanned_data)?
            } else {
                let content = fs::read_to_string(&scanned_data)?;
                serde_json::from_str(&content)?
            };

            // Run the task
            if let Ok(llm_service) = crate::llm::LlmService::from_env_with_debug(debug) {
                let executor = AiExecutor::new(llm_service);

                match executor.execute_task(&task, &snapshot_data).await {
                    Ok(findings) => {
                        println!("\n✅ Task completed successfully!");

                        // Count different types of findings for better messaging
                        let issues_count = findings.iter().filter(|f| matches!(f.severity, 
                            crate::snapshot::format::Severity::Critical | 
                            crate::snapshot::format::Severity::High | 
                            crate::snapshot::format::Severity::Medium)).count();
                        let findings_count = findings.len() - issues_count;

                        match (issues_count, findings_count) {
                            (0, 0) => println!("No findings reported.\n"),
                            (0, n) => println!("Found {} finding{}:\n", n, if n == 1 { "" } else { "s" }),
                            (i, 0) => println!("Found {} issue{}:\n", i, if i == 1 { "" } else { "s" }),
                            (i, f) => println!("Found {} issue{} and {} finding{}:\n",
                                               i, if i == 1 { "" } else { "s" },
                                               f, if f == 1 { "" } else { "s" })
                        }

                        for finding in findings {
                            println!("  {} [{}] {}",
                                     match finding.severity {
                                         crate::snapshot::format::Severity::Critical => "🔴",
                                         crate::snapshot::format::Severity::High => "🟠",
                                         crate::snapshot::format::Severity::Medium => "🟡",
                                         crate::snapshot::format::Severity::Low => "🟢",
                                         crate::snapshot::format::Severity::Info => "ℹ️",
                                     },
                                     finding.id,
                                     finding.title
                            );
                            println!("     {}", finding.description);
                            println!();
                        }
                    }
                    Err(e) => {
                        println!("\n❌ Task failed: {}", e);
                    }
                }
            } else {
                println!("❌ LLM service not configured. Please set OPENAI_API_KEY.");
            }
        }

        TaskCommand::New { id, name, output_dir } => {
            // Create directory if it doesn't exist
            fs::create_dir_all(&output_dir)?;

            let task_name = name.unwrap_or_else(|| format!("New Task {}", id));

            // Create a template task
            let template = format!(r#"# Task definition for {}
id: {}
name: {}
description: Analyze Kafka cluster for specific issues
category: cluster_hygiene

# The prompt sent to the AI
# Available placeholders: {{logs}}, {{config}}, {{metrics}}, {{admin}}, {{topics}}
prompt: |
  Analyze this Kafka cluster data for issues:
  
  Cluster Data:
  {{admin}}
  
  Configuration:
  {{config}}
  
  Please identify any problems and provide recommendations.
  
  Format your response as JSON with a "findings" array.

# Which data to include (leave empty for all)
include_data: []

# Keywords that indicate severity levels
severity_keywords:
  critical: "critical"
  high: "high" 
  medium: "medium"
  low: "low"

default_severity: medium
enabled: true
"#, task_name, id, task_name);

            let file_path = output_dir.join(format!("{}.yaml", id));
            fs::write(&file_path, template)?;

            println!("✅ Created task template: {}", file_path.display());
            println!("\n📝 Edit the file to customize:");
            println!("  - Update the prompt with your specific analysis requirements");
            println!("  - Adjust severity keywords for your use case");
            println!("  - Specify required data in include_data if needed");
        }

        TaskCommand::Show { task_id } => {
            let loader = TaskLoader::default_tasks_dir();
            let tasks = loader.load_all()?;
            let task = tasks.into_iter()
                .find(|t| t.id == task_id)
                .ok_or_else(|| anyhow::anyhow!("Task '{}' not found", task_id))?;

            println!("\n📄 Task Details: {}\n", task.name);
            println!("{}", "─".repeat(60));
            println!("ID: {}", task.id);
            println!("Description: {}", task.description);
            println!("Category: {}", task.category);
            println!("Default Severity: {}", task.default_severity);
            println!("Enabled: {}", task.enabled);

            if !task.include_data.is_empty() {
                println!("\nRequired Data:");
                for data in &task.include_data {
                    println!("  • {}", data);
                }
            }

            if !task.severity_keywords.is_empty() {
                println!("\nSeverity Keywords:");
                for (keyword, severity) in &task.severity_keywords {
                    println!("  • {} → {}", keyword, severity);
                }
            }

            println!("\nPrompt:");
            println!("{}", "─".repeat(40));
            println!("{}", task.prompt);
        }
    }

    Ok(())
}