---
layout: page
title: Installation
permalink: /installation/
---

# Installation Guide

Multiple ways to install and run KafkaPilot depending on your environment and preferences.

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
git clone https://github.com/softwaremill/kafkapilot.git
cd kafkapilot

# Build release binary
cargo build --release

# Binary location
ls -la target/release/kafkapilot

# Optional: Add to PATH
echo 'export PATH="$PATH:'$(pwd)'/target/release"' >> ~/.bashrc
source ~/.bashrc
```

### 2. Development Mode

For development or trying the latest changes:

```bash
# Clone and run directly
git clone https://github.com/softwaremill/kafkapilot.git
cd kafkapilot

# Run without building
cargo run --bin kafkapilot -- --help

# Run with debug logging
RUST_LOG=kafkapilot=debug cargo run --bin kafkapilot -- scan --bastion kafka-prod
```

### 3. Future Release Options

*These installation methods are planned for future releases:*

```bash
# Coming soon: Binary releases
curl -L https://github.com/softwaremill/kafkapilot/releases/latest/download/kafkapilot-linux-x64 -o kafkapilot
chmod +x kafkapilot

# Coming soon: Homebrew (macOS)
brew install softwaremill/tap/kafkapilot

# Coming soon: Cargo install
cargo install kafkapilot
```

## Configuration

### SSH Configuration

KafkaPilot relies on your SSH configuration for accessing bastion hosts. Configure your `~/.ssh/config`:

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
export KAFKAPILOT_OUTPUT_DIR=/home/user/kafka-diagnostics

# Logging level (optional)
export RUST_LOG=kafkapilot=info

# LLM debugging (optional)
export LLM_DEBUG=true
```

## Verification

### Test Installation

```bash
# Check version and basic functionality
kafkapilot --version
kafkapilot --help

# Test SSH connectivity
kafkapilot scan --bastion kafka-prod --dry-run
```

### Minimal Test

```bash
# Simple local test (if running on bastion)
kafkapilot scan --output test-installation

# Verify output structure
ls -la test-installation/
cat test-installation/COLLECTION_SUMMARY.md
```

## Deployment Options

### Single-User Installation

```bash
# Install to user's home directory
mkdir -p ~/.local/bin
cp target/release/kafkapilot ~/.local/bin/
echo 'export PATH="$PATH:$HOME/.local/bin"' >> ~/.bashrc
```

### System-Wide Installation

```bash
# Install system-wide (requires sudo)
sudo cp target/release/kafkapilot /usr/local/bin/
sudo chmod +x /usr/local/bin/kafkapilot

# Verify installation
which kafkapilot
kafkapilot --version
```

### Container Deployment

Create a container with KafkaPilot for isolated execution:

```dockerfile
# Dockerfile
FROM rust:1.70 as builder
COPY . /app
WORKDIR /app
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    openssh-client \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/kafkapilot /usr/local/bin/
COPY --from=builder /app/analysis_tasks /usr/share/kafkapilot/analysis_tasks

# Mount SSH config and keys at runtime
VOLUME ["/root/.ssh"]
ENTRYPOINT ["kafkapilot"]
```

```bash
# Build and run
docker build -t kafkapilot .
docker run -v ~/.ssh:/root/.ssh:ro kafkapilot scan --bastion kafka-prod
```

## Upgrading

### From Source

```bash
cd kafkapilot
git pull origin main
cargo build --release

# Backup existing binary (optional)
cp target/release/kafkapilot target/release/kafkapilot.backup.$(date +%Y%m%d)
```

### Configuration Migration

When upgrading, check for new configuration options:

```bash
# Compare available commands
kafkapilot --help

# Check for new analysis tasks
kafkapilot task list --detailed

# Review new environment variables
kafkapilot info
```

## Troubleshooting

### Build Issues

```bash
# Update Rust toolchain
rustup update

# Clear cargo cache
cargo clean
cargo build --release

# Check dependencies
cargo check
```

### SSH Issues

```bash
# Test SSH connectivity manually
ssh kafka-prod "echo 'SSH test successful'"

# Debug SSH configuration
ssh -v kafka-prod

# Check SSH agent
ssh-add -l
echo $SSH_AUTH_SOCK
```

### Permission Issues

```bash
# Verify file permissions
ls -la target/release/kafkapilot

# Fix executable permissions
chmod +x target/release/kafkapilot

# Check PATH
echo $PATH
which kafkapilot
```

## Performance Tuning

### Resource Optimization

```bash
# Reduce memory usage for large clusters
export KAFKAPILOT_MAX_PARALLEL_BROKERS=3

# Limit log collection size
export KAFKAPILOT_MAX_LOG_SIZE=100MB

# Configure timeouts
export KAFKAPILOT_SSH_TIMEOUT=30
export KAFKAPILOT_COLLECTION_TIMEOUT=600
```

## Next Steps

- **[Quick Start](quickstart.html)** - Get running in 5 minutes
- **[Tutorials](tutorials.html)** - Learn common diagnostic workflows
- **[Examples](examples.html)** - Real-world troubleshooting scenarios
- **[API Reference](api.html)** - Complete command documentation

---

**Need help?** Check our [support options](support.html) or [report an issue](https://github.com/softwaremill/kafkapilot/issues).