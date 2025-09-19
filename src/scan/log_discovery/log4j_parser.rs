use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_log4j_basic() {
        let log4j_content = r#"
log4j.appender.serverAppender=org.apache.log4j.RollingFileAppender
log4j.appender.serverAppender.File=/var/log/kafka/server.log
log4j.appender.controllerAppender=org.apache.log4j.RollingFileAppender
log4j.appender.controllerAppender.File=/var/log/kafka/controller.log
"#;
        let env_vars = HashMap::new();

        let result = Log4jParser::parse(log4j_content, &env_vars).unwrap();

        assert_eq!(result.log_files.len(), 2);
        assert_eq!(result.log_files[0].path, PathBuf::from("/var/log/kafka/server.log"));
        assert_eq!(result.log_files[0].log_type, "server");
        assert_eq!(result.log_files[0].appender_name, "serverAppender");
        assert_eq!(result.log_files[1].path, PathBuf::from("/var/log/kafka/controller.log"));
        assert_eq!(result.log_files[1].log_type, "controller");
        assert!(!result.uses_stdout);
        assert!(!result.uses_journald);
    }

    #[test]
    fn test_parse_log4j_with_console() {
        let log4j_content = r#"
log4j.appender.console=org.apache.log4j.ConsoleAppender
log4j.appender.serverAppender.File=/var/log/kafka/server.log
"#;
        let env_vars = HashMap::new();

        let result = Log4jParser::parse(log4j_content, &env_vars).unwrap();

        assert_eq!(result.log_files.len(), 1);
        assert!(result.uses_stdout);
        assert!(result.uses_journald);
    }

    #[test]
    fn test_resolve_env_vars() {
        let mut env_vars = HashMap::new();
        env_vars.insert("LOG_DIR".to_string(), "/custom/log/path".to_string());
        env_vars.insert("APP_NAME".to_string(), "myapp".to_string());

        let path = "${LOG_DIR}/${APP_NAME}.log";
        let resolved = Log4jParser::resolve_env_vars(path, &env_vars);
        assert_eq!(resolved, "/custom/log/path/myapp.log");
    }

    #[test]
    fn test_resolve_env_vars_kafka_defaults() {
        let env_vars = HashMap::new();
        let path = "${kafka.logs.dir}/server.log";
        let resolved = Log4jParser::resolve_env_vars(path, &env_vars);
        assert_eq!(resolved, "/opt/kafka/logs/server.log");
    }

    #[test]
    fn test_determine_log_type() {
        assert_eq!(Log4jParser::determine_log_type("/var/log/kafka/server.log"), "server");
        assert_eq!(Log4jParser::determine_log_type("/var/log/kafka/controller.log"), "controller");
        assert_eq!(Log4jParser::determine_log_type("/var/log/kafka/state-change.log"), "state-change");
        assert_eq!(Log4jParser::determine_log_type("/var/log/kafka/request.log"), "request");
        assert_eq!(Log4jParser::determine_log_type("/var/log/kafka/gc.log"), "gc");
        assert_eq!(Log4jParser::determine_log_type("/var/log/kafka/custom.log"), "custom");
    }

    #[test]
    fn test_extract_appender_name() {
        let line = "log4j.appender.serverAppender.File=/var/log/kafka/server.log";
        let name = Log4jParser::extract_appender_name(line);
        assert_eq!(name, "serverAppender");

        let line2 = "log4j2.appender.fileAppender.fileName=/opt/kafka/logs/kafka.log";
        let name2 = Log4jParser::extract_appender_name(line2);
        assert_eq!(name2, "fileAppender");

        let line3 = "some.other.config=value";
        let name3 = Log4jParser::extract_appender_name(line3);
        assert_eq!(name3, "detected");
    }

    #[test]
    fn test_parse_llm_response() {
        let response = r#"
{
  "log_files": [
    {
      "path": "/var/log/kafka/server.log",
      "log_type": "server",
      "appender_name": "serverAppender"
    },
    {
      "path": "/var/log/kafka/controller.log",
      "log_type": "controller", 
      "appender_name": "controllerAppender"
    }
  ],
  "uses_stdout": false,
  "uses_journald": false,
  "analysis": "Found 2 log files configured"
}
"#;

        let result = Log4jParser::parse_llm_response(response).unwrap();

        assert_eq!(result.log_files.len(), 2);
        assert_eq!(result.log_files[0].path, PathBuf::from("/var/log/kafka/server.log"));
        assert_eq!(result.log_files[0].log_type, "server");
        assert_eq!(result.log_files[0].appender_name, "serverAppender");
        assert!(!result.uses_stdout);
        assert!(!result.uses_journald);
        assert_eq!(result.log4j_analysis, "Found 2 log files configured");
    }

    #[test]
    fn test_parse_llm_response_with_code_block() {
        let response = r#"
Here's the analysis:

```json
{
  "log_files": [
    {
      "path": "/opt/kafka/logs/server.log",
      "log_type": "server",
      "appender_name": "serverLog"
    }
  ],
  "uses_stdout": true,
  "uses_journald": true,
  "analysis": "Console and file output detected"
}
```

That's the result.
"#;

        let result = Log4jParser::parse_llm_response(response).unwrap();

        assert_eq!(result.log_files.len(), 1);
        assert_eq!(result.log_files[0].path, PathBuf::from("/opt/kafka/logs/server.log"));
        assert!(result.uses_stdout);
        assert!(result.uses_journald);
    }

    #[test]
    fn test_extract_json_code_block() {
        let response = "```json\n{\"test\": \"value\"}\n```";
        let json = Log4jParser::extract_json(response).unwrap();
        assert_eq!(json, "{\"test\": \"value\"}");
    }

    #[test]
    fn test_extract_json_simple() {
        let response = "Some text {\"log_files\": []} more text";
        let json = Log4jParser::extract_json(response).unwrap();
        assert_eq!(json, "{\"log_files\": []}");
    }

    #[test]
    fn test_extract_json_nested() {
        let response = r#"Text {"outer": {"log_files": [{"inner": "value"}]}} more"#;
        let json = Log4jParser::extract_json(response).unwrap();
        assert_eq!(json, r#"{"outer": {"log_files": [{"inner": "value"}]}}"#);
    }
}