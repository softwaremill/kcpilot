use anyhow::{Result};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::debug;

use super::types::SystemdServiceInfo;

/// Parser for systemd service configuration
pub struct SystemdParser;

impl SystemdParser {
    /// Get systemd service name from PID
    pub async fn get_service_from_pid<F>(executor: F, pid: u32) -> Result<SystemdServiceInfo>
    where
        F: Fn(&str) -> Result<String>,
    {
        debug!("Getting systemd service for PID {}...", pid);
        
        // Use systemctl status PID to get service name
        let status_output = executor(&format!("systemctl status {} 2>/dev/null", pid))?;
        
        if status_output.trim().is_empty() {
            return Err(anyhow::anyhow!("Process {} is not managed by systemd", pid));
        }

        // Extract service name from the first line of systemctl status output
        let service_name = Self::extract_service_name(&status_output)?;
        
        Ok(SystemdServiceInfo {
            service_name,
            service_content: String::new(), // Will be filled in parse_service_config
            environment_file: None,
            environment_vars: HashMap::new(),
            exec_start: None,
        })
    }

    /// Parse systemctl cat output to get full service configuration
    pub async fn parse_service_config<F>(
        executor: F, 
        service_name: &str
    ) -> Result<SystemdServiceInfo>
    where
        F: Fn(&str) -> Result<String>,
    {
        debug!("Parsing systemd configuration for {}...", service_name);
        
        // Get full service configuration
        let service_content = executor(&format!("systemctl cat {} 2>/dev/null", service_name))?;
        
        if service_content.trim().is_empty() {
            return Err(anyhow::anyhow!("Could not get service configuration for {}", service_name));
        }
        
        // Parse environment variables and files from service content
        let (environment_file, mut environment_vars, exec_start) = Self::parse_service_content(&service_content);
        
        // If there's an environment file, read it
        if let Some(ref env_file_path) = environment_file {
            if let Ok(env_file_content) = executor(&format!("sudo cat '{}' 2>/dev/null", env_file_path.display())) {
                let env_file_vars = Self::parse_environment_file(&env_file_content);
                environment_vars.extend(env_file_vars);
            }
        }

        Ok(SystemdServiceInfo {
            service_name: service_name.to_string(),
            service_content,
            environment_file,
            environment_vars,
            exec_start,
        })
    }

    /// Extract service name from systemctl status output
    fn extract_service_name(status_output: &str) -> Result<String> {
        // First line typically looks like: "● kafka.service - Apache Kafka Server"
        let first_line = status_output.lines().next()
            .ok_or_else(|| anyhow::anyhow!("Empty systemctl status output"))?;
        
        // Extract service name (between ● and the dash)
        if let Some(service_start) = first_line.find(' ') {
            let after_bullet = &first_line[service_start + 1..];
            if let Some(dash_pos) = after_bullet.find(" - ") {
                return Ok(after_bullet[..dash_pos].trim().to_string());
            }
        }
        
        Err(anyhow::anyhow!("Could not parse service name from status output"))
    }

    /// Parse systemd service content for environment info
    fn parse_service_content(content: &str) -> (Option<PathBuf>, HashMap<String, String>, Option<String>) {
        let mut environment_file = None;
        let mut environment_vars = HashMap::new();
        let mut exec_start = None;
        
        for line in content.lines() {
            let line = line.trim();
            
            if line.starts_with("EnvironmentFile=") {
                let path = line[16..].trim_matches('"');
                environment_file = Some(PathBuf::from(path));
            } else if line.starts_with("Environment=") {
                let env_part = &line[12..];
                if let Some(eq_pos) = env_part.find('=') {
                    let key = env_part[..eq_pos].trim();
                    let value = env_part[eq_pos + 1..].trim_matches('"');
                    environment_vars.insert(key.to_string(), value.to_string());
                }
            } else if line.starts_with("ExecStart=") {
                exec_start = Some(line[10..].to_string());
            }
        }
        
        (environment_file, environment_vars, exec_start)
    }

    /// Parse environment file content
    fn parse_environment_file(content: &str) -> HashMap<String, String> {
        let mut vars = HashMap::new();
        
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            
            if let Some(eq_pos) = line.find('=') {
                let key = line[..eq_pos].trim();
                let value = line[eq_pos + 1..].trim_matches('"').trim_matches('\'');
                vars.insert(key.to_string(), value.to_string());
            }
        }
        
