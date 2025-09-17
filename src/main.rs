use anyhow::Result;
use clap::Parser;
use kafkapilot::cli::commands::{Cli, Commands};
use kafkapilot::scan::Scanner;
use kafkapilot::analyzers::{AnalyzerRegistry, config_validator::ConfigValidator};
use kafkapilot::analysis::AiExecutor;
use kafkapilot::snapshot::format::Snapshot;
use kafkapilot::report::terminal::TerminalReporter;
use kafkapilot::report::markdown::MarkdownReporter;
use kafkapilot::report::html::HtmlReporter;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use walkdir::WalkDir;

#[tokio::main]
async fn main() -> Result<()> {
    // Parse CLI arguments
    let cli = Cli::parse();
    
    // Initialize logging
    init_logging(cli.verbose, &cli.log_format);
    
    // Execute command
    match cli.command {
        Commands::Scan {
            bastion,
            output,
            broker,
        } => {
            info!("Starting Kafka cluster scan");
            
            // bastion is already Option<String>, pass it directly
            // None means we're running locally on the bastion
            let mut scanner = Scanner::new(bastion)?;
            
            // Set custom output directory if provided
            if let Some(output_path) = output {
                scanner = scanner.with_output_dir(output_path);
            }
            
            // Handle broker discovery based on input parameters
            match broker {
                Some(broker_address) => {
                    // Single broker provided - discover cluster from it
                    info!("Using broker discovery from: {}", broker_address);
                    scanner = scanner.discover_brokers_from_single(&broker_address).await?;
                }
                None => {
                    // No broker provided - try to discover from kafkactl
                    info!("No broker provided, attempting to discover brokers from kafkactl");
                    scanner = scanner.discover_brokers_from_kafkactl().await?;
                }
            }
            
            // Run the scan
            let result = scanner.scan().await?;
            
            // Log results
            info!(
                "Scan completed. Collected data from {} brokers out of {}",
                result.metadata.accessible_brokers,
                result.metadata.broker_count
            );
            
            Ok(())
        }
        
        Commands::Analyze { snapshot, report, output, llmdbg, llm_timeout } => {
            info!("Starting analysis of snapshot: {}", snapshot.display());
            
            // Load snapshot data
            let snapshot_data = if snapshot.is_dir() {
                // Load from scan directory
                load_snapshot_from_directory(&snapshot)?
            } else {
                // Load from JSON file
                info!("\n📄 Loading snapshot from JSON file: {}", snapshot.display());
                info!("────────────────────────────────────────");
                let content = fs::read_to_string(&snapshot)?;
                let loaded_snapshot: Snapshot = serde_json::from_str(&content)?;
                
                // Log what's available in the JSON snapshot
                info!("✓ Snapshot loaded successfully");
                info!("  Version: {}", loaded_snapshot.version);
                info!("  Timestamp: {}", loaded_snapshot.timestamp);
                
                // Check what data is available
                let mut has_data = false;
                if loaded_snapshot.collectors.logs.is_some() {
                    info!("  ✓ Log data: Available");
                    has_data = true;
                }
                if loaded_snapshot.collectors.config.is_some() {
                    info!("  ✓ Config data: Available");
                    has_data = true;
                }
                if loaded_snapshot.collectors.metrics.is_some() {
                    info!("  ✓ Metrics data: Available");
                    has_data = true;
                }
                if loaded_snapshot.collectors.admin.is_some() {
                    info!("  ✓ Admin data: Available");
                    has_data = true;
                }
                
                if !has_data {
                    warn!("⚠ WARNING: No collector data found in the snapshot!");
                    warn!("  The analysis may not produce meaningful results.");
                }
                
                info!("────────────────────────────────────────\n");
                loaded_snapshot
            };
            
            // Print pre-analysis summary
            info!("\n🔍 Pre-Analysis Summary");
            info!("────────────────────────────────────────");
            
            // Count available data types
            let mut available_data = Vec::new();
            if snapshot_data.collectors.logs.is_some() {
                available_data.push("Logs");
            }
            if snapshot_data.collectors.config.is_some() {
                available_data.push("Configurations");
            }
            if snapshot_data.collectors.metrics.is_some() {
                available_data.push("Metrics");
            }
            if snapshot_data.collectors.admin.is_some() {
                available_data.push("Admin Metadata");
            }
            
            if !available_data.is_empty() {
                info!("Data types to analyze: {}", available_data.join(", "));
            } else {
                warn!("⚠ No data available for analysis!");
                warn!("  Please ensure you have a valid snapshot with data.");
                return Err(anyhow::anyhow!("No data available for analysis"));
            }
            
            // Use AI-only analysis
            info!("🤖 Using AI-powered analysis...");
            
            let findings = if let Ok(llm_service) = kafkapilot::llm::LlmService::from_env_with_options(llmdbg, llm_timeout) {
                info!("✓ AI executor initialized");
                if llm_timeout != 300 {
                    info!("  Using custom timeout: {} seconds", llm_timeout);
                }
                
                let mut executor = AiExecutor::new(llm_service);
                info!("  Loading analysis tasks from 'analysis_tasks' directory...");
                
                executor.analyze_all(&snapshot_data).await?
            } else {
                warn!("AI analysis not available - LLM API key not configured");
                warn!("Please set OPENAI_API_KEY or LLM_API_KEY environment variable");
                
                // Fall back to basic static analysis if no LLM available
                info!("Falling back to static configuration validator...");
                let mut registry = AnalyzerRegistry::new();
                registry.register(Box::new(ConfigValidator::new()));
                registry.analyze_all(&snapshot_data).await?
            };
            
            info!("Analysis complete. Found {} findings", findings.len());
            
            // Generate report based on format
            match report {
                kafkapilot::cli::commands::ReportFormat::Terminal => {
                    let reporter = TerminalReporter::new();
                    reporter.report(&snapshot_data, &findings)?;
                }
                kafkapilot::cli::commands::ReportFormat::Json => {
                    let json = serde_json::to_string_pretty(&findings)?;
                    println!("{}", json);
                }
                kafkapilot::cli::commands::ReportFormat::Markdown => {
                    let output_path = output.unwrap_or_else(|| {
                        // Generate default filename with timestamp
                        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
                        PathBuf::from(format!("kafka_report_{}.md", timestamp))
                    });
                    
                    info!("Generating markdown report: {}", output_path.display());
                    let reporter = MarkdownReporter::new();
                    reporter.save_report(&snapshot_data, &findings, &output_path)?;
                    info!("✅ Report saved to: {}", output_path.display());
                }
                kafkapilot::cli::commands::ReportFormat::Html => {
                    let output_path = output.unwrap_or_else(|| {
                        // Generate default directory name with timestamp
                        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
                        PathBuf::from(format!("kafka_report_{}", timestamp))
                    });
                    
                    info!("Generating HTML report: {}", output_path.display());
                    let reporter = HtmlReporter::new();
                    reporter.save_report(&snapshot_data, &findings, &output_path)?;
                    info!("✅ HTML report saved to: {}/index.html", output_path.display());
                    info!("   Assets saved to: {}/assets/", output_path.display());
                }
                _ => {
                    println!("Report format {:?} not yet implemented", report);
                }
            }
            
            Ok(())
        }
        
        Commands::Watch { .. } => {
            println!("Watch command not yet implemented");
            Ok(())
        }
        
        Commands::Fix { .. } => {
            println!("Fix command not yet implemented");
            Ok(())
        }
        
        Commands::Config { subcommand } => {
            println!("Config command not yet implemented");
            println!("  Subcommand: {:?}", subcommand);
            Ok(())
        }
        
        Commands::Info => {
            print_info();
            Ok(())
        }
        
        Commands::Task { action } => {
            handle_task_command(action).await
        }
        
        Commands::TestSsh { bastion } => {
            handle_ssh_test(bastion).await
        }
    }
}

