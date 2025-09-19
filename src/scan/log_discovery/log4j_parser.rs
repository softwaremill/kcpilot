use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
// No tracing needed for this basic parser module

use super::types::{LogOutputInfo, LogFileLocation};

/// Parser for log4j configuration files
pub struct Log4jParser;

impl Log4jParser {
    /// Parse log4j configuration to extract log output information
    pub fn parse(log4j_content: &str, env_vars: &HashMap<String, String>) -> Result<LogOutputInfo> {
        let mut log_files = Vec::new();
        let mut uses_stdout = false;
        
        for line in log4j_content.lines() {
            let line = line.trim();
            
            // Look for file appenders
            if line.contains("appender") && line.contains(".File=") {
                if let Some(eq_pos) = line.find('=') {
                    let mut file_path = line[eq_pos + 1..].trim().to_string();
                    
                    // Resolve environment variables
                    file_path = Self::resolve_env_vars(&file_path, env_vars);
                    
                    let log_type = Self::determine_log_type(&file_path);
                    let appender_name = Self::extract_appender_name(line);
                    
                    log_files.push(LogFileLocation {
                        path: PathBuf::from(file_path),
                        log_type,
                        appender_name,
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
            log4j_analysis: "Basic parsing".to_string(),
        })
    }

    /// Resolve environment variables in paths
    fn resolve_env_vars(path: &str, env_vars: &HashMap<String, String>) -> String {
        let mut resolved_path = path.to_string();
        
        // Resolve known environment variables
        for (key, value) in env_vars {
            resolved_path = resolved_path.replace(&format!("${{{}}}", key), value);
            resolved_path = resolved_path.replace(&format!("${}", key), value);
        }
        
        // Common fallback substitutions for Kafka
        if resolved_path.contains("${kafka.logs.dir}") || resolved_path.contains("${log.dir}") {
            // Try common Kafka log directory locations
            let possible_log_dirs = vec![
                "/opt/kafka/logs",        // Common for manual installs
                "/var/log/kafka",         // Common for package installs  
                "/usr/local/kafka/logs",  // Alternative install location
                "/home/kafka/logs",       // User-based installs
            ];
            
            for log_dir in possible_log_dirs {
                resolved_path = resolved_path.replace("${kafka.logs.dir}", log_dir);
                resolved_path = resolved_path.replace("${log.dir}", log_dir);
                break; // Use the first one for now
            }
        }
        
        resolved_path
    }

    /// Determine log type based on file path
    fn determine_log_type(file_path: &str) -> String {
        if file_path.contains("server") {
            "server".to_string()
        } else if file_path.contains("controller") {
            "controller".to_string()
        } else if file_path.contains("state") {
            "state-change".to_string()
        } else if file_path.contains("request") {
            "request".to_string()
        } else if file_path.contains("gc") {
            "gc".to_string()
        } else {
            "custom".to_string()
        }
    }

    /// Extract appender name from log4j configuration line
    fn extract_appender_name(line: &str) -> String {
        // Try to extract appender name from patterns like:
        // log4j.appender.serverAppender.File=...
        // log4j2.appender.file.fileName=...
        
        if line.contains(".appender.") {
            let parts: Vec<&str> = line.split('.').collect();
            for (i, part) in parts.iter().enumerate() {
                if *part == "appender" && i + 1 < parts.len() {
                    return parts[i + 1].to_string();
                }
            }
        }
        
        "detected".to_string()
    }

    /// Parse JSON response from LLM analysis
    pub fn parse_llm_response(response: &str) -> Result<LogOutputInfo> {
        // Try multiple strategies to extract JSON
        let json_str = Self::extract_json(response)
            .ok_or_else(|| anyhow::anyhow!("Could not extract JSON from response"))?;
        
        let json_value: serde_json::Value = serde_json::from_str(&json_str)?;
        
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
        
        let uses_stdout = json_value.get("uses_stdout")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        
        let uses_journald = json_value.get("uses_journald")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        
        let log4j_analysis = json_value.get("analysis")
            .and_then(|v| v.as_str())
            .unwrap_or("LLM analysis")
            .to_string();
        
        Ok(LogOutputInfo {
            log_files,
            uses_stdout,
            uses_journald,
            log4j_analysis,
        })
    }

    /// Extract JSON from LLM response using multiple strategies
    fn extract_json(response: &str) -> Option<String> {
        // Strategy 1: Look for JSON code block
        if let Some(start) = response.find("```json") {
            let start = start + 7; // Skip "```json"
            if let Some(end) = response[start..].find("```") {
                return Some(response[start..start + end].trim().to_string());
            }
        }
        
        // Strategy 2: Find the largest JSON object
        let mut best_json = None;
        let mut best_len = 0;
        
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
                    // Quick validation
                    if json_candidate.contains("log_files") {
                        best_json = Some(json_candidate.to_string());
                        best_len = json_candidate.len();
                    }
                }
            }
        }
        
        best_json.or_else(|| {
            // Strategy 3: Simple extraction
            let json_start = response.find('{')?;
            let json_end = response.rfind('}').map(|pos| pos + 1)?;
            Some(response[json_start..json_end].to_string())
        })
    }
}