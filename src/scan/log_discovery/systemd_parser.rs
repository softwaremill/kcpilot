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