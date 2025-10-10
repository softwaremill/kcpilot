# KafkaPilot

A CLI-first Kafka health diagnostics tool that automatically collects cluster signals, identifies issues, and provides actionable remediation guidance. KafkaPilot combines SSH-based data collection with AI-powered analysis using LLM integration.

## Features

- **SSH-based data collection** - Collects comprehensive cluster data through SSH bastion hosts
- **AI-powered analysis** - Uses LLMs to analyze collected data and identify issues
- **Configurable analysis tasks** - YAML-defined analysis prompts for customizable diagnostics
- **Per-broker analysis** - Intelligent chunking to handle large clusters without token limits
- **Zero configuration** - Works with standard SSH config aliases
- **Multiple output formats** - Terminal, JSON, and Markdown reports
- **Organized output** - Timestamped directories with structured data organization
- **Summary reports** - Automatic generation of collection summaries

## Installation

```bash
# Build from source
cargo build --release

# The binary will be available at target/release/kafkapilot
```

## Environment Configuration

### Required Environment Variables
```bash
# For AI-powered analysis (required for LLM features)
export OPENAI_API_KEY=your_openai_api_key_here

# Optional LLM debugging
export LLM_DEBUG=true
```

## Quick Start

You can run KafkaPilot directly from source using `cargo run`:

```bash
# 1. Scan a cluster (data collection)
cargo run --bin kafkapilot -- scan --bastion kafka-poligon --broker kafka-broker-1.internal:9092 --output test-scan

# 2. Analyze the collected data with AI
cargo run --bin kafkapilot -- analyze ./test-scan --report terminal

# 3. Generate markdown report
cargo run --bin kafkapilot -- analyze ./test-scan --report markdown
```

## Commands Overview

### Data Collection

#### Local Scan (running directly on bastion host)
```bash
# Basic local scan - must specify at least one broker
cargo run --bin kafkapilot -- scan --broker kafka-broker-1.internal:9092

# Local scan with custom output directory
cargo run --bin kafkapilot -- scan --broker kafka-broker-1.internal:9092 --output my-cluster-scan

# With debug logging
RUST_LOG=kafkapilot=debug cargo run --bin kafkapilot -- scan --broker kafka-broker-1.internal:9092
```

#### Remote Scan (via SSH bastion)
```bash
# Basic remote scan - requires both bastion and broker
cargo run --bin kafkapilot -- scan --bastion kafka-poligon --broker kafka-broker-1.internal:9092

# Remote scan with custom output directory
cargo run --bin kafkapilot -- scan --bastion kafka-poligon --broker kafka-broker-1.internal:9092 --output test-scan

# With debug logging
RUST_LOG=kafkapilot=debug cargo run --bin kafkapilot -- scan --bastion kafka-poligon --broker kafka-broker-1.internal:9092
```

### Analysis
```bash
# Analyze collected data (terminal output)
cargo run --bin kafkapilot -- analyze ./test-scan

# Generate JSON report
cargo run --bin kafkapilot -- analyze ./test-scan --report json 

# Generate markdown report
cargo run --bin kafkapilot -- analyze ./test-scan --report markdown

# Enable LLM debug logging
cargo run --bin kafkapilot -- analyze ./test-scan --llmdbg

# Custom LLM timeout (default: 300s)
cargo run --bin kafkapilot -- analyze ./test-scan --llm-timeout 600
```

### Analysis Task Management
```bash
# List all available analysis tasks
cargo run --bin kafkapilot -- task list

# List tasks with detailed information
cargo run --bin kafkapilot -- task list --detailed

# Test a specific analysis task
cargo run --bin kafkapilot -- task test recent_log_errors ./test-scan

# Show details of a specific task
cargo run --bin kafkapilot -- task show recent_log_errors

```

### Utility Commands
```bash
# Test SSH connectivity to brokers
cargo run --bin kafkapilot -- test-ssh --bastion kafka-poligon --broker kafka-broker-1.internal:9092

# Show configuration
cargo run --bin kafkapilot -- config

# Show help
cargo run --bin kafkapilot -- --help

# Show version and info
cargo run --bin kafkapilot -- info
```

## Using the Compiled Binary

After building with `cargo build --release`, you can use the binary directly:

