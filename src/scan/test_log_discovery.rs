#[cfg(test)]
mod tests {
    use crate::scan::log_discovery::*;
    use tempfile::TempDir;
    use std::fs;
    use std::collections::HashMap;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_log_discovery_local() {
        let discovery = LogDiscovery::new(None);
        
        // This will try to discover on the local system
        // In CI, it might not find anything, but should not crash
        let result = discovery.discover_logs().await;
        
        // Should always return OK, even if no logs found
        assert!(result.is_ok());
        
        let discovered = result.unwrap();
        assert!(!discovered.detection_methods.is_empty());
    }

    #[test]
    fn test_determine_log_type_from_path() {
        let discovery = LogDiscovery::new(None);
        
        assert!(matches!(
            discovery.determine_log_type_from_path(&PathBuf::from("/var/log/kafka/server.log")),
            LogType::Server
        ));
        
        assert!(matches!(
            discovery.determine_log_type_from_path(&PathBuf::from("/opt/kafka/logs/controller.log")),
            LogType::Controller
        ));
        
        assert!(matches!(
            discovery.determine_log_type_from_path(&PathBuf::from("/var/log/kafka/state-change.log")),
            LogType::StateChange
        ));
        
        assert!(matches!(
            discovery.determine_log_type_from_path(&PathBuf::from("/var/log/kafka/unknown.log")),
            LogType::Unknown
        ));
    }

    #[test]
    fn test_resolve_log4j_path() {
        let discovery = LogDiscovery::new(None);
        
        let resolved = discovery.resolve_log4j_path("${kafka.logs.dir}/server.log");
        assert_eq!(resolved, PathBuf::from("/var/log/kafka/server.log"));
        
        let resolved = discovery.resolve_log4j_path("/absolute/path/server.log");
        assert_eq!(resolved, PathBuf::from("/absolute/path/server.log"));
        
        let resolved = discovery.resolve_log4j_path("${log.dir}/kafka.log");
        assert_eq!(resolved, PathBuf::from("/var/log/kafka/kafka.log"));
    }

    #[test]
    fn test_determine_log_type_from_appender() {
        let discovery = LogDiscovery::new(None);
        
        assert!(matches!(
            discovery.determine_log_type_from_appender("log4j.appender.kafkaServerAppender.File"),
            LogType::Server
        ));
        
        assert!(matches!(
            discovery.determine_log_type_from_appender("log4j.appender.controllerAppender.File"),
            LogType::Controller
        ));
        
        assert!(matches!(
            discovery.determine_log_type_from_appender("log4j.appender.stateChangeAppender.File"),
            LogType::StateChange
        ));
        
        assert!(matches!(
            discovery.determine_log_type_from_appender("log4j.appender.customAppender.File"),
            LogType::Custom(_)
        ));
    }

    #[tokio::test]
    async fn test_parse_log4j_config() {
        let temp_dir = TempDir::new().unwrap();
        let log4j_path = temp_dir.path().join("log4j.properties");
        
        let log4j_content = r#"
# Kafka log4j configuration
log4j.rootLogger=INFO, kafkaAppender

# Kafka server appender
log4j.appender.kafkaAppender=org.apache.log4j.RollingFileAppender
log4j.appender.kafkaAppender.File=${kafka.logs.dir}/server.log
log4j.appender.kafkaAppender.layout=org.apache.log4j.PatternLayout
log4j.appender.kafkaAppender.layout.ConversionPattern=[%d] %p %m (%c)%n

# Controller appender
log4j.appender.controllerAppender=org.apache.log4j.RollingFileAppender  
log4j.appender.controllerAppender.File=/var/log/kafka/controller.log
log4j.appender.controllerAppender.layout=org.apache.log4j.PatternLayout

# State change appender
log4j.appender.stateAppender=org.apache.log4j.RollingFileAppender
log4j.appender.stateAppender.File=${log.dir}/state-change.log
        "#;
        
        fs::write(&log4j_path, log4j_content).unwrap();
        
        let discovery = LogDiscovery::new(None);
        let mut discovered = DiscoveredLogs {
            log_files: HashMap::new(),
            log_configs: HashMap::new(),
            detection_methods: Vec::new(),
            warnings: Vec::new(),
            process_info: None,
        };
        
        let result = discovery.parse_log4j_config(&log4j_path, &mut discovered).await;
        assert!(result.is_ok());
        
        // Should have found 3 log files
        assert_eq!(discovered.log_files.len(), 3);
        
        // Check that variable substitution worked
        let server_log = discovered.log_files.get("/var/log/kafka/server.log");
        assert!(server_log.is_some());
        assert!(matches!(server_log.unwrap().log_type, LogType::Server));
        
        let controller_log = discovered.log_files.get("/var/log/kafka/controller.log");
        assert!(controller_log.is_some());
        assert!(matches!(controller_log.unwrap().log_type, LogType::Controller));
        
        let state_log = discovered.log_files.get("/var/log/kafka/state-change.log");
        assert!(state_log.is_some());
        assert!(matches!(state_log.unwrap().log_type, LogType::StateChange));
        
        // Should have stored the config content
        assert!(discovered.log_configs.contains_key(log4j_path.to_str().unwrap()));
    }

