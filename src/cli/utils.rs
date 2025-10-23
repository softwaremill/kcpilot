use anyhow::Result;
use crate::snapshot::format::{Snapshot, SnapshotMetadata, ClusterMode};
use std::fs;
use std::path::Path;
use tracing::{info, warn};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use walkdir::WalkDir;

pub fn init_logging(verbose: bool, log_format: &str) {
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
        .try_init()
        .expect("Failed to initialize tracing subscriber");
}

pub fn print_info() {
    println!("KCPilot v{}", env!("CARGO_PKG_VERSION"));
    println!("{}", env!("CARGO_PKG_DESCRIPTION"));
    println!();
    println!("Authors: {}", env!("CARGO_PKG_AUTHORS"));
    println!("License: {}", env!("CARGO_PKG_LICENSE"));
    println!();
    println!("For more information, visit: {}", env!("CARGO_PKG_REPOSITORY"));
}

/// Recursively load all files from a directory into a JSON structure
pub fn load_directory_recursive(dir: &Path, base_path: &Path) -> Result<serde_json::Value> {
    let mut result = serde_json::Map::new();

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name().to_string_lossy().into_owned();

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
pub fn load_snapshot_from_directory(path: &Path) -> Result<Snapshot> {
    info!("\n📂 Loading ALL files from directory: {}", path.display());
    info!("────────────────────────────────────────");

    let mut data_summary = DataSummary::default();

    // Create a snapshot with default metadata
    let mut snapshot = Snapshot::new(SnapshotMetadata::new(
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
                        "kraft" => ClusterMode::Kraft,
                        "zookeeper" => ClusterMode::Zookeeper,
                        _ => ClusterMode::Unknown,
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
                                    log_content.clone(),
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
                                    config_content.clone(),
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