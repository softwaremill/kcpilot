use anyhow::Result;
use crate::analyzers::{AnalyzerRegistry, config_validator::ConfigValidator};
use crate::analysis::AiExecutor;
use crate::cli::utils::load_snapshot_from_directory;
use crate::snapshot::format::Snapshot;
use crate::report::terminal::TerminalReporter;
use crate::report::markdown::MarkdownReporter;
use std::fs;
use std::path::PathBuf;
use tracing::{info, warn};

pub async fn handle_analyze_command(
    scanned_data: PathBuf,
    report: crate::cli::commands::ReportFormat,
    output: Option<PathBuf>,
    llmdbg: bool,
    llm_timeout: u64,
) -> Result<()> {
    info!("Starting analysis of scanned data: {}", scanned_data.display());

    // Load snapshot data
    let snapshot_data = if scanned_data.is_dir() {
        // Load from scan directory
        load_snapshot_from_directory(&scanned_data)?
    } else {
        // Load from JSON file
        info!("\n📄 Loading snapshot from JSON file: {}", scanned_data.display());
        info!("────────────────────────────────────────");
        let content = fs::read_to_string(&scanned_data)?;
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

    let findings = if let Ok(llm_service) = crate::llm::LlmService::from_env_with_options(llmdbg, llm_timeout) {
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
        crate::cli::commands::ReportFormat::Terminal => {
            let reporter = TerminalReporter::new();
            reporter.report(&snapshot_data, &findings)?;
        }
        crate::cli::commands::ReportFormat::Json => {
            let json = serde_json::to_string_pretty(&findings)?;
            println!("{}", json);
        }
        crate::cli::commands::ReportFormat::Markdown => {
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
    }

    Ok(())
}