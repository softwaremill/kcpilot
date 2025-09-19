use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{debug, info, warn};

use super::types::KafkaProcessInfo;

/// Parser for Kafka process information from ps aux output
pub struct ProcessParser;

impl ProcessParser {
    /// Find Kafka Java process using ps aux
    pub async fn find_kafka_process<F>(executor: F) -> Result<KafkaProcessInfo>
    where
        F: Fn(&str) -> Result<String>,
    {
        debug!("Finding Kafka Java process...");
        
        // Look for Kafka main class in Java processes
        info!("   Trying primary pattern: java.*kafka\\.Kafka");
        let ps_output = executor("ps aux | grep -E 'java.*kafka\\.Kafka[^a-zA-Z]' | grep -v grep")?;
        
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
                let alt_output = executor(pattern)?;
                if !alt_output.trim().is_empty() {
                    info!("   ✓ Found match with pattern: {}", name);
                    return Self::parse_process_info(&alt_output);
                } else {
                    info!("   ✗ No match with pattern: {}", name);
                }
            }
            
            warn!("   All patterns failed. Let's see what Java processes are running:");
            let all_java = executor("ps aux | grep java | grep -v grep")?;
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
        Self::parse_process_info(&ps_output)
    }

    /// Parse ps aux output to extract Kafka process information
    pub fn parse_process_info(ps_output: &str) -> Result<KafkaProcessInfo> {
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
        let config_paths = Self::extract_config_paths(&command_line);
        info!("   Found {} config paths: {:?}", config_paths.len(), config_paths);
        
        // Extract log4j path from command line
        let log4j_path = Self::extract_log4j_path(&command_line);
        info!("   Log4j path: {:?}", log4j_path);

        Ok(KafkaProcessInfo {
            pid,
            command_line,
            service_name: None, // Will be filled in later steps
            config_paths,
            log4j_path,
            environment_vars: HashMap::new(), // Will be filled later
        })
    }

    /// Extract configuration file paths from command line
    fn extract_config_paths(cmdline: &str) -> Vec<PathBuf> {
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
    fn extract_log4j_path(cmdline: &str) -> Option<PathBuf> {
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
}