/// Handle SSH connectivity testing
async fn handle_ssh_test(bastion: Option<String>) -> Result<()> {
    println!("🔍 KafkaPilot SSH Connectivity Test");
    println!("═══════════════════════════════════════");
    
    let scanner = kafkapilot::scan::Scanner::new(bastion.clone())?;
    
    match &bastion {
        Some(alias) => {
            println!("Mode: Remote via bastion '{}'", alias);
            println!("\n🔍 Running SSH diagnostics...");
            scanner.run_ssh_diagnostics(alias).await;
        }
        None => {
            println!("Mode: Local/direct connections");
            println!("\n🔍 Testing direct broker connections...");
        }
    }
    
    println!("\n🔗 Testing individual broker connectivity:");
    println!("{}", "─".repeat(50));
    
    for broker in &scanner.config.brokers {
        print!("• {} ({}): ", broker.hostname, broker.datacenter);
        let accessible = scanner.test_broker_access(broker).await;
        if accessible {
            println!("✅ Connected");
        } else {
            println!("❌ Failed");
        }
    }
    
    println!("\n💡 Use 'RUST_LOG=debug' for detailed SSH error messages");
    
    Ok(())
}

/// Handle task management commands
async fn handle_task_command(action: kafkapilot::cli::commands::TaskCommand) -> Result<()> {
    use kafkapilot::cli::commands::TaskCommand;
    use kafkapilot::analysis::{TaskLoader, AiExecutor};
    
    match action {
        TaskCommand::List { detailed } => {
            let loader = TaskLoader::default();
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
        
        TaskCommand::Test { task_id, snapshot, debug } => {
            // Load the specific task
            let loader = TaskLoader::default();
            let tasks = loader.load_all()?;
            let task = tasks.into_iter()
                .find(|t| t.id == task_id)
                .ok_or_else(|| anyhow::anyhow!("Task '{}' not found", task_id))?;
            
            println!("🧪 Testing task: {}", task.name);
            
            // Load snapshot
            let snapshot_data = if snapshot.is_dir() {
                load_snapshot_from_directory(&snapshot)?
            } else {
                let content = fs::read_to_string(&snapshot)?;
                serde_json::from_str(&content)?
            };
            
            // Run the task
            if let Ok(llm_service) = kafkapilot::llm::LlmService::from_env_with_debug(debug) {
                let executor = AiExecutor::new(llm_service);
                
                match executor.execute_task(&task, &snapshot_data).await {
                    Ok(findings) => {
                        println!("\n✅ Task completed successfully!");
                        
                        // Count different types of findings for better messaging
                        let issues_count = findings.iter().filter(|f| matches!(f.severity, 
                            kafkapilot::snapshot::format::Severity::Critical | 
                            kafkapilot::snapshot::format::Severity::High | 
                            kafkapilot::snapshot::format::Severity::Medium)).count();
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
                                    kafkapilot::snapshot::format::Severity::Critical => "🔴",
                                    kafkapilot::snapshot::format::Severity::High => "🟠",
                                    kafkapilot::snapshot::format::Severity::Medium => "🟡",
                                    kafkapilot::snapshot::format::Severity::Low => "🟢",
                                    kafkapilot::snapshot::format::Severity::Info => "ℹ️",
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
            let loader = TaskLoader::default();
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

fn init_logging(verbose: bool, log_format: &str) {
    let env_filter = if verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    };
    
    let fmt_layer = if log_format == "json" {
        fmt::layer()
            .json()
            .with_current_span(false)
            .with_span_list(false)
            .boxed()
    } else {
        fmt::layer()
            .with_target(false)
            .with_thread_ids(false)
            .with_thread_names(false)
            .boxed()
    };
    
    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .init();
}

fn print_info() {
    println!("KafkaPilot v{}", env!("CARGO_PKG_VERSION"));
    println!("{}", env!("CARGO_PKG_DESCRIPTION"));
    println!();
    println!("Authors: {}", env!("CARGO_PKG_AUTHORS"));
    println!("License: {}", env!("CARGO_PKG_LICENSE"));
    println!();
    println!("For more information, visit: https://github.com/yourusername/kafkapilot");
}

/// Recursively load all files from a directory into a JSON structure
fn load_directory_recursive(dir: &Path, base_path: &Path) -> Result<serde_json::Value> {
    let mut result = serde_json::Map::new();
    
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name().to_string_lossy().to_string();
        
        if path.is_dir() {
            // Recursively load subdirectory
            let subdir_data = load_directory_recursive(&path, base_path)?;
            if !subdir_data.as_object().unwrap().is_empty() {
                result.insert(file_name, subdir_data);
            }
        } else if path.is_file() {
            // Load file content
            if let Ok(content) = fs::read_to_string(&path) {
                // Try to parse as JSON first
                if file_name.ends_with(".json") {
                    if let Ok(json_value) = serde_json::from_str(&content) {
                        result.insert(file_name, json_value);
                    } else {
                        // If JSON parsing fails, store as string
                        result.insert(file_name, serde_json::Value::String(content));
                    }
                } else {
                    // Store text files as strings
                    result.insert(file_name, serde_json::Value::String(content));
                }
                
                // Log relative path
                let relative_path = path.strip_prefix(base_path).unwrap_or(&path);
                info!("    ✓ Loaded: {}", relative_path.display());
            } else {
                // Binary file or unreadable - skip but log
                let relative_path = path.strip_prefix(base_path).unwrap_or(&path);
                info!("    ⚠ Skipped (binary/unreadable): {}", relative_path.display());
            }
        }
    }
    
    Ok(serde_json::Value::Object(result))
}

/// Load snapshot from a scan directory
fn load_snapshot_from_directory(path: &PathBuf) -> Result<Snapshot> {
    info!("\n📂 Loading ALL files from directory: {}", path.display());
    info!("────────────────────────────────────────");
    
    let mut data_summary = DataSummary::default();
    
    // Create a snapshot with default metadata
    let mut snapshot = Snapshot::new(kafkapilot::snapshot::format::SnapshotMetadata::new(
        env!("CARGO_PKG_VERSION").to_string()
    ));
    
    // Try to load scan_metadata.json if it exists
    let metadata_path = path.join("scan_metadata.json");
    if metadata_path.exists() {
        if let Ok(content) = fs::read_to_string(&metadata_path) {
            if let Ok(metadata) = serde_json::from_str::<serde_json::Value>(&content) {
                info!("✓ Found scan metadata");
                
                // Extract cluster mode from scan metadata
                if let Some(cluster_mode_str) = metadata.get("cluster_mode").and_then(|v| v.as_str()) {
                    snapshot.cluster.mode = match cluster_mode_str {
                        "kraft" => kafkapilot::snapshot::format::ClusterMode::Kraft,
                        "zookeeper" => kafkapilot::snapshot::format::ClusterMode::Zookeeper,
                        _ => kafkapilot::snapshot::format::ClusterMode::Unknown,
                    };
                    info!("  • Cluster mode: {}", cluster_mode_str);
                }
                
                // Extract other useful metadata
                if let Some(broker_count) = metadata.get("broker_count").and_then(|v| v.as_u64()) {
                    info!("  • Broker count: {}", broker_count);
                }
            }
        }
    }
    
    // Count total files first
    let total_files: usize = WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .count();
    
    info!("📊 Found {} total files to process", total_files);
    info!("\n📁 Loading file structure:");
    
    // Load broker data with full structure
    let brokers_dir = path.join("brokers");
    if brokers_dir.exists() {
        info!("\n  📂 Loading brokers/...");
        let brokers_data = load_directory_recursive(&brokers_dir, path)?;
        
        // For backward compatibility, also extract logs and configs
        let mut logs_data = serde_json::Map::new();
        let mut config_data = serde_json::Map::new();
        
        if let Some(brokers_obj) = brokers_data.as_object() {
            for (broker_name, broker_value) in brokers_obj {
                if let Some(broker_obj) = broker_value.as_object() {
                    data_summary.broker_count += 1;
                    
                    // Extract logs for backward compatibility
                    if let Some(logs) = broker_obj.get("logs") {
                        if let Some(logs_obj) = logs.as_object() {
                            for (log_name, log_content) in logs_obj {
                                logs_data.insert(
                                    format!("{}/{}", broker_name, log_name),
                                    log_content.clone()
                                );
                                data_summary.log_files.push(format!("{}/{}", broker_name, log_name));
                            }
                        }
                    }
                    
                    // Extract configs for backward compatibility
                    if let Some(configs) = broker_obj.get("configs") {
                        if let Some(configs_obj) = configs.as_object() {
                            for (config_name, config_content) in configs_obj {
                                config_data.insert(
                                    format!("{}/{}", broker_name, config_name),
                                    config_content.clone()
                                );
                                data_summary.config_files.push(format!("{}/{}", broker_name, config_name));
                            }
                        }
                    }
                    
                    // Check for other data types
                    if broker_obj.contains_key("metrics") {
                        data_summary.has_metrics = true;
                    }
                    if broker_obj.contains_key("system") {
                        data_summary.has_system_info = true;
                    }
                    if broker_obj.contains_key("data") {
                        data_summary.has_data_info = true;
                    }
                }
            }
        }
        
        // Store backward-compatible data
        if !logs_data.is_empty() {
            snapshot.collectors.logs = Some(serde_json::Value::Object(logs_data));
        }
        if !config_data.is_empty() {
            snapshot.collectors.config = Some(serde_json::Value::Object(config_data));
        }
        
        // Store full brokers data in custom field
        snapshot.collectors.custom.insert("brokers".to_string(), brokers_data);
    }
    
    // Load cluster data with full structure
    let cluster_dir = path.join("cluster");
    if cluster_dir.exists() {
        info!("\n  📂 Loading cluster/...");
        let cluster_data = load_directory_recursive(&cluster_dir, path)?;
        
        // Store in custom field
        snapshot.collectors.custom.insert("cluster".to_string(), cluster_data.clone());
        
        // Also store as admin data since cluster data contains admin information (brokers, topics, etc.)
        snapshot.collectors.admin = Some(cluster_data.clone());
        
        // Check for kafkactl data
        if let Some(cluster_obj) = cluster_data.as_object() {
            if cluster_obj.contains_key("kafkactl") {
                data_summary.has_kafkactl = true;
            }
        }
    }
    
    // Load metrics data
    let metrics_dir = path.join("metrics");
    if metrics_dir.exists() {
        info!("\n  📂 Loading metrics/...");
        let metrics_data = load_directory_recursive(&metrics_dir, path)?;
        
        // Store both in standard field and custom for full data
        snapshot.collectors.metrics = Some(metrics_data.clone());
        snapshot.collectors.custom.insert("metrics".to_string(), metrics_data);
        data_summary.has_metrics = true;
    }
    
    // Load system data
    let system_dir = path.join("system");
    if system_dir.exists() {
        info!("\n  📂 Loading system/...");
        let system_data = load_directory_recursive(&system_dir, path)?;
        
        snapshot.collectors.custom.insert("system".to_string(), system_data);
        data_summary.has_system_info = true;
    }
    
    // Load any other top-level files
    info!("\n  📂 Loading other files...");
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();
        let file_name = entry.file_name().to_string_lossy().to_string();
        
        // Skip directories we've already processed
        if entry_path.is_file() && file_name != "scan_metadata.json" {
            if let Ok(content) = fs::read_to_string(&entry_path) {
                if file_name.ends_with(".json") {
                    if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&content) {
                        snapshot.collectors.custom.insert(file_name.clone(), json_value);
                    } else {
                        snapshot.collectors.custom.insert(file_name.clone(), serde_json::Value::String(content));
                    }
                } else {
                    snapshot.collectors.custom.insert(file_name.clone(), serde_json::Value::String(content));
                }
                info!("    ✓ Loaded: {}", file_name);
                data_summary.other_files.push(file_name);
            }
        }
    }
    
    // Print comprehensive summary
    info!("\n📊 Data Loading Summary");
    info!("────────────────────────────────────────");
    
    info!("✓ Total files loaded: {}", total_files);
    
    if data_summary.broker_count > 0 {
        info!("✓ Brokers processed: {}", data_summary.broker_count);
        if !data_summary.config_files.is_empty() {
            info!("  • Config files: {}", data_summary.config_files.len());
        }
        if !data_summary.log_files.is_empty() {
            info!("  • Log files: {}", data_summary.log_files.len());
        }
    }
    
    // Report on all data types found
    let mut data_types_found = Vec::new();
    
    if !data_summary.config_files.is_empty() {
        data_types_found.push("Configurations");
    }
    if !data_summary.log_files.is_empty() {
        data_types_found.push("Logs");
    }
    if data_summary.has_metrics {
        data_types_found.push("Metrics");
    }
    if data_summary.has_system_info {
        data_types_found.push("System Info");
    }
    if data_summary.has_data_info {
        data_types_found.push("Data/Partition Info");
    }
    if data_summary.has_kafkactl {
        data_types_found.push("Kafka Admin Data");
    }
    if !data_summary.other_files.is_empty() {
        data_types_found.push("Additional Files");
        info!("  • Other files: {}", data_summary.other_files.len());
    }
    
    if !data_types_found.is_empty() {
        info!("\n✓ Data types available for analysis:");
        for data_type in &data_types_found {
            info!("  • {}", data_type);
        }
    } else {
        warn!("\n⚠ WARNING: No analyzable data found!");
        warn!("  Please ensure the directory contains valid Kafka scan data.");
    }
    
    // Store loaded files count in metadata
    snapshot.metadata.tags.insert("total_files_loaded".to_string(), total_files.to_string());
    snapshot.metadata.tags.insert("data_types".to_string(), data_types_found.join(", "));
    
    info!("────────────────────────────────────────\n");
    
    Ok(snapshot)
}

#[derive(Default)]
struct DataSummary {
    broker_count: usize,
    config_files: Vec<String>,
    log_files: Vec<String>,
    has_metrics: bool,
    has_system_info: bool,
    has_data_info: bool,
    has_kafkactl: bool,
    other_files: Vec<String>,
}