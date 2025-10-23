---
layout: page
title: API Reference
permalink: /api/
---

Complete command-line interface documentation for KCPilot.

## Global Options

All KCPilot commands support these global options:

```bash
kcpilot [GLOBAL_OPTIONS] <COMMAND> [COMMAND_OPTIONS]
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
| `KCPILOT_OUTPUT_DIR` | Default output directory | `./` |

---

## Commands

### `scan` - Data Collection

Collect comprehensive data from Kafka cluster.

```bash
kcpilot scan [OPTIONS]
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
kcpilot scan --broker kafka-broker-1.internal:9092

# Remote scan via SSH bastion
kcpilot scan --bastion kafka-prod --broker kafka-broker-1.internal:9092

# Custom output directory
kcpilot scan --bastion kafka-prod --broker kafka-broker-1.internal:9092 --output health-check-2024-01-15

# Parallel collection with timeout
kcpilot scan --bastion kafka-prod --broker kafka-broker-1.internal:9092 --parallel 3 --timeout 600

# Dry run to see what would be collected
kcpilot scan --bastion kafka-prod --broker kafka-broker-1.internal:9092 --dry-run
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
kcpilot analyze [OPTIONS] <SCAN_DIRECTORY>
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
kcpilot analyze ./kafka-scan-data --report terminal
```

**Markdown** - Formatted report suitable for documentation:
```bash
kcpilot analyze ./kafka-scan-data --report markdown --output report.md
```

**JSON** - Structured data for automation and integration:
```bash
kcpilot analyze ./kafka-scan-data --report json --output analysis.json
```

**Multiple formats** - Generate all formats:
```bash
kcpilot analyze ./kafka-scan-data --report terminal,markdown,json
```

#### Examples

```bash
# Basic analysis with terminal output
kcpilot analyze ./kafka-scan-20240115

# Generate markdown report
kcpilot analyze ./kafka-scan-20240115 --report markdown

# JSON output for automation
kcpilot analyze ./kafka-scan-20240115 --report json > analysis.json

# Run specific analysis tasks only
kcpilot analyze ./kafka-scan-20240115 --tasks jvm_heap_memory,thread_configuration

# Extended timeout for large clusters
kcpilot analyze ./kafka-scan-20240115 --llm-timeout 120
```

#### JSON Output Schema

```json
{
  "metadata": {
    "scan_directory": "./kafka-scan-20240115",
    "analysis_timestamp": "2024-01-15T14:30:45Z",
    "kcpilot_version": "0.1.0",
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
kcpilot task <SUBCOMMAND> [OPTIONS]
```

#### Subcommands

##### `list` - List Available Tasks

```bash
kcpilot task list [OPTIONS]
```

**Options:**
- `--detailed` - Show full task descriptions and metadata
- `--category <CATEGORY>` - Filter by category (performance, security, configuration, etc.)

**Examples:**
```bash
# List all tasks
kcpilot task list

# Detailed task information
kcpilot task list --detailed

# Performance-related tasks only
kcpilot task list --category performance
```

##### `test` - Execute Single Task

```bash
kcpilot task test <TASK_ID> <SCAN_DIRECTORY> [OPTIONS]
```

**Options:**
- `--output <FORMAT>` - Output format: `terminal`, `json`
- `--llm-timeout <SECONDS>` - LLM request timeout

**Examples:**
```bash
# Test JVM heap memory configuration
kcpilot task test jvm_heap_memory ./kafka-scan-data

# Test with JSON output
kcpilot task test thread_configuration ./kafka-scan-data --output json

# Extended timeout
kcpilot task test comprehensive_analysis ./kafka-scan-data --llm-timeout 180
```

##### `new` - Create Custom Task (Future)

*Planned for future release*

```bash
kcpilot task new --id custom_check --name "Custom Check" --category compliance
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

Display KCPilot version and system information.

```bash
kcpilot info
```
#### Example

```bash
# Basic version information
kcpilot info

```

#### Output

```
KCPilot v0.1.0
Kafka cluster health diagnostics and remediation tool

Authors: SoftwareMill <hello@softwaremill.com>
License: Apache-2.0

```

---