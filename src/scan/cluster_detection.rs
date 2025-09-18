use std::collections::HashMap;
use tracing::{debug, info};
use serde_json;

/// Detect the cluster mode (Zookeeper vs KRaft) from configuration data
pub fn detect_cluster_mode(config_data: &serde_json::Value) -> crate::snapshot::format::ClusterMode {
    use crate::snapshot::format::ClusterMode;
    
    debug!("Starting cluster mode detection from configuration data");
    
    // Extract all server.properties content from the config data
    let server_properties_configs = extract_server_properties_configs(config_data);
    
    if server_properties_configs.is_empty() {
        debug!("No server.properties files found in configuration data");
        return ClusterMode::Unknown;
    }
    
    debug!("Found {} server.properties files to analyze", server_properties_configs.len());
    
    // Analyze each server.properties file
    let mut zookeeper_count = 0;
    let mut kraft_count = 0;
    let mut unknown_count = 0;
    
    for (file_path, content) in &server_properties_configs {
        debug!("Analyzing server.properties from: {}", file_path);
        let properties = parse_server_properties(content);
        
        if is_zookeeper_mode(&properties) {
            debug!("Detected Zookeeper mode in: {}", file_path);
            zookeeper_count += 1;
        } else if is_kraft_mode(&properties) {
            debug!("Detected KRaft mode in: {}", file_path);
            kraft_count += 1;
        } else {
            debug!("Could not determine mode from: {}", file_path);
            unknown_count += 1;
        }
    }
    
    // Determine overall cluster mode based on majority
    info!("Cluster mode analysis: {} ZK, {} KRaft, {} unknown", 
          zookeeper_count, kraft_count, unknown_count);
    
    match (zookeeper_count, kraft_count) {
        (zk, 0) if zk > 0 => {
            info!("Detected Zookeeper cluster mode");
            ClusterMode::Zookeeper
        }
        (0, kraft) if kraft > 0 => {
            info!("Detected KRaft cluster mode");
            ClusterMode::Kraft
        }
        (zk, kraft) if zk > 0 && kraft > 0 => {
            debug!("Mixed cluster modes detected! ZK: {}, KRaft: {}. Setting to Unknown.", zk, kraft);
            ClusterMode::Unknown
        }
        _ => {
            debug!("Could not determine cluster mode from configuration");
            ClusterMode::Unknown
        }
    }
}

