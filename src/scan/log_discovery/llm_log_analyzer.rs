use anyhow::Result;
use std::collections::HashMap;
// PathBuf not needed here
use tracing::{info, warn};

use crate::llm::{LlmService, service::ChatMessage};
use super::types::LogOutputInfo;
use super::log4j_parser::Log4jParser;

/// LLM-based analyzer for log4j configuration
pub struct LlmLogAnalyzer;

impl LlmLogAnalyzer {
    /// Analyze log4j configuration using LLM
    pub async fn analyze(
        log4j_content: &str,
        env_vars: &HashMap<String, String>,
    ) -> Result<LogOutputInfo> {
        info!("Analyzing log4j configuration with LLM...");
        
        // Prepare LLM prompt
        let prompt = Self::create_analysis_prompt(log4j_content, env_vars);
        
        // Use LLM service to analyze
        match LlmService::from_env() {
            Ok(llm_service) => {
                let messages = vec![ChatMessage::user(&prompt)];
                match llm_service.chat(messages).await {
                    Ok(llm_response) => {
                        match Log4jParser::parse_llm_response(&llm_response) {
                            Ok(result) if result.log_files.is_empty() => {
                                warn!("LLM found no log files, falling back to basic parsing");
                                Log4jParser::parse(log4j_content, env_vars)
                            }
                            Ok(result) => Ok(result),
                            Err(e) => {
                                warn!("LLM response parsing failed: {}, falling back to basic parsing", e);
                                Log4jParser::parse(log4j_content, env_vars)
                            }
                        }
                    }
                    Err(e) => {
                        warn!("LLM analysis failed: {}, falling back to basic parsing", e);
                        Log4jParser::parse(log4j_content, env_vars)
                    }
                }
            }
            Err(e) => {
                warn!("LLM service not available: {}, using basic parsing", e);
                Log4jParser::parse(log4j_content, env_vars)
            }
        }
    }

    /// Create LLM prompt for log4j analysis
    fn create_analysis_prompt(log4j_content: &str, env_vars: &HashMap<String, String>) -> String {
        let env_vars_str = env_vars.iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"Analyze this Kafka log4j configuration and determine where logs are written.

Environment Variables:
{}

Log4j Configuration:
{}

IMPORTANT: Respond with ONLY valid JSON, no explanations or additional text.

JSON Structure Required:
{{
  "log_files": [
    {{
      "path": "/resolved/path/to/logfile.log",
      "log_type": "server|controller|state-change|request|gc|custom",
      "appender_name": "appender_name"
    }}
  ],
  "uses_stdout": true/false,
  "uses_journald": true/false,
  "analysis": "Brief explanation of the log configuration"
}}

Requirements:
1. Resolve variable substitutions like ${{kafka.logs.dir}}, ${{LOG_DIR}} using environment variables
2. Identify if logs go to stdout/console (appears in journald)
3. Classify log types by appender names/file names
4. Use full resolved file paths
5. Return ONLY the JSON object, no other text"#,
            env_vars_str, log4j_content
        )
    }
}