```bash
# Add to PATH (optional)
export PATH="$PATH:$(pwd)/target/release"

# Complete workflow: scan + analyze (remote)
kafkapilot scan --bastion kafka-poligon --broker kafka-broker-1.internal:9092 
kafkapilot analyze ./my-cluster-scan --report terminal

# Complete workflow: scan + analyze (local)
kafkapilot scan --broker kafka-broker-1.internal:9092 --output my-local-scan
kafkapilot analyze ./my-local-scan --report terminal

# Generate reports
kafkapilot analyze ./my-cluster-scan --report markdown 
```

### Prerequisites

#### For Local Scan (running on bastion)
- Direct SSH access to Kafka brokers
- `kafkactl` installed
- Optional: `kafka_exporter` for Prometheus metrics

#### For Remote Scan (running from your local machine)

1. **SSH Configuration**: Your SSH bastion should be configured in `~/.ssh/config`:
   ```
   Host kafka-poligon
       HostName your-bastion-host.example.com
       User your-username
       ForwardAgent yes
       IdentityFile ~/.ssh/your-key
   ```

2. **SSH Agent**: Ensure your SSH key is loaded:
   ```bash
   ssh-add ~/.ssh/your-key
   ```

3. **Bastion Requirements**: The bastion host should have:
   - `kafkactl` for cluster metadata
   - SSH access to individual Kafka brokers
   - Optional: `kafka_exporter` for Prometheus metrics

## Output Structure

```
kafka-scan-TIMESTAMP/
├── brokers/           # Per-broker data
│   ├── broker_11/
│   │   ├── configs/   # Configuration files
│   │   ├── logs/      # Log files
│   │   ├── metrics/   # JVM metrics
│   │   ├── system/    # System information
│   │   └── data/      # Data directory info
│   └── ...
├── cluster/           # Cluster-wide data
│   └── kafkactl/      # Broker lists, topics, consumer groups
├── metrics/           # Prometheus metrics
├── system/            # Bastion system info
├── COLLECTION_SUMMARY.md
└── scan_metadata.json
```

## Data Collected

### Cluster Level (from bastion)
- Broker list and configurations
- Topic list and details
- Consumer groups
- Prometheus metrics (if available)
- Bastion system information

### Per Broker
- System information (CPU, memory, disk)
- Java/JVM information and metrics
- Configuration files (server.properties, log4j, etc.)
- **Enhanced log collection** - Dynamically discovers log files from any Kafka deployment:
  - Automatic process discovery and service analysis
  - AI-powered log4j configuration parsing
  - Both file-based logs and systemd journal logs
  - Works with any log directory structure
- Data directory information and sizes
- Network connections and ports

## Analysis Tasks System

KafkaPilot uses a sophisticated analysis system powered by YAML-defined tasks that leverage AI for intelligent cluster diagnostics.

### Built-in Analysis Tasks

- **Recent Log Error Detection** - Scans broker logs for ERROR/FATAL messages and connectivity issues
- **JVM Heap Configuration** - Validates heap size settings and memory allocation
- **Thread Configuration** - Checks broker thread pool settings
- **Authentication & Authorization** - Verifies security configuration
- **High Availability Checks** - Ensures proper HA setup (broker count, ISR, etc.)
- **Network Configuration** - Validates listener and encryption settings
- **And many more...**

### Creating Custom Tasks

Create custom analysis tasks by adding YAML files to the `analysis_tasks/` directory:

```yaml
id: my_custom_check
name: My Custom Analysis
description: Custom analysis for specific requirements
category: performance

prompt: |
  Analyze the Kafka configuration and logs for custom requirements:
  Configuration: {config}
  Logs: {logs}
  
  Look for specific patterns and provide recommendations.

include_data:
  - config
  - logs

# For large clusters, enable per-broker analysis to avoid token limits
per_broker_analysis: true
max_tokens_per_request: 80000

severity_keywords:
  "critical issue": "high"
  "minor issue": "low"

default_severity: medium
enabled: true
```

### Per-Broker Analysis

For large clusters that exceed LLM token limits, KafkaPilot automatically:
- Processes each broker individually when `per_broker_analysis: true`
- Filters data to only include relevant information (logs + configs for log analysis)
- Combines findings from all brokers into a comprehensive report
- Handles any cluster size without token limit errors

## Commercial Support

We offer commercial support for Kafka and related technologies, as well as development services. Contact us to learn more about our offer!

## Copyright

Copyright (C) 2025 SoftwareMill https://softwaremill.com.
