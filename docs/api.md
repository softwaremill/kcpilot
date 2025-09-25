---
layout: page
title: API Reference
permalink: /api/
---

# API Reference

Complete command-line interface documentation for KafkaPilot.

## Global Options

All KafkaPilot commands support these global options:

```bash
kafkapilot [GLOBAL_OPTIONS] <COMMAND> [COMMAND_OPTIONS]
```

### Global Flags

| Flag | Description |
|------|-------------|
| `-h, --help` | Show help information |
| `-V, --version` | Show version information |
| `-v, --verbose` | Enable verbose output |
| `-q, --quiet` | Suppress non-error output |

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `RUST_LOG` | Logging level (error, warn, info, debug, trace) | `info` |
| `OPENAI_API_KEY` | OpenAI API key for AI analysis | - |
| `LLM_API_KEY` | Alternative LLM API key | - |
| `LLM_DEBUG` | Enable LLM debugging | `false` |
| `KAFKAPILOT_OUTPUT_DIR` | Default output directory | `./` |

---

## Commands

### `scan` - Data Collection

Collect comprehensive data from Kafka cluster.

```bash
kafkapilot scan [OPTIONS]
```

#### Options

| Option | Description | Default |
|--------|-------------|---------|
| `--broker <HOST:PORT>` | **Required.** One or more broker addresses (comma-separated) | - |
| `--bastion <HOST>` | SSH bastion host alias from `~/.ssh/config` | Local execution |
| `--output <DIR>` | Output directory for collected data | `kafka-scan-<timestamp>` |
| `--timeout <SECONDS>` | SSH operation timeout | `300` |
| `--parallel <NUM>` | Maximum parallel broker connections | `5` |
| `--dry-run` | Show what would be collected without executing | `false` |

#### Examples

```bash
# Local scan (from bastion host)
kafkapilot scan --broker kafka-broker-1.internal:9092

# Remote scan via SSH bastion
kafkapilot scan --bastion kafka-prod --broker kafka-broker-1.internal:9092

# Custom output directory
kafkapilot scan --bastion kafka-prod --broker kafka-broker-1.internal:9092 --output health-check-2024-01-15

# Parallel collection with timeout
kafkapilot scan --bastion kafka-prod --broker kafka-broker-1.internal:9092 --parallel 3 --timeout 600

# Dry run to see what would be collected
kafkapilot scan --bastion kafka-prod --broker kafka-broker-1.internal:9092 --dry-run
```

#### Output Structure

```
kafka-scan-TIMESTAMP/
├── scan_metadata.json          # Scan execution metadata
├── COLLECTION_SUMMARY.md       # Human-readable summary
├── brokers/                    # Per-broker data
│   ├── broker_<id>/
│   │   ├── configs/           # Configuration files
│   │   │   ├── server.properties
│   │   │   ├── log4j.properties
│   │   │   └── jvm_args.txt
│   │   ├── logs/              # Log files
│   │   │   ├── server.log
│   │   │   ├── controller.log
│   │   │   └── journald.log
│   │   ├── metrics/           # JVM and system metrics
│   │   │   ├── jstat_gc.txt
│   │   │   ├── jmap_histo.txt
│   │   │   └── thread_dump.txt
│   │   ├── system/            # System information
│   │   │   ├── ps_aux.txt
│   │   │   ├── df.txt
│   │   │   ├── free.txt
│   │   │   ├── netstat.txt
│   │   │   └── uptime.txt
│   │   └── data/              # Data directory info
│   │       ├── disk_usage.txt
│   │       └── log_sizes.txt
├── cluster/                    # Cluster-wide data
│   └── kafkactl/              # kafkactl outputs
│       ├── brokers.json
│       ├── topics.json
│       └── consumer_groups.json
├── metrics/                    # Prometheus metrics
│   └── kafka_metrics.txt
└── system/                     # Bastion system info
    ├── system_info.txt
    └── network_info.txt
```

---

### `analyze` - AI-Powered Analysis

Analyze collected data using AI and generate insights.

```bash
kafkapilot analyze [OPTIONS] <SCAN_DIRECTORY>
```

#### Options

