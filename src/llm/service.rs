use crate::llm::config::LlmConfig;
use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestUserMessageArgs, ChatCompletionRequestAssistantMessageArgs,
        CreateChatCompletionRequestArgs,
    },
    Client,
};
use serde_json::Value;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;
use std::time::Duration;
use thiserror::Error;
use chrono::Local;

/// Errors that can occur in LLM service
#[derive(Debug, Error)]
pub enum LlmServiceError {
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("API error: {0}")]
    ApiError(String),
    
    #[error("Parse error: {0}")]
    ParseError(String),
    
    #[error("Request timeout after {0} seconds. Try increasing timeout with --llm-timeout flag")]
    Timeout(u64),
    
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    
    #[error("Other error: {0}")]
    Other(String),
}

/// Service for interacting with LLM APIs
pub struct LlmService {
    client: Client<OpenAIConfig>,
    config: LlmConfig,
    debug_file: Option<Mutex<std::fs::File>>,
}

impl LlmService {
    /// Create a new LLM service from configuration
    pub fn new(config: LlmConfig) -> Result<Self, LlmServiceError> {
        config.validate()
            .map_err(|e| LlmServiceError::ConfigError(e))?;
        
        let mut openai_config = OpenAIConfig::new()
            .with_api_key(&config.api_key);
        
        if let Some(api_base) = &config.api_base {
            openai_config = openai_config.with_api_base(api_base);
        }
        
        let client = Client::with_config(openai_config);
        
        Ok(Self { 
            client, 
            config,
            debug_file: None,
        })
    }
    
    /// Create a service from environment variables
    pub fn from_env() -> Result<Self, LlmServiceError> {
        let config = LlmConfig::from_env()
            .map_err(|e| LlmServiceError::ConfigError(e))?;
        Self::new(config)
    }
    
    /// Create a service from environment variables with debug logging
    pub fn from_env_with_debug(enable_debug: bool) -> Result<Self, LlmServiceError> {
        let config = LlmConfig::from_env()
            .map_err(|e| LlmServiceError::ConfigError(e))?;
        let service = Self::new(config)?;
        service.with_debug_file(enable_debug)
    }
    
    /// Create a service from environment variables with options
    pub fn from_env_with_options(enable_debug: bool, timeout_secs: u64) -> Result<Self, LlmServiceError> {
        let mut config = LlmConfig::from_env()
            .map_err(|e| LlmServiceError::ConfigError(e))?;
        
        // Override timeout with CLI flag value
        config.timeout_secs = timeout_secs;
        
        let service = Self::new(config)?;
        service.with_debug_file(enable_debug)
    }
    
    /// Enable debug logging to file
    pub fn with_debug_file(mut self, enable_debug: bool) -> Result<Self, LlmServiceError> {
        if enable_debug {
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open("llmdbg.txt")
                .map_err(|e| LlmServiceError::Other(format!("Failed to open debug file: {}", e)))?;
            self.debug_file = Some(Mutex::new(file));
            self.log_debug("==================================================");
            self.log_debug("=== LLM Debug Logging Session Started ===");
            self.log_debug("==================================================");
            self.log_debug(&format!("Model: {}", self.config.model));
            self.log_debug(&format!("Max Tokens: {}", self.config.max_tokens));
            self.log_debug(&format!("Temperature: {}", self.config.temperature));
            self.log_debug(&format!("Timeout: {} seconds", self.config.timeout_secs));
            if let Some(api_base) = &self.config.api_base {
                self.log_debug(&format!("API Base: {}", api_base));
            }
            self.log_debug("==================================================\n");
        }
        Ok(self)
    }
    
    /// Log debug information to file if debug mode is enabled
    fn log_debug(&self, message: &str) {
        if let Some(ref file_mutex) = self.debug_file {
            if let Ok(mut file) = file_mutex.lock() {
                let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
                let _ = writeln!(file, "[{}] {}", timestamp, message);
                let _ = file.flush();
            }
        }
    }
    
