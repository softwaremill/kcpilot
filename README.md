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

## Usage

### Basic Scan

```bash
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

## Development

See [DESIGN.md](DESIGN.md) for architecture and development details.

## License

MIT