| Option | Description | Default |
|--------|-------------|---------|
| `--report <FORMAT>` | Output format: `terminal`, `markdown`, `json` | `terminal` |
| `--output <FILE>` | Output file (for non-terminal formats) | Auto-generated |
| `--llm-timeout <SECONDS>` | LLM request timeout | `60` |
| `--llmdbg` | Enable LLM debugging | `false` |
| `--tasks <LIST>` | Comma-separated list of specific tasks to run | All tasks |

#### Report Formats

**Terminal** - Interactive colored output for immediate viewing:
```bash
kafkapilot analyze ./kafka-scan-data --report terminal
```

**Markdown** - Formatted report suitable for documentation:
```bash
kafkapilot analyze ./kafka-scan-data --report markdown --output report.md
```

**JSON** - Structured data for automation and integration:
```bash
kafkapilot analyze ./kafka-scan-data --report json --output analysis.json
```

**Multiple formats** - Generate all formats:
```bash
kafkapilot analyze ./kafka-scan-data --report terminal,markdown,json
```

#### Examples

```bash
# Basic analysis with terminal output
kafkapilot analyze ./kafka-scan-20240115

# Generate markdown report
kafkapilot analyze ./kafka-scan-20240115 --report markdown

# JSON output for automation
kafkapilot analyze ./kafka-scan-20240115 --report json > analysis.json

# Run specific analysis tasks only
kafkapilot analyze ./kafka-scan-20240115 --tasks jvm_heap_memory,thread_configuration

# Extended timeout for large clusters
kafkapilot analyze ./kafka-scan-20240115 --llm-timeout 120
```

#### JSON Output Schema

```json
{
  "metadata": {
    "scan_directory": "./kafka-scan-20240115",
    "analysis_timestamp": "2024-01-15T14:30:45Z",
    "kafkapilot_version": "0.1.0",
    "tasks_executed": ["task1", "task2", "..."]
  },
  "cluster_summary": {
    "broker_count": 6,
    "topic_count": 245,
    "partition_count": 2940,
    "consumer_group_count": 38
  },
  "findings": [
    {
      "task_id": "jvm_heap_memory",
      "category": "performance",
      "severity": "warning",
      "title": "JVM Heap Memory Usage High",
      "description": "Multiple brokers showing heap usage >80%",
      "affected_brokers": ["broker_1", "broker_3"],
      "recommendation": "Increase JVM heap size or optimize memory usage",
      "details": {
        "current_usage": "85%",
        "recommended_action": "Increase -Xmx to 8GB"
      }
    }
  ],
  "health_score": 85,
  "recommendations": [
    "Increase JVM heap size on affected brokers",
    "Review log retention policies",
    "Monitor disk usage trends"
  ]
}
```

---

### `task` - Analysis Task Management

Manage and execute individual analysis tasks.

```bash
kafkapilot task <SUBCOMMAND> [OPTIONS]
```

#### Subcommands

##### `list` - List Available Tasks

```bash
kafkapilot task list [OPTIONS]
```

**Options:**
- `--detailed` - Show full task descriptions and metadata
- `--category <CATEGORY>` - Filter by category (performance, security, configuration, etc.)

**Examples:**
```bash
# List all tasks
kafkapilot task list

# Detailed task information
kafkapilot task list --detailed

# Performance-related tasks only
kafkapilot task list --category performance
```

##### `test` - Execute Single Task

```bash
kafkapilot task test <TASK_ID> <SCAN_DIRECTORY> [OPTIONS]
```

**Options:**
- `--output <FORMAT>` - Output format: `terminal`, `json`
- `--llm-timeout <SECONDS>` - LLM request timeout

**Examples:**
```bash
# Test JVM heap memory configuration
kafkapilot task test jvm_heap_memory ./kafka-scan-data

# Test with JSON output
kafkapilot task test thread_configuration ./kafka-scan-data --output json

# Extended timeout
kafkapilot task test comprehensive_analysis ./kafka-scan-data --llm-timeout 180
```

##### `new` - Create Custom Task (Future)

*Planned for future release*

```bash
kafkapilot task new --id custom_check --name "Custom Check" --category compliance
```

#### Built-in Analysis Tasks

