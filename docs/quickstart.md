---
layout: page
title: Quick Start
permalink: /quickstart/
---

Get KafkaPilot running in under 5 minutes and perform your first Kafka cluster diagnostic.

## Prerequisites

- Rust toolchain (for building from source)
- SSH access to your Kafka cluster environment
- Basic familiarity with SSH configuration

## Installation

### Build from Source (Recommended)

```bash
# Clone the repository
git clone https://github.com/softwaremill/kafkapilot.git
cd kafkapilot

# Build the binary
cargo build --release

# Add to PATH (optional)
export PATH="$PATH:$(pwd)/target/release"
```

## First Scan

### Local Scan (from bastion host)

If you're running directly on a machine that has access to your Kafka brokers:

```bash
kafkapilot scan --broker kafka-poligon-dc1-1.c.sml-sandbox.internal:9092
```
where `--broker` is an address to one of your brokers

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
kafkapilot scan --bastion kafka-prod --broker kafka-poligon-dc1-1.c.sml-sandbox.internal:9092
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

```
Possible report formats are: `terminal`, `markdown` or `json` 

### Test Specific Issues

```bash
# List available analysis tasks
kafkapilot task list

# Test for specific configuration issues
kafkapilot task test minimum_cpu_cores ./kafka-scan-2024-01-15-14-30-45
kafkapilot task test in_transit_encryption ./kafka-scan-2024-01-15-14-30-45
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