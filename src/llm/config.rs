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
            model: "gpt-4o".to_string(),
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
        Self::from_env_internal(true)
    }
    
    /// Load configuration from environment variables with optional dotenv loading
    #[cfg(test)]
    fn from_env_no_dotenv() -> Result<Self, String> {
        Self::from_env_internal(false)
    }
    
    fn from_env_internal(load_dotenv: bool) -> Result<Self, String> {
        // Load .env file if it exists and not disabled
        if load_dotenv {
            let _ = dotenv::dotenv();
        }
        
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use serial_test::serial;

    #[test]
    fn test_default_config() {
        let config = LlmConfig::default();
        
        assert_eq!(config.api_key, "");
        assert_eq!(config.model, "gpt-4o");
        assert_eq!(config.api_base, None);
        assert_eq!(config.timeout_secs, 300);
        assert_eq!(config.max_tokens, 8000);
        assert_eq!(config.temperature, 0.3);
        assert!(!config.debug);
    }

    #[test]
    fn test_validate_success() {
        let config = LlmConfig {
            api_key: "test-key".to_string(),
            model: "gpt-4".to_string(),
            api_base: None,
            timeout_secs: 60,
            max_tokens: 1000,
            temperature: 0.5,
            debug: false,
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_empty_api_key() {
        let config = LlmConfig {
            api_key: "".to_string(),
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("API key is empty"));
    }

    #[test]
    fn test_validate_invalid_temperature() {
        let config = LlmConfig {
            api_key: "test-key".to_string(),
            temperature: 1.5,
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Temperature must be between"));
    }

    #[test]
    fn test_validate_zero_max_tokens() {
        let config = LlmConfig {
            api_key: "test-key".to_string(),
            max_tokens: 0,
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Max tokens must be greater than 0"));
    }

    fn setup_clean_env() {
        env::remove_var("OPENAI_API_KEY");
        env::remove_var("OPENAI_MODEL");
        env::remove_var("OPENAI_API_BASE");
        env::remove_var("LLM_REQUEST_TIMEOUT");
        env::remove_var("LLM_MAX_TOKENS");
        env::remove_var("LLM_TEMPERATURE");
        env::remove_var("LLM_DEBUG");
    }

    #[test]
    #[serial]
    fn test_from_env_missing_api_key() {
        setup_clean_env();
        
        let result = LlmConfig::from_env_no_dotenv();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("OPENAI_API_KEY not found"));
    }

    #[test]
    #[serial]
    fn test_from_env_with_api_key() {
        setup_clean_env();
        
        // Set required environment variable
        env::set_var("OPENAI_API_KEY", "test-api-key");
        
        let result = LlmConfig::from_env_no_dotenv();
        assert!(result.is_ok());
        
        let config = result.unwrap();
        assert_eq!(config.api_key, "test-api-key");
        assert_eq!(config.model, "gpt-4o"); // default
        
        setup_clean_env();
    }

    #[test]
    #[serial]
    fn test_from_env_with_overrides() {
        setup_clean_env();
        
        // Set environment variables
        env::set_var("OPENAI_API_KEY", "test-key");
        env::set_var("OPENAI_MODEL", "gpt-3.5-turbo");
        env::set_var("OPENAI_API_BASE", "https://custom.api.com");
        env::set_var("LLM_REQUEST_TIMEOUT", "120");
        env::set_var("LLM_MAX_TOKENS", "2000");
        env::set_var("LLM_TEMPERATURE", "0.7");
        env::set_var("LLM_DEBUG", "true");
        
        let result = LlmConfig::from_env_no_dotenv();
        assert!(result.is_ok());
        
        let config = result.unwrap();
        assert_eq!(config.api_key, "test-key");
        assert_eq!(config.model, "gpt-3.5-turbo");
        assert_eq!(config.api_base, Some("https://custom.api.com".to_string()));
        assert_eq!(config.timeout_secs, 120);
        assert_eq!(config.max_tokens, 2000);
        assert_eq!(config.temperature, 0.7);
        assert!(config.debug);
        
        setup_clean_env();
    }

    #[test]
    #[serial]
    fn test_from_env_invalid_numeric_values() {
        setup_clean_env();
        
        env::set_var("OPENAI_API_KEY", "test-key");
        env::set_var("LLM_REQUEST_TIMEOUT", "invalid");
        env::set_var("LLM_MAX_TOKENS", "not-a-number");
        env::set_var("LLM_TEMPERATURE", "invalid-temp");
        
        let result = LlmConfig::from_env_no_dotenv();
        assert!(result.is_ok());
        
        let config = result.unwrap();
        // Should fall back to defaults for invalid values
        assert_eq!(config.timeout_secs, 300);
        assert_eq!(config.max_tokens, 8000);
        assert_eq!(config.temperature, 0.3);
        
        setup_clean_env();
    }

    #[test]
    #[serial]
    fn test_debug_flag_parsing() {
        setup_clean_env();
        env::set_var("OPENAI_API_KEY", "test-key");
        
        // Test "true"
        env::remove_var("LLM_DEBUG");
        env::set_var("LLM_DEBUG", "true");
        let config = LlmConfig::from_env_no_dotenv().unwrap();
        assert!(config.debug);
        
        // Test "1"
        env::remove_var("LLM_DEBUG");
        env::set_var("LLM_DEBUG", "1");
        let config = LlmConfig::from_env_no_dotenv().unwrap();
        assert!(config.debug);
        
        // Test "false"
        env::remove_var("LLM_DEBUG");
        env::set_var("LLM_DEBUG", "false");
        let config = LlmConfig::from_env_no_dotenv().unwrap();
        assert!(!config.debug);
        
        // Test "0"
        env::remove_var("LLM_DEBUG");
        env::set_var("LLM_DEBUG", "0");
        let config = LlmConfig::from_env_no_dotenv().unwrap();
        assert!(!config.debug);
        
        setup_clean_env();
    }

    #[test]
    #[serial]
    fn test_temperature_bounds() {
        setup_clean_env();
        env::set_var("OPENAI_API_KEY", "test-key");
        
        // Test out of bounds temperature (should use default)
        env::set_var("LLM_TEMPERATURE", "1.5");
        let config = LlmConfig::from_env_no_dotenv().unwrap();
        assert_eq!(config.temperature, 0.3); // Should use default
        
        // Test negative temperature (should use default)
        env::remove_var("LLM_TEMPERATURE");
        env::set_var("LLM_TEMPERATURE", "-0.1");
        let config = LlmConfig::from_env_no_dotenv().unwrap();
        assert_eq!(config.temperature, 0.3); // Should use default
        
        // Test valid temperature
        env::remove_var("LLM_TEMPERATURE");
        env::set_var("LLM_TEMPERATURE", "0.8");
        let config = LlmConfig::from_env_no_dotenv().unwrap();
        assert_eq!(config.temperature, 0.8);
        
        setup_clean_env();
    }
}