/// Extract server.properties content from configuration data
fn extract_server_properties_configs(config_data: &serde_json::Value) -> Vec<(String, String)> {
    let mut configs = Vec::new();
    
    // Handle different possible structures in config data
    if let Some(config_obj) = config_data.as_object() {
        for (file_path, content) in config_obj {
            if file_path.contains("server.properties") {
                if let Some(content_str) = content.as_str() {
                    configs.push((file_path.clone(), content_str.to_string()));
                }
            }
        }
    }
    
    // Also check nested broker structures
    if let Some(brokers_data) = config_data.get("brokers") {
        if let Some(brokers_obj) = brokers_data.as_object() {
            for (broker_name, broker_data) in brokers_obj {
                if let Some(broker_obj) = broker_data.as_object() {
                    if let Some(configs_data) = broker_obj.get("configs") {
                        if let Some(configs_obj) = configs_data.as_object() {
                            for (config_name, config_content) in configs_obj {
                                if config_name == "server.properties" {
                                    if let Some(content_str) = config_content.as_str() {
                                        let file_path = format!("{}/configs/{}", broker_name, config_name);
                                        configs.push((file_path, content_str.to_string()));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    configs
}

/// Parse server.properties file format into key-value pairs
pub fn parse_server_properties(content: &str) -> HashMap<String, String> {
    let mut properties = HashMap::new();
    
    for line in content.lines() {
        let line = line.trim();
        
        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        
        // Parse key=value pairs
        if let Some(eq_pos) = line.find('=') {
            let key = line[..eq_pos].trim().to_string();
            let value = line[eq_pos + 1..].trim().to_string();
            
            if !key.is_empty() {
                properties.insert(key, value);
            }
        }
    }
    
    debug!("Parsed {} properties from server.properties", properties.len());
    properties
}

/// Check if configuration indicates Zookeeper mode
pub fn is_zookeeper_mode(properties: &HashMap<String, String>) -> bool {
    // ZK detection rules:
    // 1. zookeeper.connect is defined and has at least one value
    // 2. process.roles=controller does not exist or is not set  
    // 3. broker.id exists
    // 4. node.id does not exist
    // 5. controller.quorum.voters does not exist
    
    // Check zookeeper.connect
    let has_zk_connect = if let Some(zk_connect) = properties.get("zookeeper.connect") {
        !zk_connect.is_empty() && !zk_connect.trim().is_empty()
    } else {
        false
    };
    
    // Check process.roles does not contain controller
    let has_controller_role = if let Some(process_roles) = properties.get("process.roles") {
        process_roles.contains("controller")
    } else {
        false
    };
    
    // Check broker.id exists
    let has_broker_id = properties.contains_key("broker.id");
    
    // Check node.id does not exist
    let has_node_id = properties.contains_key("node.id");
    
    // Check controller.quorum.voters does not exist
    let has_quorum_voters = properties.contains_key("controller.quorum.voters");
    
    debug!("ZK mode check - zk_connect: {}, controller_role: {}, broker_id: {}, node_id: {}, quorum_voters: {}",
           has_zk_connect, has_controller_role, has_broker_id, has_node_id, has_quorum_voters);
    
    has_zk_connect && !has_controller_role && has_broker_id && !has_node_id && !has_quorum_voters
}

/// Check if configuration indicates KRaft mode
pub fn is_kraft_mode(properties: &HashMap<String, String>) -> bool {
    // KRaft detection rules:
    // 1. zookeeper.connect does not exist
    // 2. process.roles contains "controller" 
    // 3. node.id exists (broker.id can coexist for compatibility)
    // 4. controller.quorum.voters exists
    
    // Check zookeeper.connect does not exist
    let has_zk_connect = properties.contains_key("zookeeper.connect");
    
    // Check process.roles contains controller
    let has_controller_role = if let Some(process_roles) = properties.get("process.roles") {
        process_roles.contains("controller")
    } else {
        false
    };
    
    // Check node.id exists (broker.id can also exist for compatibility)
    let has_node_id = properties.contains_key("node.id");
    
    // Check controller.quorum.voters exists
    let has_quorum_voters = properties.contains_key("controller.quorum.voters");
    
    debug!("KRaft mode check - zk_connect: {}, controller_role: {}, node_id: {}, quorum_voters: {}",
           has_zk_connect, has_controller_role, has_node_id, has_quorum_voters);
    
    !has_zk_connect && has_controller_role && has_node_id && has_quorum_voters
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use serde_json::json;

    #[test]
    fn test_parse_server_properties() {
        let content = r#"
# This is a comment
broker.id=11
zookeeper.connect=zk1:2181,zk2:2181,zk3:2181

# Another comment
log.dirs=/var/kafka/logs
num.network.threads=3

# Empty line above and below should be ignored

process.roles=controller,broker
"#;
        
        let properties = parse_server_properties(content);
        
        assert_eq!(properties.get("broker.id"), Some(&"11".to_string()));
        assert_eq!(properties.get("zookeeper.connect"), Some(&"zk1:2181,zk2:2181,zk3:2181".to_string()));
        assert_eq!(properties.get("log.dirs"), Some(&"/var/kafka/logs".to_string()));
        assert_eq!(properties.get("num.network.threads"), Some(&"3".to_string()));
        assert_eq!(properties.get("process.roles"), Some(&"controller,broker".to_string()));
        assert_eq!(properties.len(), 5);
    }

    #[test]
    fn test_parse_server_properties_with_spaces() {
        let content = r#"
broker.id = 11
zookeeper.connect = zk1:2181
log.dirs=/var/kafka/logs
"#;
        
        let properties = parse_server_properties(content);
        
        assert_eq!(properties.get("broker.id"), Some(&"11".to_string()));
        assert_eq!(properties.get("zookeeper.connect"), Some(&"zk1:2181".to_string()));
        assert_eq!(properties.get("log.dirs"), Some(&"/var/kafka/logs".to_string()));
    }

    #[test]
    fn test_is_zookeeper_mode() {
        let mut properties = HashMap::new();
        properties.insert("zookeeper.connect".to_string(), "zk1:2181,zk2:2181".to_string());
        properties.insert("broker.id".to_string(), "11".to_string());
        
        assert!(is_zookeeper_mode(&properties));
    }

    #[test]
    fn test_is_zookeeper_mode_false_with_empty_zk_connect() {
        let mut properties = HashMap::new();
        properties.insert("zookeeper.connect".to_string(), "".to_string()); // Empty value
        properties.insert("broker.id".to_string(), "11".to_string());
        
        assert!(!is_zookeeper_mode(&properties));
    }

    #[test]
    fn test_is_zookeeper_mode_false_with_controller_role() {
        let mut properties = HashMap::new();
        properties.insert("zookeeper.connect".to_string(), "zk1:2181".to_string());
        properties.insert("broker.id".to_string(), "11".to_string());
        properties.insert("process.roles".to_string(), "controller".to_string());
        
        assert!(!is_zookeeper_mode(&properties));
    }

    #[test]
    fn test_is_zookeeper_mode_false_with_node_id() {
        let mut properties = HashMap::new();
        properties.insert("zookeeper.connect".to_string(), "zk1:2181".to_string());
        properties.insert("broker.id".to_string(), "11".to_string());
        properties.insert("node.id".to_string(), "1".to_string());
        
        assert!(!is_zookeeper_mode(&properties));
    }

    #[test]
    fn test_is_zookeeper_mode_false_with_quorum_voters() {
        let mut properties = HashMap::new();
        properties.insert("zookeeper.connect".to_string(), "zk1:2181".to_string());
        properties.insert("broker.id".to_string(), "11".to_string());
        properties.insert("controller.quorum.voters".to_string(), "1@kafka1:9093".to_string());
        
        assert!(!is_zookeeper_mode(&properties));
    }

    #[test]
    fn test_is_kraft_mode() {
        let mut properties = HashMap::new();
        properties.insert("process.roles".to_string(), "controller,broker".to_string());
        properties.insert("node.id".to_string(), "1".to_string());
        properties.insert("controller.quorum.voters".to_string(), "1@kafka1:9093".to_string());
        
        assert!(is_kraft_mode(&properties));
    }

    #[test]
    fn test_is_kraft_mode_with_only_controller_role() {
        let mut properties = HashMap::new();
        properties.insert("process.roles".to_string(), "controller".to_string());
        properties.insert("node.id".to_string(), "1".to_string());
        properties.insert("controller.quorum.voters".to_string(), "1@kafka1:9093".to_string());
        
        assert!(is_kraft_mode(&properties));
    }

    #[test]
    fn test_is_kraft_mode_false_with_zk_connect() {
        let mut properties = HashMap::new();
        properties.insert("zookeeper.connect".to_string(), "zk1:2181".to_string());
        properties.insert("process.roles".to_string(), "controller".to_string());
        properties.insert("node.id".to_string(), "1".to_string());
        properties.insert("controller.quorum.voters".to_string(), "1@kafka1:9093".to_string());
        
        assert!(!is_kraft_mode(&properties));
    }

    #[test]
    fn test_is_kraft_mode_with_broker_id_compatibility() {
        let mut properties = HashMap::new();
        properties.insert("process.roles".to_string(), "controller".to_string());
        properties.insert("broker.id".to_string(), "1".to_string()); // Can coexist in KRaft
        properties.insert("node.id".to_string(), "1".to_string());
        properties.insert("controller.quorum.voters".to_string(), "1@kafka1:9093".to_string());
        
        assert!(is_kraft_mode(&properties));
    }

    #[test]
    fn test_is_kraft_mode_false_without_node_id() {
        let mut properties = HashMap::new();
        properties.insert("process.roles".to_string(), "controller".to_string());
        properties.insert("controller.quorum.voters".to_string(), "1@kafka1:9093".to_string());
        
        assert!(!is_kraft_mode(&properties));
    }

    #[test]
    fn test_detect_cluster_mode_zookeeper() {
        let config_data = json!({
            "broker_1/server.properties": "broker.id=11\nzookeeper.connect=zk1:2181,zk2:2181\nlog.dirs=/var/kafka/logs",
            "broker_2/server.properties": "broker.id=12\nzookeeper.connect=zk1:2181,zk2:2181\nlog.dirs=/var/kafka/logs"
        });

        let mode = detect_cluster_mode(&config_data);
        assert!(matches!(mode, crate::snapshot::format::ClusterMode::Zookeeper));
    }

    #[test]
    fn test_detect_cluster_mode_kraft() {
        let config_data = json!({
            "broker_1/server.properties": "node.id=1\nprocess.roles=controller,broker\ncontroller.quorum.voters=1@kafka1:9093",
            "broker_2/server.properties": "node.id=2\nprocess.roles=controller,broker\ncontroller.quorum.voters=1@kafka1:9093"
        });

        let mode = detect_cluster_mode(&config_data);
        assert!(matches!(mode, crate::snapshot::format::ClusterMode::Kraft));
    }

    #[test]
    fn test_detect_cluster_mode_unknown() {
        let config_data = json!({
            "broker_1/server.properties": "some.other.config=value\nlog.dirs=/var/kafka/logs",
        });

        let mode = detect_cluster_mode(&config_data);
        assert!(matches!(mode, crate::snapshot::format::ClusterMode::Unknown));
    }

    #[test]
    fn test_detect_cluster_mode_mixed() {
        let config_data = json!({
            "broker_1/server.properties": "broker.id=11\nzookeeper.connect=zk1:2181",
            "broker_2/server.properties": "node.id=2\nprocess.roles=controller\ncontroller.quorum.voters=1@kafka1:9093"
        });

        let mode = detect_cluster_mode(&config_data);
        assert!(matches!(mode, crate::snapshot::format::ClusterMode::Unknown));
    }

    #[test]
    fn test_detect_cluster_mode_no_config() {
        let config_data = json!({
            "some_other_file.txt": "not a server.properties file"
        });

        let mode = detect_cluster_mode(&config_data);
        assert!(matches!(mode, crate::snapshot::format::ClusterMode::Unknown));
    }

    #[test]
    fn test_extract_server_properties_configs() {
        let config_data = json!({
            "broker_1/server.properties": "broker.id=11",
            "broker_1/log4j.properties": "log4j.rootLogger=INFO",
            "broker_2/server.properties": "broker.id=12",
        });

        let configs = extract_server_properties_configs(&config_data);
        assert_eq!(configs.len(), 2);
        assert!(configs.iter().any(|(path, _)| path == "broker_1/server.properties"));
        assert!(configs.iter().any(|(path, _)| path == "broker_2/server.properties"));
        assert!(!configs.iter().any(|(path, _)| path.contains("log4j")));
    }

    #[test]
    fn test_extract_server_properties_configs_nested_structure() {
        let config_data = json!({
            "brokers": {
                "broker_1": {
                    "configs": {
                        "server.properties": "broker.id=11\nzookeeper.connect=zk1:2181"
                    }
                },
                "broker_2": {
                    "configs": {
                        "server.properties": "broker.id=12\nzookeeper.connect=zk1:2181"
                    }
                }
            }
        });

        let configs = extract_server_properties_configs(&config_data);
        assert_eq!(configs.len(), 2);
        assert!(configs.iter().any(|(path, content)| 
            path == "broker_1/configs/server.properties" && content.contains("broker.id=11")));
        assert!(configs.iter().any(|(path, content)| 
            path == "broker_2/configs/server.properties" && content.contains("broker.id=12")));
    }

    #[test]
    fn test_detect_kraft_with_real_config() {
        let kraft_config = r#"# ansible managed

# kraft control plane
node.id=13
controller.listener.names=CONTROLLER
process.roles=broker,controller
controller.quorum.voters=11@kafka-poligon-dc1-1.c.bartek-rekke-sandbox.internal:9094,13@kafka-poligon-dc2-1.c.bartek-rekke-sandbox.internal:9094,15@kafka-poligon-dc3-1.c.bartek-rekke-sandbox.internal:9094

# kafka files goes here
log.dirs=/data/kafka/generic

# common options
listeners=CLIENTS://0.0.0.0:9092,CLUSTER://0.0.0.0:9093,CONTROLLER://0.0.0.0:9094"#;

        let properties = parse_server_properties(kraft_config);
        assert!(is_kraft_mode(&properties));
        assert!(!is_zookeeper_mode(&properties));
    }
}