| Task ID | Category | Description |
|---------|----------|-------------|
| `jvm_heap_memory` | performance | Check JVM heap memory usage and configuration |
| `jvm_heap_size_limit` | performance | Validate JVM heap size limits |
| `jvm_heap_preallocation` | performance | Check heap preallocation settings |
| `thread_configuration` | performance | Analyze thread pool configurations |
| `minimum_cpu_cores` | performance | Validate CPU core requirements |
| `multiple_log_dirs` | performance | Check log directory distribution |
| `log_dirs_disk_size_uniformity` | performance | Analyze disk usage uniformity |
| `isr_replication_margin` | reliability | Check In-Sync Replica margins |
| `separate_listeners` | security | Validate listener separation |
| `in_transit_encryption` | security | Check encryption in transit |
| `authentication_authorization` | security | Validate auth configuration |
| `zookeeper_ha_check` | reliability | ZooKeeper high availability check |
| `zookeeper_heap_memory` | performance | ZooKeeper JVM heap analysis |
| `kraft_controller_ha_check` | reliability | KRaft controller HA validation |

---

### `info` - System Information

Display KafkaPilot version and system information.

```bash
kafkapilot info [OPTIONS]
```

#### Options

| Option | Description |
|--------|-------------|
| `--check-deps` | Check for required dependencies |
| `--env` | Show relevant environment variables |

#### Examples

```bash
# Basic version information
kafkapilot info

# Check dependencies
kafkapilot info --check-deps

# Show environment configuration
kafkapilot info --env
```

#### Output

```
KafkaPilot v0.1.0
Built: 2024-01-15T10:30:00Z
Commit: abc123def456
Rust version: 1.70.0

System Information:
  OS: Linux 5.4.0-74-generic
  Architecture: x86_64
  Available memory: 16 GB
  SSH client: OpenSSH_8.9p1

Dependencies:
  ✅ SSH client available
  ✅ Required system tools present
  ⚠️  kafkactl not found in PATH (required for remote scans)
  ❌ OpenAI API key not configured (AI analysis disabled)

Environment Variables:
  RUST_LOG: info
  OPENAI_API_KEY: [configured]
  LLM_DEBUG: false
```

---

## Exit Codes

KafkaPilot uses standard exit codes to indicate execution status:

| Code | Description | Usage |
|------|-------------|-------|
| `0` | Success | Command completed successfully |
| `1` | General error | Command failed due to error |
| `2` | Misuse | Invalid command line arguments |
| `3` | SSH error | SSH connection or authentication failed |
| `4` | Collection error | Data collection failed |
| `5` | Analysis error | AI analysis failed |
| `6` | IO error | File system operation failed |

### Error Handling in Scripts

```bash
#!/bin/bash
kafkapilot scan --bastion kafka-prod --broker kafka-broker-1.internal:9092 --output health-check

case $? in
  0)
    echo "Scan completed successfully"
    ;;
  3)
    echo "SSH connection failed - check credentials and connectivity"
    exit 1
    ;;
  4)
    echo "Data collection failed - check permissions and tools"
    exit 1
    ;;
  *)
    echo "Unexpected error occurred"
    exit 1
    ;;
esac
```

---

## Configuration Files

### SSH Configuration

KafkaPilot relies on standard SSH configuration in `~/.ssh/config`:

```
# Production Kafka cluster
Host kafka-prod
    HostName kafka-prod.example.com
    User kafka-admin
    ForwardAgent yes
    IdentityFile ~/.ssh/kafka-prod-key
    ConnectTimeout 30
    ServerAliveInterval 60
    
# Staging cluster with jump host
Host kafka-staging
    HostName kafka-staging.internal
    User ubuntu
    ProxyJump bastion.example.com
    ForwardAgent yes
    IdentityFile ~/.ssh/kafka-staging-key

# Development cluster
Host kafka-dev
    HostName 192.168.1.100
    User kafkauser
    IdentityFile ~/.ssh/kafka-dev-key
    Port 2222
```

### Analysis Task Configuration

Custom analysis tasks are defined in YAML format in the `analysis_tasks/` directory:

```yaml
# analysis_tasks/custom_compliance_check.yaml
id: custom_compliance_check
name: "Custom Compliance Check"
description: "Validates cluster configuration against company standards"
category: "compliance"
include_data:
  - config
  - cluster
prompt: |
  Review the Kafka cluster configuration for compliance with company standards:
  
  1. Replication factor must be >= 3
  2. Log retention must be <= 7 days
  3. All topics must follow naming convention
  
  Configuration: {{config}}
  Topics: {{cluster.topics}}
  
  Report any non-compliant configurations.
severity_keywords:
  critical: ["violation", "non-compliant", "critical"]
  warning: ["recommend", "should", "warning"]
  info: ["compliant", "meets", "follows"]
timeout: 120
```

