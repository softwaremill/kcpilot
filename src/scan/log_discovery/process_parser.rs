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
            // Skip JVM parameters (starting with -D)
            if part.starts_with("-D") {
                continue;
            }
            
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_process_info_success() {
        let ps_output = "kafka     1234  5.0  2.0 8765432 123456 ?     Sl   10:30   1:23 java -Xms1G -Xmx1G -Dlog4j.configuration=/opt/kafka/config/log4j.properties kafka.Kafka /opt/kafka/config/server.properties";
        
        let result = ProcessParser::parse_process_info(ps_output).unwrap();
        
        assert_eq!(result.pid, 1234);
        assert!(result.command_line.contains("kafka.Kafka"));
        assert_eq!(result.config_paths.len(), 1); // Only server.properties is extracted as config path
        assert!(result.config_paths.contains(&PathBuf::from("/opt/kafka/config/server.properties")));
        // log4j.properties is in -D flag, not extracted as config path
        assert_eq!(result.log4j_path, Some(PathBuf::from("/opt/kafka/config/log4j.properties")));
    }

    #[test]
    fn test_parse_process_info_empty() {
        let result = ProcessParser::parse_process_info("");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Empty ps output"));
    }

    #[test]
    fn test_parse_process_info_invalid_format() {
        let ps_output = "kafka 1234 short";
        let result = ProcessParser::parse_process_info(ps_output);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid ps output format"));
    }

    #[test]
    fn test_parse_process_info_invalid_pid() {
        let ps_output = "kafka abc  5.0  2.0 8765432 123456 ?     Sl   10:30   1:23 java kafka.Kafka";
        let result = ProcessParser::parse_process_info(ps_output);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Failed to parse PID"));
    }

    #[test]
    fn test_extract_config_paths() {
        let cmdline = "java -jar kafka.jar /opt/kafka/server.properties /etc/kafka/log4j.conf other-file.txt";
        let paths = ProcessParser::extract_config_paths(cmdline);
        
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], PathBuf::from("/opt/kafka/server.properties"));
        assert_eq!(paths[1], PathBuf::from("/etc/kafka/log4j.conf"));
    }

    #[test]
    fn test_extract_config_paths_empty() {
        let cmdline = "java -jar kafka.jar other-file.txt";
        let paths = ProcessParser::extract_config_paths(cmdline);
        assert!(paths.is_empty());
    }

    #[test]
    fn test_extract_log4j_path_log4j_configuration() {
        let cmdline = "java -Dlog4j.configuration=file:/opt/kafka/log4j.properties kafka.Kafka";
        let path = ProcessParser::extract_log4j_path(cmdline);
        assert_eq!(path, Some(PathBuf::from("/opt/kafka/log4j.properties")));
    }

    #[test]
    fn test_extract_log4j_path_log4j2_configuration() {
        let cmdline = "java -Dlog4j2.configurationFile=/opt/kafka/log4j2.xml kafka.Kafka";
        let path = ProcessParser::extract_log4j_path(cmdline);
        assert_eq!(path, Some(PathBuf::from("/opt/kafka/log4j2.xml")));
    }

    #[test]
    fn test_extract_log4j_path_none() {
        let cmdline = "java kafka.Kafka /opt/kafka/server.properties";
        let path = ProcessParser::extract_log4j_path(cmdline);
        assert_eq!(path, None);
    }

    #[test]
    fn test_extract_log4j_path_file_prefix() {
        let cmdline = "java -Dlog4j.configuration=file:/var/log/kafka/log4j.properties kafka.Kafka";
        let path = ProcessParser::extract_log4j_path(cmdline);
        assert_eq!(path, Some(PathBuf::from("/var/log/kafka/log4j.properties")));
    }
}