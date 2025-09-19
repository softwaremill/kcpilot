---
layout: page
title: Quick Start
permalink: /quickstart/
---

# Quick Start

Get KafkaPilot running in under 5 minutes and perform your first Kafka cluster diagnostic.

## Prerequisites

- Rust toolchain (for building from source)
- SSH access to your Kafka cluster environment
- Basic familiarity with SSH configuration

## Installation

### Option 1: Build from Source (Recommended)

```bash
# Clone the repository
git clone https://github.com/softwaremill/kafkapilot.git
cd kafkapilot

# Build the binary
cargo build --release

# Add to PATH (optional)
export PATH="$PATH:$(pwd)/target/release"
```

### Option 2: Run from Source

```bash
# Clone and run directly with cargo
git clone https://github.com/softwaremill/kafkapilot.git
cd kafkapilot

# Run without building
cargo run --bin kafkapilot -- --help
```

## First Scan

### Local Scan (from bastion host)

If you're running directly on a machine that has access to your Kafka brokers:

```bash
kafkapilot scan
```

### Remote Scan (through SSH bastion)

If you need to connect through a bastion host, first ensure your SSH config is set up:

```bash
# Add to ~/.ssh/config
Host kafka-prod
    HostName your-bastion.example.com
    User your-username
    ForwardAgent yes
    IdentityFile ~/.ssh/your-key
```

Then run the scan:

```bash
kafkapilot scan --bastion kafka-prod
```

## Understanding the Output

KafkaPilot creates a timestamped directory with all collected data:

```
kafka-scan-2024-01-15-14-30-45/
├── brokers/                    # Individual broker data
│   ├── broker_1/
│   │   ├── configs/           # Configuration files
│   │   ├── logs/              # Log files
│   │   ├── metrics/           # JVM and system metrics
│   │   └── system/            # System information
│   └── ...
├── cluster/                    # Cluster-wide information
├── metrics/                    # Prometheus metrics (if available)
├── COLLECTION_SUMMARY.md       # Human-readable summary
└── scan_metadata.json         # Scan metadata
```

## Basic Analysis

### View Collection Summary

```bash
cat ./kafka-scan-*/COLLECTION_SUMMARY.md
```

### Run AI Analysis (if OpenAI API key configured)

```bash
# Set up AI analysis
export OPENAI_API_KEY=your_api_key_here

# Analyze the collected data
kafkapilot analyze ./kafka-scan-2024-01-15-14-30-45 --report terminal

# Generate multiple report formats
kafkapilot analyze ./kafka-scan-2024-01-15-14-30-45 --report terminal,markdown,json
```

### Test Specific Issues

```bash
# List available analysis tasks
kafkapilot task list

# Test for specific configuration issues
kafkapilot task test replication_factor ./kafka-scan-2024-01-15-14-30-45
kafkapilot task test jvm_heap_size ./kafka-scan-2024-01-15-14-30-45
```

## Common Troubleshooting

### SSH Connection Issues

```bash
# Test SSH connectivity
ssh kafka-prod "echo 'Connection successful'"

# Verify SSH agent has your key loaded
ssh-add -l
```

### Permission Issues

Ensure your SSH user has appropriate permissions:
- Read access to Kafka configuration directories
- Access to log files (may require sudo)
- Ability to run system commands like `ps`, `df`, `free`

### No Data Collected

If brokers aren't discovered automatically:

```bash
# Check cluster discovery
kafkapilot scan --bastion kafka-prod --debug

# Manual broker specification (future feature)
# kafkapilot scan --bastion kafka-prod --brokers broker1,broker2,broker3
```

## Next Steps

- **[Installation Guide](installation.html)** - Detailed installation options and configuration
- **[Tutorials](tutorials.html)** - Step-by-step guides for common scenarios  
- **[Examples](examples.html)** - Real-world troubleshooting workflows
- **[API Reference](api.html)** - Complete command-line interface documentation

## Getting Help

- 📖 [Full Documentation](../)
- 🐛 [Report Issues](https://github.com/softwaremill/kafkapilot/issues)
- 💬 [GitHub Discussions](https://github.com/softwaremill/kafkapilot/discussions)
- 🏢 [Professional Support](support.html)

---

**Next**: Learn about [advanced installation options](installation.html) or dive into [detailed tutorials](tutorials.html).