    /// Send a chat completion request
    pub async fn chat(&self, messages: Vec<ChatMessage>) -> Result<String, LlmServiceError> {
        // Log the request if debug mode is enabled
        if self.debug_file.is_some() {
            self.log_debug("\n==== NEW LLM REQUEST ====");
            self.log_debug(&format!("Timestamp: {}", Local::now().format("%Y-%m-%d %H:%M:%S")));
            self.log_debug(&format!("Model: {}", self.config.model));
            self.log_debug(&format!("Max tokens: {}", self.config.max_tokens));
            self.log_debug(&format!("Number of messages: {}", messages.len()));
            self.log_debug("--- Messages ---");
            for (i, msg) in messages.iter().enumerate() {
                let role = match msg {
                    ChatMessage::System(_) => "System",
                    ChatMessage::User(_) => "User",
                    ChatMessage::Assistant(_) => "Assistant",
                };
                let content = match msg {
                    ChatMessage::System(c) | ChatMessage::User(c) | ChatMessage::Assistant(c) => c,
                };
                self.log_debug(&format!("\n[Message {}] Role: {}", i + 1, role));
                self.log_debug(&format!("[Message {}] Content:\n{}", i + 1, content));
            }
            self.log_debug("\n--- End of Messages ---");
        }
        
        let openai_messages = messages
            .into_iter()
            .map(|msg| msg.into_openai_message())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| LlmServiceError::Other(e))?;
        
        // Build request with conditional temperature support
        // Newer models (gpt-4o, gpt-4-turbo, etc.) only support default temperature
        let mut request_builder = CreateChatCompletionRequestArgs::default();
        request_builder
            .model(&self.config.model)
            .messages(openai_messages)
            .max_completion_tokens(self.config.max_tokens);
        
        // Only set custom temperature for models that support it
        // Newer models like gpt-4o, gpt-4-turbo only accept default temperature (1.0)
        let model_lower = self.config.model.to_lowercase();
        let skip_temperature = model_lower.contains("gpt-4o") 
            || model_lower.contains("gpt-4-turbo")
            || model_lower.contains("gpt-5");
        
        if !skip_temperature {
            request_builder.temperature(self.config.temperature);
            if self.config.debug {
                tracing::debug!("Setting temperature to {}", self.config.temperature);
            }
        } else if self.config.debug {
            tracing::debug!("Skipping temperature parameter for model {} (uses default 1.0)", self.config.model);
        }
        
        let request = request_builder
            .build()
            .map_err(|e| LlmServiceError::ApiError(e.to_string()))?;
        
        if self.config.debug {
            tracing::debug!("Sending request to OpenAI: model={}, messages_count={}", 
                          self.config.model, request.messages.len());
        }
        
        // Log that we're sending the request
        if self.debug_file.is_some() {
            self.log_debug(&format!("Sending request to LLM API (timeout: {} seconds)...", self.config.timeout_secs));
        }
        
        let response = tokio::time::timeout(
            Duration::from_secs(self.config.timeout_secs),
            self.client.chat().create(request)
        )
        .await
        .map_err(|_| {
            let timeout_msg = format!(
                "Request timed out after {} seconds. Consider increasing timeout with --llm-timeout flag or LLM_REQUEST_TIMEOUT env var", 
                self.config.timeout_secs
            );
            if self.debug_file.is_some() {
                self.log_debug(&format!("ERROR: {}", timeout_msg));
            }
            tracing::error!("{}", timeout_msg);
            LlmServiceError::Timeout(self.config.timeout_secs)
        })?
        .map_err(|e| {
            if self.debug_file.is_some() {
                self.log_debug(&format!("ERROR: API request failed: {}", e));
            }
            if e.to_string().contains("rate limit") {
                LlmServiceError::RateLimitExceeded
            } else {
                LlmServiceError::ApiError(e.to_string())
            }
        })?;
        
        // Log raw response details
        if self.debug_file.is_some() {
            self.log_debug("--- LLM Response Received ---");
            self.log_debug(&format!("Number of choices: {}", response.choices.len()));
            if let Some(usage) = &response.usage {
                self.log_debug(&format!("Tokens used - Prompt: {}, Completion: {}, Total: {}", 
                    usage.prompt_tokens, usage.completion_tokens, usage.total_tokens));
            }
        }
        
        let content = response
            .choices
            .first()
            .and_then(|choice| choice.message.content.as_ref())
            .ok_or_else(|| {
                if self.debug_file.is_some() {
                    self.log_debug("ERROR: No response content in API response");
                    self.log_debug("This usually means the model hit the token limit without generating output.");
                    self.log_debug("Try: 1) Using a shorter prompt, 2) Increasing max_tokens, 3) Using a different model");
                }
                LlmServiceError::ParseError("No response content - likely hit token limit".to_string())
            })?
            .to_string();
        