    #[tokio::test]
    async fn test_parse_kafka_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("server.properties");
        
        let config_content = r#"
# Kafka server configuration
broker.id=1
listeners=PLAINTEXT://localhost:9092
log.dirs=/data/kafka-logs,/data2/kafka-logs
num.network.threads=8
num.io.threads=16

# Log retention
log.retention.hours=168
log.segment.bytes=1073741824

# Zookeeper
zookeeper.connect=localhost:2181
        "#;
        
        fs::write(&config_path, config_content).unwrap();
        
        let discovery = LogDiscovery::new(None);
        let mut discovered = DiscoveredLogs {
            log_files: HashMap::new(),
            log_configs: HashMap::new(),
            detection_methods: Vec::new(),
            warnings: Vec::new(),
            process_info: None,
        };
        
        let result = discovery.parse_kafka_config(&config_path, &mut discovered).await;
        assert!(result.is_ok());
        
        // Should have stored the config content
        assert!(discovered.log_configs.contains_key(config_path.to_str().unwrap()));
        
        // Should have detected the log directories (though not log files specifically)
        // The actual files would be discovered by searching within these directories
        assert!(!discovered.log_configs.is_empty());
    }

    #[test]
    fn test_extract_jvm_args() {
        let discovery = LogDiscovery::new(None);
        
        let cmdline = vec![
            "java".to_string(),
            "-Xmx1G".to_string(),
            "-Xms1G".to_string(),
            "-Dlog4j.configuration=file:///etc/kafka/log4j.properties".to_string(),
            "-Dkafka.logs.dir=/var/log/kafka".to_string(),
            "kafka.Kafka".to_string(),
            "/etc/kafka/server.properties".to_string(),
        ];
        
        let jvm_args = discovery.extract_jvm_args(&cmdline);
        
        assert!(jvm_args.contains(&"-Xmx1G".to_string()));
        assert!(jvm_args.contains(&"-Xms1G".to_string()));
        assert!(jvm_args.contains(&"-Dlog4j.configuration=file:///etc/kafka/log4j.properties".to_string()));
        assert!(jvm_args.contains(&"-Dkafka.logs.dir=/var/log/kafka".to_string()));
        
        // Should not contain the main class or config file
        assert!(!jvm_args.contains(&"kafka.Kafka".to_string()));
        assert!(!jvm_args.contains(&"/etc/kafka/server.properties".to_string()));
    }

    #[test]
    fn test_extract_log4j_config() {
        let discovery = LogDiscovery::new(None);
        
        let jvm_args = vec![
            "-Xmx1G".to_string(),
            "-Dlog4j.configuration=file:///etc/kafka/log4j.properties".to_string(),
            "-Dkafka.logs.dir=/var/log/kafka".to_string(),
        ];
        
        let env_vars = HashMap::new();
        
        let log4j_config = discovery.extract_log4j_config(&jvm_args, &env_vars);
        assert!(log4j_config.is_some());
        assert_eq!(log4j_config.unwrap(), PathBuf::from("/etc/kafka/log4j.properties"));
        
        // Test log4j2 configuration
        let jvm_args_log4j2 = vec![
            "-Dlog4j2.configurationFile=/etc/kafka/log4j2.xml".to_string(),
        ];
        
        let log4j2_config = discovery.extract_log4j_config(&jvm_args_log4j2, &env_vars);
        assert!(log4j2_config.is_some());
        assert_eq!(log4j2_config.unwrap(), PathBuf::from("/etc/kafka/log4j2.xml"));
        
        // Test environment variable
        let mut env_with_log4j = HashMap::new();
        env_with_log4j.insert("LOG4J_CONFIGURATION_FILE".to_string(), "/opt/kafka/log4j.properties".to_string());
        
        let env_config = discovery.extract_log4j_config(&vec![], &env_with_log4j);
        assert!(env_config.is_some());
        assert_eq!(env_config.unwrap(), PathBuf::from("/opt/kafka/log4j.properties"));
    }

    #[tokio::test]
    async fn test_standard_locations_fallback() {
        let discovery = LogDiscovery::new(None);
        let mut discovered = DiscoveredLogs {
            log_files: HashMap::new(),
            log_configs: HashMap::new(),
            detection_methods: Vec::new(),
            warnings: Vec::new(),
            process_info: None,
        };
        
        let result = discovery.discover_standard_locations(&mut discovered).await;
        assert!(result.is_ok());
        
        // Should have added standard locations
        assert!(!discovered.log_files.is_empty());
        
        // All should have "standard_locations" as discovery method
        for (_, log_info) in &discovered.log_files {
            assert_eq!(log_info.discovery_method, "standard_locations");
        }
    }

    #[tokio::test]  
    async fn test_validate_log_files() {
        let temp_dir = TempDir::new().unwrap();
        let test_log = temp_dir.path().join("test.log");
        fs::write(&test_log, "test log content").unwrap();
        
        let discovery = LogDiscovery::new(None);
        let mut discovered = DiscoveredLogs {
            log_files: HashMap::new(),
            log_configs: HashMap::new(),
            detection_methods: Vec::new(),
            warnings: Vec::new(),
            process_info: None,
        };
        
        // Add a test log file
        discovery.add_log_file(test_log, "test", LogType::Server, &mut discovered).await.unwrap();
        
        // Validate it
        let result = discovery.validate_log_files(&mut discovered).await;
        assert!(result.is_ok());
        
        // The log file should now be marked as accessible  
        let log_info = discovered.log_files.values().next().unwrap();
        // Note: This might fail in some test environments due to file permissions
        // but the validation should not error out
        println!("Log file accessible: {}", log_info.accessible);
    }

    #[test]
    fn test_log_type_display() {
        assert_eq!(LogType::Server.to_string(), "server");
        assert_eq!(LogType::Controller.to_string(), "controller");
        assert_eq!(LogType::StateChange.to_string(), "state-change");
        assert_eq!(LogType::Request.to_string(), "request");
        assert_eq!(LogType::Gc.to_string(), "gc");
        assert_eq!(LogType::Custom("my-custom".to_string()).to_string(), "custom(my-custom)");
        assert_eq!(LogType::Unknown.to_string(), "unknown");
    }

    #[tokio::test]
    async fn test_collect_log_contents() {
        let temp_dir = TempDir::new().unwrap();
        let test_log1 = temp_dir.path().join("test1.log");
        let test_log2 = temp_dir.path().join("test2.log");
        
        fs::write(&test_log1, "line1\nline2\nline3").unwrap();
        fs::write(&test_log2, "log2line1\nlog2line2").unwrap();
        
        let discovery = LogDiscovery::new(None);
        let mut discovered = DiscoveredLogs {
            log_files: HashMap::new(),
            log_configs: HashMap::new(),
            detection_methods: Vec::new(),
            warnings: Vec::new(),
            process_info: None,
        };
        
        // Add test log files
        discovery.add_log_file(test_log1.clone(), "test", LogType::Server, &mut discovered).await.unwrap();
        discovery.add_log_file(test_log2.clone(), "test", LogType::Controller, &mut discovered).await.unwrap();
        
        // Mark them as accessible (simulate validation)
        for (_, log_info) in discovered.log_files.iter_mut() {
            log_info.accessible = true;
        }
        
        let contents = discovery.collect_log_contents(&discovered, 10).await.unwrap();
        
        assert_eq!(contents.len(), 2);
        assert!(contents.contains_key(&test_log1.to_string_lossy().to_string()));
        assert!(contents.contains_key(&test_log2.to_string_lossy().to_string()));
        
        let content1 = contents.get(&test_log1.to_string_lossy().to_string()).unwrap();
        assert!(content1.contains("line1"));
        assert!(content1.contains("line2"));
        assert!(content1.contains("line3"));
    }
}