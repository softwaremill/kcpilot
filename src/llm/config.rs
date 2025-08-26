use serde::{Deserialize, Serialize};
use std::env;

/// Configuration for LLM service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// OpenAI API Key
    pub api_key: String,
    
    /// Model to use (e.g., "gpt-4-turbo-preview", "gpt-3.5-turbo")
    pub model: String,
    
    /// Optional API base URL for custom endpoints
    pub api_base: Option<String>,
    
    /// Request timeout in seconds
    pub timeout_secs: u64,
    
    /// Maximum tokens for response (maps to max_completion_tokens for newer models)
    pub max_tokens: u16,
    
    /// Temperature for creativity (0.0-1.0)
    /// Note: Newer models (GPT-4o, GPT-4-turbo) only support default value (1.0)
    pub temperature: f32,
    
    /// Enable debug logging
    pub debug: bool,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: "gpt-4-turbo-preview".to_string(),
            api_base: None,
            timeout_secs: 300,  // Increased from 60 to 300 seconds (5 minutes)
            max_tokens: 8000,  // Increased from 4000 to 8000 for larger responses
            temperature: 0.3,
            debug: false,
        }
    }
}

impl LlmConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self, String> {
        // Load .env file if it exists
        let _ = dotenv::dotenv();
        
        let api_key = env::var("OPENAI_API_KEY")
            .map_err(|_| "OPENAI_API_KEY not found in environment. Please set it in .env file or environment variables.")?;
        
        if api_key.is_empty() {
            return Err("OPENAI_API_KEY is empty".to_string());
        }
        
        let mut config = Self {
            api_key,
            ..Default::default()
        };
        
        // Override defaults with environment variables if present
        if let Ok(model) = env::var("OPENAI_MODEL") {
            config.model = model;
        }
        
        if let Ok(api_base) = env::var("OPENAI_API_BASE") {
            config.api_base = Some(api_base);
        }
        
        if let Ok(timeout) = env::var("LLM_REQUEST_TIMEOUT") {
            if let Ok(timeout_secs) = timeout.parse::<u64>() {
                config.timeout_secs = timeout_secs;
            }
        }
        
        if let Ok(max_tokens) = env::var("LLM_MAX_TOKENS") {
            if let Ok(tokens) = max_tokens.parse::<u16>() {
                config.max_tokens = tokens;
            }
        }
        
        if let Ok(temperature) = env::var("LLM_TEMPERATURE") {
            if let Ok(temp) = temperature.parse::<f32>() {
                if temp >= 0.0 && temp <= 1.0 {
                    config.temperature = temp;
                }
            }
        }
        
        if let Ok(debug) = env::var("LLM_DEBUG") {
            config.debug = debug.to_lowercase() == "true" || debug == "1";
        }
        
        Ok(config)
    }
    
    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.api_key.is_empty() {
            return Err("API key is empty".to_string());
        }
        
        if self.temperature < 0.0 || self.temperature > 1.0 {
            return Err(format!("Temperature must be between 0.0 and 1.0, got {}", self.temperature));
        }
        
        if self.max_tokens == 0 {
            return Err("Max tokens must be greater than 0".to_string());
        }
        
        // Validate model name
        let valid_models = vec![
            "gpt-4", "gpt-4-turbo", "gpt-4-turbo-preview", "gpt-4o", "gpt-4o-mini",
            "gpt-3.5-turbo", "gpt-3.5-turbo-16k"
        ];
        
        let model_lower = self.model.to_lowercase();
        
        // Check for invalid model names
        if model_lower.contains("gpt-5") {
            eprintln!("WARNING: Model '{}' does not exist yet. Consider using 'gpt-4-turbo-preview' or 'gpt-4o' instead.", self.model);
            eprintln!("Set OPENAI_MODEL environment variable to override the model.");
        } else if !valid_models.iter().any(|&m| model_lower.contains(m)) {
            eprintln!("WARNING: Model '{}' may not be valid. Known models: {:?}", self.model, valid_models);
        }
        
        Ok(())
    }
}