        // Check for empty response content
        if content.is_empty() {
            if self.debug_file.is_some() {
                self.log_debug("ERROR: Response content is empty");
                self.log_debug("The model returned an empty response, which often indicates:");
                self.log_debug("1. Token limit reached before any output could be generated");
                self.log_debug("2. Model issue (check if model name is valid)");
                self.log_debug("3. API configuration issue");
                if let Some(usage) = &response.usage {
                    self.log_debug(&format!("Token usage: prompt={}, completion={}, total={}", 
                        usage.prompt_tokens, usage.completion_tokens, usage.total_tokens));
                    if usage.completion_tokens >= (self.config.max_tokens as u32).saturating_sub(10) {
                        self.log_debug("WARNING: Completion tokens nearly at max limit - increase max_tokens!");
                    }
                }
            }
            return Err(LlmServiceError::ParseError(
                format!("Empty response from model. Token limit may be too low (current: {}). Try increasing with LLM_MAX_TOKENS env var.", 
                    self.config.max_tokens)
            ));
        }
        
        // Log the response content if debug mode is enabled
        if self.debug_file.is_some() {
            self.log_debug(&format!("\n--- Response Content ---"));
            self.log_debug(&format!("Response length: {} characters", content.len()));
            self.log_debug(&format!("Response preview (first 500 chars): {}", 
                if content.len() > 500 { 
                    format!("{}...", &content[..500]) 
                } else { 
                    content.clone() 
                }
            ));
            self.log_debug(&format!("\n--- Full Response ---\n{}", content));
            self.log_debug(&format!("\n==== END OF REQUEST/RESPONSE ===="));
            self.log_debug(&format!("Completed at: {}\n", Local::now().format("%Y-%m-%d %H:%M:%S")));
        }
        
        if self.config.debug {
            tracing::debug!("Received response: {} chars", content.len());
        }
        
        Ok(content)
    }
    
    /// Analyze Kafka logs using LLM
    pub async fn analyze_logs(&self, logs: &str, context: Option<&str>) -> Result<LogAnalysis, LlmServiceError> {
        let system_prompt = "You are an expert Kafka administrator analyzing Kafka logs. \
                           Identify issues, patterns, and provide actionable insights.";
        
        let user_prompt = if let Some(ctx) = context {
            format!("Context: {}\n\nPlease analyze these Kafka logs:\n{}", ctx, logs)
        } else {
            format!("Please analyze these Kafka logs:\n{}", logs)
        };
        
        let messages = vec![
            ChatMessage::system(system_prompt),
            ChatMessage::user(&user_prompt),
        ];
        
        let response = self.chat(messages).await?;
        
        // Try to parse as JSON if the response is structured
        if response.trim().starts_with('{') {
            serde_json::from_str(&response)
                .map_err(|e| LlmServiceError::ParseError(e.to_string()))
        } else {
            // Return as plain text analysis
            Ok(LogAnalysis {
                summary: response.clone(),
                issues: vec![],
                recommendations: vec![],
                severity: "info".to_string(),
            })
        }
    }
    
    /// Generate remediation script for a specific issue
    pub async fn generate_remediation(&self, 
                                     issue: &str, 
                                     evidence: &str,
                                     constraints: Option<&str>) -> Result<RemediationScript, LlmServiceError> {
        let system_prompt = "You are an expert Kafka administrator. Generate safe, \
                           idempotent remediation scripts for Kafka issues. Always include \
                           validation steps and rollback procedures.";
        
        let user_prompt = format!(
            "Issue: {}\n\nEvidence:\n{}\n\n{}Generate a remediation script.",
            issue,
            evidence,
            constraints.map(|c| format!("Constraints: {}\n\n", c)).unwrap_or_default()
        );
        
        let messages = vec![
            ChatMessage::system(system_prompt),
            ChatMessage::user(&user_prompt),
        ];
        
        let response = self.chat(messages).await?;
        
        // Parse the response to extract script and metadata
        Ok(RemediationScript {
            script: response.clone(),
            risk_level: "medium".to_string(),
            estimated_duration_minutes: 10,
            requires_downtime: false,
        })
    }
    
    /// Perform root cause analysis
    pub async fn root_cause_analysis(&self, 
                                    symptoms: &[String],
                                    evidence: &Value) -> Result<RootCauseAnalysis, LlmServiceError> {
        let system_prompt = "You are an expert in distributed systems and Kafka. \
                           Perform root cause analysis based on symptoms and evidence. \
                           Provide a structured analysis with confidence levels.";
        
        let user_prompt = format!(
            "Symptoms:\n{}\n\nEvidence:\n{}",
            symptoms.join("\n- "),
            serde_json::to_string_pretty(evidence).unwrap_or_default()
        );
        
        let messages = vec![
            ChatMessage::system(system_prompt),
            ChatMessage::user(&user_prompt),
        ];
        
        let response = self.chat(messages).await?;
        
        // Simple parsing - in production, you'd want structured output
        Ok(RootCauseAnalysis {
            root_cause: response.clone(),
            confidence: 0.75,
            contributing_factors: vec![],
            timeline: None,
        })
    }
    
    /// Get configuration recommendations based on workload
    pub async fn optimize_configuration(&self, 
                                       current_config: &Value,
                                       metrics: &Value,
                                       workload_description: &str) -> Result<Vec<ConfigRecommendation>, LlmServiceError> {
        let system_prompt = "You are a Kafka performance tuning expert. \
                           Analyze configurations and metrics to provide optimization recommendations. \
                           Focus on practical, safe changes with clear justifications.";
        
        let user_prompt = format!(
            "Workload: {}\n\nCurrent Configuration:\n{}\n\nMetrics:\n{}",
            workload_description,
            serde_json::to_string_pretty(current_config).unwrap_or_default(),
            serde_json::to_string_pretty(metrics).unwrap_or_default()
        );
        
        let messages = vec![
            ChatMessage::system(system_prompt),
            ChatMessage::user(&user_prompt),
        ];
        
        let response = self.chat(messages).await?;
        
        // Parse recommendations from response
        // In production, you'd want to enforce structured output format
        Ok(vec![ConfigRecommendation {
            parameter: "example.parameter".to_string(),
            current_value: "current".to_string(),
            recommended_value: "recommended".to_string(),
            justification: response,
            impact: "Performance improvement".to_string(),
        }])
    }
}