---

## Troubleshooting Commands

### Debug Output

Enable debug logging for troubleshooting:

```bash
# Detailed execution logging
RUST_LOG=kafkapilot=debug kafkapilot scan --bastion kafka-prod --broker kafka-broker-1.internal:9092

# Trace level (very verbose)
RUST_LOG=trace kafkapilot scan --bastion kafka-prod --broker kafka-broker-1.internal:9092

# LLM debugging
LLM_DEBUG=true kafkapilot analyze ./kafka-scan-data
```

### Connectivity Testing

```bash
# Test SSH connectivity
kafkapilot scan --bastion kafka-prod --broker kafka-broker-1.internal:9092 --dry-run

# Manual SSH test
ssh kafka-prod "echo 'Connection successful'"

# Check required tools
kafkapilot info --check-deps
```

### Performance Monitoring

```bash
# Monitor scan progress
kafkapilot scan --bastion kafka-prod --broker kafka-broker-1.internal:9092 --verbose

# Time execution
time kafkapilot analyze ./kafka-scan-data

# Memory usage monitoring
/usr/bin/time -v kafkapilot analyze ./kafka-scan-data
```

---

## Integration Examples

### Shell Scripts

```bash
#!/bin/bash
# automated-health-check.sh

set -euo pipefail

BASTION="${1:-kafka-prod}"
OUTPUT_DIR="health-check-$(date +%Y%m%d-%H%M%S)"

echo "Starting Kafka health check for $BASTION..."

# Collect data
kafkapilot scan --bastion "$BASTION" --broker kafka-broker-1.internal:9092 --output "$OUTPUT_DIR"

# Analyze if API key available
if [ -n "${OPENAI_API_KEY:-}" ]; then
    echo "Running AI analysis..."
    kafkapilot analyze "$OUTPUT_DIR" --report json > "$OUTPUT_DIR/analysis.json"
    
    # Check for critical issues
    CRITICAL_COUNT=$(jq '.findings[] | select(.severity == "critical") | length' "$OUTPUT_DIR/analysis.json")
    
    if [ "$CRITICAL_COUNT" -gt 0 ]; then
        echo "⚠️  ALERT: $CRITICAL_COUNT critical issues found!"
        exit 1
    else
        echo "✅ No critical issues detected"
    fi
else
    echo "ℹ️  AI analysis skipped (no OpenAI API key configured)"
fi

echo "Health check completed: $OUTPUT_DIR"
```

### Python Integration

```python
#!/usr/bin/env python3
import subprocess
import json
import sys
from pathlib import Path

def run_kafka_health_check(bastion_host: str) -> dict:
    """Run KafkaPilot health check and return analysis results."""
    
    # Run scan
    scan_cmd = ['kafkapilot', 'scan', '--bastion', bastion_host, '--output', 'health-check']
    result = subprocess.run(scan_cmd, check=True, capture_output=True, text=True)
    
    # Run analysis
    analyze_cmd = ['kafkapilot', 'analyze', 'health-check', '--report', 'json']
    result = subprocess.run(analyze_cmd, check=True, capture_output=True, text=True)
    
    return json.loads(result.stdout)

def main():
    if len(sys.argv) != 2:
        print("Usage: python kafka_health.py <bastion-host>")
        sys.exit(1)
    
    bastion = sys.argv[1]
    
    try:
        analysis = run_kafka_health_check(bastion)
        
        print(f"Kafka Health Report for {bastion}")
        print(f"Health Score: {analysis['health_score']}/100")
        print(f"Total Findings: {len(analysis['findings'])}")
        
        critical = [f for f in analysis['findings'] if f['severity'] == 'critical']
        if critical:
            print(f"🚨 {len(critical)} Critical Issues:")
            for finding in critical:
                print(f"  - {finding['title']}")
            sys.exit(1)
        else:
            print("✅ No critical issues found")
            
    except subprocess.CalledProcessError as e:
        print(f"Error running KafkaPilot: {e}")
        sys.exit(1)

if __name__ == '__main__':
    main()
```

---

**Need more examples?** Check our [examples](examples.html) for practical usage patterns.