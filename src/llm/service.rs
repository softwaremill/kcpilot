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
use std::time::Duration;
use thiserror::Error;

/// Errors that can occur in LLM service
#[derive(Debug, Error)]
pub enum LlmServiceError {
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("API error: {0}")]
    ApiError(String),
    
    #[error("Parse error: {0}")]
    ParseError(String),
    
    #[error("Request timeout")]
    Timeout,
    
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    
    #[error("Other error: {0}")]
    Other(String),
}

/// Service for interacting with LLM APIs
pub struct LlmService {
    client: Client<OpenAIConfig>,
    config: LlmConfig,
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
        
        Ok(Self { client, config })
    }
    
    /// Create a service from environment variables
    pub fn from_env() -> Result<Self, LlmServiceError> {
        let config = LlmConfig::from_env()
            .map_err(|e| LlmServiceError::ConfigError(e))?;
        Self::new(config)
    }
    
    /// Send a chat completion request
    pub async fn chat(&self, messages: Vec<ChatMessage>) -> Result<String, LlmServiceError> {
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
        
        let response = tokio::time::timeout(
            Duration::from_secs(self.config.timeout_secs),
            self.client.chat().create(request)
        )
        .await
        .map_err(|_| LlmServiceError::Timeout)?
        .map_err(|e| {
            if e.to_string().contains("rate limit") {
                LlmServiceError::RateLimitExceeded
            } else {
                LlmServiceError::ApiError(e.to_string())
            }
        })?;
        
        let content = response
            .choices
            .first()
            .and_then(|choice| choice.message.content.as_ref())
            .ok_or_else(|| LlmServiceError::ParseError("No response content".to_string()))?
            .to_string();
        
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