/// Chat message for LLM interactions
#[derive(Debug, Clone)]
pub enum ChatMessage {
    System(String),
    User(String),
    Assistant(String),
}

impl ChatMessage {
    pub fn system(content: &str) -> Self {
        Self::System(content.to_string())
    }
    
    pub fn user(content: &str) -> Self {
        Self::User(content.to_string())
    }
    
    pub fn assistant(content: &str) -> Self {
        Self::Assistant(content.to_string())
    }
    
    fn into_openai_message(self) -> Result<ChatCompletionRequestMessage, String> {
        match self {
            ChatMessage::System(content) => {
                ChatCompletionRequestSystemMessageArgs::default()
                    .content(content)
                    .build()
                    .map(ChatCompletionRequestMessage::System)
                    .map_err(|e| e.to_string())
            }
            ChatMessage::User(content) => {
                ChatCompletionRequestUserMessageArgs::default()
                    .content(content)
                    .build()
                    .map(ChatCompletionRequestMessage::User)
                    .map_err(|e| e.to_string())
            }
            ChatMessage::Assistant(content) => {
                ChatCompletionRequestAssistantMessageArgs::default()
                    .content(content)
                    .build()
                    .map(ChatCompletionRequestMessage::Assistant)
                    .map_err(|e| e.to_string())
            }
        }
    }
}

/// Log analysis result
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LogAnalysis {
    pub summary: String,
    pub issues: Vec<String>,
    pub recommendations: Vec<String>,
    pub severity: String,
}

/// Remediation script
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RemediationScript {
    pub script: String,
    pub risk_level: String,
    pub estimated_duration_minutes: u32,
    pub requires_downtime: bool,
}

/// Root cause analysis result
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RootCauseAnalysis {
    pub root_cause: String,
    pub confidence: f64,
    pub contributing_factors: Vec<String>,
    pub timeline: Option<String>,
}

/// Configuration recommendation
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConfigRecommendation {
    pub parameter: String,
    pub current_value: String,
    pub recommended_value: String,
    pub justification: String,
    pub impact: String,
}
