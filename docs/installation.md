---
layout: page
title: Installation
permalink: /installation/
---

Multiple ways to install and run KCPilot depending on your environment and preferences.

## Prerequisites

### System Requirements
- **Operating System**: Linux, macOS, or Windows with WSL
- **Rust**: 1.70+ (for building from source)
- **SSH**: OpenSSH client configured
- **Memory**: 100MB+ available RAM
- **Disk**: 50MB+ for binary, additional space for scan outputs

### Network Requirements
- SSH access to Kafka cluster bastion hosts
- SSH agent with appropriate keys loaded
- Network connectivity from bastion to Kafka brokers

## Installation Methods

### 1. Build from Source (Recommended)

This method gives you the latest features and allows customization:

```bash
# Clone the repository
git clone https://github.com/softwaremill/kcpilot.git
cd kcpilot

# Build release binary
cargo build --release

# Binary location
ls -la target/release/kcpilot

# Optional: Add to PATH
echo 'export PATH="$PATH:'$(pwd)'/target/release"' >> ~/.bashrc
source ~/.bashrc
```

### 2. Development Mode

For development or trying the latest changes:

```bash
# Clone and run directly
git clone https://github.com/softwaremill/kcpilot.git
cd kcpilot

# Run without building
cargo run --bin kcpilot -- --help

# Run with debug logging
RUST_LOG=kcpilot=debug cargo run --bin kcpilot -- scan --bastion kafka-prod --broker kafka-poligon-dc1-1.c.sml-sandbox.internal:9092
```

### 3. Future Release Options

*These installation methods are planned for future releases:*

```bash
# Coming soon: Binary releases
curl -L https://github.com/softwaremill/kcpilot/releases/latest/download/kcpilot-linux-x64 -o kcpilot
chmod +x kcpilot

# Coming soon: Homebrew (macOS)
brew install softwaremill/tap/kcpilot

# Coming soon: Cargo install
cargo install kcpilot
```

## Configuration

### SSH Configuration

KCPilot relies on your SSH configuration for accessing bastion hosts. Configure your `~/.ssh/config`:

```bash
# Example SSH configuration
Host kafka-prod
    HostName kafka-prod.example.com
    User kafka-admin
    ForwardAgent yes
    IdentityFile ~/.ssh/kafka-prod-key
    ConnectTimeout 30
    ServerAliveInterval 60

Host kafka-staging  
    HostName kafka-staging.example.com
    User ubuntu
    ForwardAgent yes
    IdentityFile ~/.ssh/kafka-staging-key
    ProxyJump bastion.example.com

Host kafka-dev
    HostName 192.168.1.100
    User kafkauser
    IdentityFile ~/.ssh/kafka-dev-key
```

### SSH Agent Setup

Ensure your SSH keys are loaded:

```bash
# Start SSH agent (if not running)
eval $(ssh-agent)

# Add your keys
ssh-add ~/.ssh/kafka-prod-key
ssh-add ~/.ssh/kafka-staging-key

# Verify keys are loaded
ssh-add -l
```

### Environment Variables

Configure optional environment variables:

```bash
# AI Analysis (optional)
export OPENAI_API_KEY=your_openai_api_key_here
# Alternative: export LLM_API_KEY=your_alternative_llm_api_key

# Default output directory (optional)
export KCPILOT_OUTPUT_DIR=/home/user/kafka-diagnostics

# Logging level (optional)
export RUST_LOG=kcpilot=info

# LLM debugging (optional)
export LLM_DEBUG=true
```

## Verification

### Test Installation

```bash
# Check version and basic functionality
kcpilot --version
kcpilot --help

```

### Minimal Test

```bash
# Simple local test (if running on bastion)
kcpilot scan --broker kafka-poligon-dc1-1.c.sml-sandbox.internal:9092 --output test-installation

# Verify output structure
ls -la test-installation/
cat test-installation/COLLECTION_SUMMARY.md
```
---

**Need help?** Check our [support options](https://softwaremill.com/services/apache-kafka-services/) or [report an issue](https://github.com/softwaremill/kcpilot/issues).