        vars
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_service_name_success() {
        let status_output = "● kafka.service - Apache Kafka Server\n   Loaded: loaded (/etc/systemd/system/kafka.service; enabled; vendor preset: enabled)";
        let result = SystemdParser::extract_service_name(status_output).unwrap();
        assert_eq!(result, "kafka.service");
    }

    #[test]
    fn test_extract_service_name_different_format() {
        let status_output = "● confluent-kafka.service - Confluent Kafka\n   Loaded: loaded";
        let result = SystemdParser::extract_service_name(status_output).unwrap();
        assert_eq!(result, "confluent-kafka.service");
    }

    #[test]
    fn test_extract_service_name_empty() {
        let result = SystemdParser::extract_service_name("");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Empty systemctl status output"));
    }

    #[test]
    fn test_extract_service_name_no_dash() {
        let status_output = "● kafka.service no dash here";
        let result = SystemdParser::extract_service_name(status_output);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Could not parse service name"));
    }

    #[test]
    fn test_parse_service_content() {
        let content = r#"
[Unit]
Description=Apache Kafka
After=network.target

[Service]
Type=forking
User=kafka
Group=kafka
EnvironmentFile=/etc/kafka/kafka.env
Environment=KAFKA_HEAP_OPTS="-Xmx1G -Xms1G"
Environment=KAFKA_LOG_DIR="/var/log/kafka"
ExecStart=/opt/kafka/bin/kafka-server-start.sh /opt/kafka/config/server.properties
ExecStop=/opt/kafka/bin/kafka-server-stop.sh

[Install]
WantedBy=multi-user.target
"#;

        let (env_file, env_vars, exec_start) = SystemdParser::parse_service_content(content);

        assert_eq!(env_file, Some(PathBuf::from("/etc/kafka/kafka.env")));
        assert_eq!(env_vars.get("KAFKA_HEAP_OPTS"), Some(&"-Xmx1G -Xms1G".to_string()));
        assert_eq!(env_vars.get("KAFKA_LOG_DIR"), Some(&"/var/log/kafka".to_string()));
        assert_eq!(exec_start, Some("/opt/kafka/bin/kafka-server-start.sh /opt/kafka/config/server.properties".to_string()));
    }

    #[test]
    fn test_parse_service_content_minimal() {
        let content = "[Service]\nExecStart=/usr/bin/kafka";
        let (env_file, env_vars, exec_start) = SystemdParser::parse_service_content(content);

        assert_eq!(env_file, None);
        assert!(env_vars.is_empty());
        assert_eq!(exec_start, Some("/usr/bin/kafka".to_string()));
    }

    #[test]
    fn test_parse_environment_file() {
        let content = r#"
# Kafka environment variables
KAFKA_HOME=/opt/kafka
KAFKA_LOG_DIR="/var/log/kafka"
KAFKA_USER='kafka'
JAVA_HOME=/usr/lib/jvm/java-11

# Comment line
EMPTY_VALUE=
"#;

        let vars = SystemdParser::parse_environment_file(content);

        assert_eq!(vars.get("KAFKA_HOME"), Some(&"/opt/kafka".to_string()));
        assert_eq!(vars.get("KAFKA_LOG_DIR"), Some(&"/var/log/kafka".to_string()));
        assert_eq!(vars.get("KAFKA_USER"), Some(&"kafka".to_string()));
        assert_eq!(vars.get("JAVA_HOME"), Some(&"/usr/lib/jvm/java-11".to_string()));
        assert_eq!(vars.get("EMPTY_VALUE"), Some(&"".to_string()));
        assert!(!vars.contains_key("# Comment line"));
    }

    #[test]
    fn test_parse_environment_file_empty() {
        let vars = SystemdParser::parse_environment_file("");
        assert!(vars.is_empty());
    }

    #[test]
    fn test_parse_environment_file_comments_only() {
        let content = "# Just comments\n# Another comment\n";
        let vars = SystemdParser::parse_environment_file(content);
        assert!(vars.is_empty());
    }
}