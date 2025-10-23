use anyhow::Result;
use std::env;

pub fn handle_config_command() -> Result<()> {
    // Load .env file if it exists
    let env_file_loaded = dotenv::dotenv().is_ok();
    
    println!("🔧 KCPilot Configuration");
    println!("═══════════════════════════════════════");
    println!();

    // Display version information
    println!("📦 Version Information:");
    println!("  • KCPilot: v{}", env!("CARGO_PKG_VERSION"));
    println!("  • Authors: {}", env!("CARGO_PKG_AUTHORS"));
    println!("  • License: {}", env!("CARGO_PKG_LICENSE"));
    println!();
    
    // Display environment variables
    println!("🌍 Environment Configuration:");
    
    // Show .env file status
    if env_file_loaded {
        println!("  • .env file: ✅ Loaded");
    } else {
        println!("  • .env file: ⚠️  Not found (using system environment)");
    }
    
    // Check for OpenAI API key
    match env::var("OPENAI_API_KEY") {
        Ok(_) => println!("  • OPENAI_API_KEY: ✅ Set (hidden)"),
        Err(_) => println!("  • OPENAI_API_KEY: ❌ Not set"),
    }
    
    // Check for alternative LLM API key
    match env::var("LLM_API_KEY") {
        Ok(_) => println!("  • LLM_API_KEY: ✅ Set (hidden)"),
        Err(_) => println!("  • LLM_API_KEY: ❌ Not set"),
    }
    
    // Check for LLM debug mode
    match env::var("LLM_DEBUG") {
        Ok(val) => println!("  • LLM_DEBUG: {}", val),
        Err(_) => println!("  • LLM_DEBUG: false (default)"),
    }
    
    // Check for Rust log level
    match env::var("RUST_LOG") {
        Ok(val) => println!("  • RUST_LOG: {}", val),
        Err(_) => println!("  • RUST_LOG: info (default)"),
    }
    
    println!();
    
    // Display SSH configuration info
    println!("🔐 SSH Configuration:");
    let ssh_config_path = dirs::home_dir()
        .map(|h| h.join(".ssh/config"))
        .unwrap_or_else(|| std::path::PathBuf::from("~/.ssh/config"));
    
    if ssh_config_path.exists() {
        println!("  • SSH Config: ✅ Found at {}", ssh_config_path.display());
        
        // Try to count bastion hosts
        if let Ok(content) = std::fs::read_to_string(&ssh_config_path) {
            let host_count = content
                .lines()
                .filter(|line| line.trim_start().starts_with("Host ") && !line.contains("*"))
                .count();
            println!("  • Configured Hosts: {}", host_count);
        }
    } else {
        println!("  • SSH Config: ⚠️  Not found at {}", ssh_config_path.display());
    }
    
    println!();
    
    // Display paths
    println!("📁 Default Paths:");
    println!("  • Working Directory: {}", env::current_dir()?.display());
    println!("  • Analysis Tasks: ./analysis_tasks/");
    println!("  • Scan Output: kafka-scan-<timestamp>/");
    
    println!();
    
    // Display analysis configuration
    println!("🤖 Analysis Configuration:");
    let has_llm = env::var("OPENAI_API_KEY").is_ok() || env::var("LLM_API_KEY").is_ok();
    if has_llm {
        println!("  • AI Analysis: ✅ Enabled");
        println!("  • Default Timeout: 300 seconds");
        println!("  • Task Directory: analysis_tasks/");
    } else {
        println!("  • AI Analysis: ❌ Disabled (no API key)");
        println!("  • Fallback: Static configuration validator");
    }
    
    println!();
    println!("💡 Tips:");
    if !has_llm {
        println!("  • Set OPENAI_API_KEY or LLM_API_KEY to enable AI-powered analysis");
    }
    println!("  • Use RUST_LOG=debug for detailed logging");
    println!("  • Configure SSH hosts in ~/.ssh/config for remote scanning");
    
    Ok(())
}