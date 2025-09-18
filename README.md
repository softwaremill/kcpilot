# KafkaPilot

A CLI-first Kafka health diagnostics tool that automatically collects cluster signals, identifies issues, and provides actionable remediation guidance.

## Features

- **SSH-based data collection** - Collects comprehensive cluster data through SSH bastion hosts
- **Zero configuration** - Works with standard SSH config aliases
- **Comprehensive data gathering** - Configs, logs, metrics, system info from all brokers
- **Organized output** - Timestamped directories with structured data organization
- **Summary reports** - Automatic generation of collection summaries

## Installation

```bash
# Build from source
cargo build --release

# The binary will be available at target/release/kafkapilot
```

## Running from Source

You can run KafkaPilot directly from source using `cargo run`:

```bash
# Run locally on bastion (no SSH required)
cargo run --bin kafkapilot -- scan

# Run remotely via SSH bastion
cargo run --bin kafkapilot -- scan --bastion kafka-poligon

# Specify custom output directory
cargo run --bin kafkapilot -- scan --bastion kafka-poligon --output test-scan

# Run with verbose logging
RUST_LOG=kafkapilot=debug cargo run --bin kafkapilot -- scan --bastion kafka-poligon

# Show help
cargo run --bin kafkapilot -- --help

# Show scan command help
cargo run --bin kafkapilot -- scan --help

# Show version and info
cargo run --bin kafkapilot -- info
```

## Usage

### Using the Compiled Binary

After building with `cargo build --release`, you can use the binary directly:

```bash
# Add to PATH (optional)
export PATH="$PATH:$(pwd)/target/release"

# Scan locally (when running directly on the bastion)
kafkapilot scan

# Scan remotely via SSH bastion (from your local machine)
kafkapilot scan --bastion kafka-poligon

# Scan with custom bastion from ~/.ssh/config
kafkapilot scan --bastion my-kafka-bastion

# Specify output directory
kafkapilot scan --output /path/to/output
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
- Log files (server.log, controller.log, journald)
- Data directory information and sizes
- Network connections and ports

## Commercial Support

We offer commercial support for Kafka and related technologies, as well as development services. Contact us to learn more about our offer!

## Copyright

Copyright (C) 2025 SoftwareMill https://softwaremill.com.
