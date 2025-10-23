# KCPilot Design Document

## Executive Summary

KCPilot is a CLI-first Kafka health diagnostics tool that automatically collects cluster signals, identifies issues, and provides actionable remediation guidance. The tool serves dual purposes: providing immediate value to users while serving as a lead generation channel for Kafka consulting services.

### Core Value Proposition
- **One-command health assessment** with zero configuration
- **Plain-language explanations** of complex Kafka issues
- **Actionable remediation scripts** with safe defaults
- **Enterprise-ready** with air-gapped mode and sensitive data redaction

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                        CLI Interface                        │
│                     (clap-based commands)                   │
└─────────────────────────────────────────────────────────────┘
                               │
┌─────────────────────────────────────────────────────────────┐
│                         Core Engine                         │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │
│  │   Snapshot   │  │    Rules     │  │   Reporting  │     │
│  │    Engine    │  │    Engine    │  │    Engine    │     │
│  └──────────────┘  └──────────────┘  └──────────────┘     │
└─────────────────────────────────────────────────────────────┘
                               │
┌─────────────────────────────────────────────────────────────┐
│                         Collectors                          │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐  │
│  │  Admin   │  │   JMX/   │  │   Logs   │  │  Cloud   │  │
│  │  Client  │  │   Prom   │  │          │  │ Metadata │  │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## Current Implementation Status

### Phase 1: MVP (Completed)
- ✅ Basic CLI structure with clap
- ✅ Command definitions (scan, watch, analyze, fix)
- ✅ SSH-based scan functionality for remote clusters
- ✅ Comprehensive data collection (configs, logs, metrics, system info)
- ✅ Output to timestamped directories
- ✅ Collection summary reports
- ✅ Folder-based data storage (timestamped directories)
- ⏳ Analysis engine (in progress)
- ⏳ Automated remediation

## Command Interface

### Implemented Commands

```bash
# Scan locally when running directly on bastion
kcpilot scan

# Scan remotely via SSH bastion from ~/.ssh/config
kcpilot scan --bastion kafka-poligon

# Scan with custom output directory
kcpilot scan --output /path/to/output

# Analyze existing scan data from folder
kcpilot analyze ./test-local-scan \
  --report terminal,html,markdown

# Watch cluster continuously (planned)
kcpilot watch --interval 60 --alert

# Generate remediation scripts from scan data
kcpilot fix ./test-local-scan --dry-run
```

## Project Structure

```
kcpilot/
├── src/
│   ├── main.rs              # Entry point
│   ├── lib.rs               # Library exports
│   ├── cli/                 # Command-line interface
│   │   ├── mod.rs
│   │   └── commands.rs      # Command definitions
│   ├── scan/                # SSH-based scanning
│   │   ├── mod.rs           # Scanner orchestration
│   │   └── collector.rs     # Bastion/broker collectors
│   ├── collectors/          # Data collectors (legacy)
│   │   ├── mod.rs
│   │   ├── admin.rs         # Kafka AdminClient
│   │   └── logs.rs          # Log collection
│   ├── analyzers/           # Analysis engine
│   │   ├── mod.rs
│   │   └── rules.rs         # Rule engine
│   ├── snapshot/            # Snapshot management
│   │   ├── mod.rs
│   │   └── format.rs        # Data structures
│   └── report/              # Report generation
│       ├── mod.rs
│       └── terminal.rs      # Terminal output
```

## Data Collection Structure

Scan output is stored in timestamped directories with the following structure:

```
<scan-folder>/
├── scan_metadata.json       # Scan metadata (timestamp, broker count, etc.)
├── COLLECTION_SUMMARY.md    # Human-readable collection summary
├── brokers/                 # Per-broker data
│   └── broker_<id>/
│       ├── broker_info.json    # Broker metadata (id, hostname, datacenter)
│       ├── configs/            # Configuration files
│       │   ├── server.properties
│       │   └── kafka.service
│       ├── data/               # Data directory information
│       │   └── log_dirs.txt
│       ├── logs/               # Log files
│       │   └── journald.log
│       ├── metrics/            # JVM metrics
│       │   └── jstat_gc.txt
│       └── system/             # System information
│           ├── cpu.txt
│           ├── disk.txt
│           ├── memory.txt
│           ├── network.txt
│           ├── java_version.txt
│           └── kafka_process.txt
├── cluster/                 # Cluster-level data
│   └── kafkactl/
│       ├── brokers.txt
│       ├── topics.txt
│       ├── topics_detailed.txt
│       ├── consumer_groups.txt
│       └── broker_<id>_config.txt
└── metrics/                 # Prometheus metrics
    └── kafka_exporter/
        └── prometheus_metrics.txt
```

## Analysis Plan

### Configuration Analysis

#### 1. Broker Configuration Consistency
- **Data Source**: `brokers/*/configs/server.properties`
- **Checks**:
  - Verify all brokers have consistent replication settings
  - Check for mismatched `min.insync.replicas` across brokers
  - Validate `unclean.leader.election.enable` is consistent
  - Ensure `log.retention.hours/bytes` are aligned
  - Check JVM heap settings match across brokers
  - Verify network buffer sizes are optimal
  - Validate compression settings consistency

#### 2. Security Configuration
- **Data Source**: `brokers/*/configs/server.properties`
- **Checks**:
  - Check if SSL/SASL is properly configured
  - Verify ACLs are enabled if security is configured
  - Check for plaintext listeners in production
  - Validate inter-broker protocol security

### Resource Analysis

#### 3. Disk Usage and Health
- **Data Source**: `brokers/*/system/disk.txt`, `brokers/*/data/log_dirs.txt`
- **Checks**:
  - Alert on disk usage > 80%
  - Check for imbalanced disk usage across brokers
  - Verify log directories are accessible
  - Calculate data growth rate if historical data available
  - Check for adequate free space for retention periods

#### 4. Memory and JVM Health
- **Data Source**: `brokers/*/system/memory.txt`, `brokers/*/metrics/jstat_gc.txt`
- **Checks**:
  - Analyze GC frequency and pause times
  - Check heap usage patterns
  - Verify heap size is appropriate for workload
  - Check for memory pressure indicators
  - Validate JVM version consistency

#### 5. CPU and Load Analysis
- **Data Source**: `brokers/*/system/cpu.txt`, `brokers/*/system/uptime.txt`
- **Checks**:
  - Identify high load averages
  - Check CPU utilization patterns
  - Verify adequate CPU resources
  - Check for load imbalances

### Cluster Health Analysis

#### 6. Replication Health
- **Data Source**: `cluster/kafkactl/topics_detailed.txt`
- **Checks**:
  - Identify under-replicated partitions
  - Check for offline partitions
  - Verify ISR (In-Sync Replicas) health
  - Detect partitions with ISR < min.insync.replicas
  - Check for uneven partition distribution

#### 7. Topic Configuration Issues
- **Data Source**: `cluster/kafkactl/topics_detailed.txt`, `cluster/kafkactl/topics.txt`
- **Checks**:
  - Identify topics with replication factor = 1
  - Check for topics with excessive partitions
  - Verify min.insync.replicas settings
  - Detect empty/unused topics
  - Check for unbalanced partition leadership

#### 8. Consumer Group Health
- **Data Source**: `cluster/kafkactl/consumer_groups.txt`
- **Checks**:
  - Identify consumer lag issues
  - Check for inactive consumer groups
  - Verify consumer group rebalancing health
  - Detect stuck or slow consumers

### Log Analysis

#### 9. Error Pattern Detection
- **Data Source**: `brokers/*/logs/journald.log`
- **Checks**:
  - Search for OutOfMemoryError
  - Detect connection errors and timeouts
  - Identify authentication/authorization failures
  - Check for disk I/O errors
  - Find replication errors
  - Detect Zookeeper connection issues

#### 10. Performance Issues
- **Data Source**: `brokers/*/logs/journald.log`
- **Checks**:
  - Identify slow request warnings
  - Check for request queue saturation
  - Detect network thread pool exhaustion
  - Find I/O thread pool issues

### Network Analysis

#### 11. Network Health
- **Data Source**: `brokers/*/system/network.txt`
- **Checks**:
  - Verify all required ports are listening
  - Check for connection limit issues
  - Identify network bottlenecks
  - Verify inter-broker connectivity

### Metrics Analysis

#### 12. Prometheus Metrics
- **Data Source**: `metrics/kafka_exporter/prometheus_metrics.txt`
- **Checks**:
  - Analyze request rates and latencies
  - Check producer/consumer metrics
  - Verify broker-level metrics
  - Identify metric anomalies

## Analysis Output

The analysis engine will produce:

### Severity Levels
- **CRITICAL**: Immediate action required (data loss risk, cluster down)
- **WARNING**: Action needed soon (performance degradation, approaching limits)
- **INFO**: Best practice recommendations
- **OK**: Component functioning normally

### Report Formats

#### Terminal Report
```
╔════════════════════════════════════════════╗
║        KCPilot Analysis Report             ║
╚════════════════════════════════════════════╝

📊 Cluster Overview
├─ Brokers: 6 (6 healthy)
├─ Topics: 3
├─ Partitions: 18
└─ Consumer Groups: 0

🔴 CRITICAL Issues (2)
├─ [DISK-001] Broker 11: Disk usage at 92%
│  └─ Immediate action required to prevent outage
└─ [REPL-003] Topic 'events': 3 under-replicated partitions
   └─ Data loss risk if another broker fails

🟡 WARNING Issues (3)
├─ [CONF-002] Inconsistent min.insync.replicas
├─ [JVM-001] Broker 13: High GC pause times (>500ms)
└─ [PART-001] Unbalanced partition distribution

🟢 OK Components (15)
└─ All other checks passed

📝 Remediation Available
Run: kcpilot fix ./test-local-scan --issue DISK-001
```

#### HTML Report
- Interactive dashboard with drill-down capabilities
- Sortable/filterable issue list
- Visual charts for resource usage
- Historical trend analysis (if multiple scans)

#### Markdown Report
- GitHub-compatible markdown format
- Suitable for documentation and ticketing systems
- Includes remediation scripts inline

## LLM-Enhanced Analysis

Integrating LLM capabilities unlocks advanced analysis that goes far beyond rule-based checks:

### 🧠 Intelligent Log Analysis

#### Natural Language Log Understanding
- **What it does**: Interprets complex error messages and stack traces in context
- **Example**: "OutOfMemoryError in broker 11" → "Broker 11 is experiencing memory pressure due to a combination of high producer throughput (seen in metrics) and insufficient heap allocation. The GC logs show frequent full GCs starting 2 hours before the OOM."
- **Data sources**: `brokers/*/logs/*.log`, `brokers/*/metrics/jstat_gc.txt`

#### Multi-Log Correlation
- **What it does**: Finds related issues across different log files and brokers
- **Example**: Correlates a Zookeeper timeout in broker 11 with network errors in broker 12 and a controller election in broker 13
- **Benefits**: Identifies cascade failures and root causes that span multiple components

#### Anomaly Detection in Logs
- **What it does**: Identifies unusual patterns without predefined rules
- **Example**: Detects a new error pattern that started appearing after a specific time
- **Benefits**: Catches zero-day issues and emerging problems

### 🔍 Advanced Root Cause Analysis

#### Cross-Component Correlation
- **What it does**: Connects symptoms across configs, logs, metrics, and system stats
- **Example Input**: "High consumer lag on topic 'orders'"
- **LLM Analysis**: 
  - Checks consumer group logs for rebalancing issues
  - Correlates with broker GC pauses
  - Identifies disk I/O bottleneck on partition leaders
  - Links to recent config change in `fetch.min.bytes`
- **Output**: "Root cause: Disk I/O saturation on broker 13 (hosting 60% of 'orders' partition leaders) combined with suboptimal consumer fetch settings"

#### Historical Pattern Analysis
- **What it does**: Compares current issues with historical patterns from previous scans
- **Example**: "This disk usage growth pattern matches the pre-incident scan from 2 weeks ago"
- **Benefits**: Predictive failure detection

### 📊 Intelligent Performance Analysis

#### Workload Characterization
- **What it does**: Analyzes metrics and logs to understand workload patterns
- **LLM Output**: 
  - "Write-heavy workload with 80% producer traffic"
  - "Bursty pattern with 10x spikes every hour on the hour"
  - "Large message sizes (avg 500KB) causing network bottlenecks"

#### Performance Bottleneck Detection
- **What it does**: Identifies complex performance issues from multiple signals
- **Example**: Combines JVM metrics, disk I/O, network stats, and request latencies to identify the primary bottleneck
- **Output**: Ranked list of bottlenecks with confidence scores

### 🛠️ Intelligent Remediation

#### Context-Aware Fix Generation
- **What it does**: Generates custom remediation scripts based on specific environment
- **Example**: 
  ```bash
  # LLM-generated fix for under-replicated partitions on broker 11
  # Context: Broker 11 has high disk usage (92%) and slow I/O
  
  # Step 1: Temporarily increase replication throttle
  kafka-configs --alter --add-config follower.replication.throttled.rate=50000000 --entity-type brokers --entity-name 11
  
  # Step 2: Move leadership away from broker 11
  # (generated partition reassignment JSON based on current topology)
  
  # Step 3: Clean up old log segments
  # (custom script based on actual retention needs)
  ```

#### Risk Assessment
- **What it does**: Evaluates the risk of proposed remediations
- **Output**: "This fix has a 15% risk of temporary producer latency increase during execution"

### 🔮 Predictive Analysis

#### Capacity Planning
- **What it does**: Predicts future resource needs based on growth patterns
- **Example**: "At current growth rate, disk space will be exhausted in 14 days on brokers 11, 13"
- **Data used**: Historical metrics, log segment growth, topic creation patterns

#### Failure Prediction
- **What it does**: Identifies patterns that historically precede failures
- **Example**: "Current JVM heap pattern matches pre-OOM conditions in 85% of historical cases"

### 💡 Configuration Optimization

#### Workload-Specific Tuning
- **What it does**: Suggests configuration changes based on actual workload
- **Example Recommendations**:
  - "Increase `num.network.threads` from 8 to 16 based on connection queue depth"
  - "Adjust `replica.fetch.max.bytes` to match your largest message size (currently seeing rejections)"
  - "Enable compression (snappy) for topic 'events' - would save 60% storage"

#### Security Audit
- **What it does**: Identifies security misconfigurations and suggests fixes
- **Example**: "SASL is configured but `allow.everyone.if.no.acl.found=true` negates security benefits"

### 🎯 Natural Language Interface

#### Interactive Troubleshooting
```bash
$ kcpilot analyze ./test-local-scan --interactive

KCPilot> "Why is my consumer group lagging?"
> Analyzing consumer group data...
> I found that your consumer group 'payment-processor' is lagging because:
> 1. Broker 13 (partition leader for 70% of partitions) has 95% CPU usage
> 2. GC pauses on broker 13 are averaging 800ms every 30 seconds
> 3. Your consumers are timing out during these pauses
>
> Recommended fix: Increase heap size on broker 13 or rebalance partition leadership

KCPilot> "Show me the fix"
> Generating remediation script...
```

#### Plain English Reports
- **What it does**: Converts technical findings into executive-friendly summaries
- **Example**: "Your Kafka cluster is healthy but approaching capacity limits. Three brokers need disk space expansion within the next two weeks to maintain current growth."

### 📈 Advanced Metrics Analysis

#### Metric Anomaly Detection
- **What it does**: Identifies unusual metric patterns without thresholds
- **Example**: "Request latency shows unusual bimodal distribution starting at 14:30"

#### Correlation Analysis
- **What it does**: Finds hidden correlations between metrics
- **Example**: "Consumer lag correlates 0.89 with GC pause frequency on partition leaders"

### 🔄 Continuous Learning

#### Pattern Library Building
- **What it does**: Learns from each analysis to improve future diagnostics
- **Example**: Builds a library of environment-specific patterns and their solutions

#### Feedback Loop
- **What it does**: Incorporates user feedback on remediation success
- **Example**: "This fix resolved the issue" → Increases confidence for similar future cases

## LLM Integration Architecture

```
┌─────────────────────────────────────────────────────────┐
│                   KCPilot Analyzer                      │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐ │
│  │ Rule Engine  │  │ LLM Analyzer │  │   Reporter   │ │
│  │              │  │              │  │              │ │
│  │ - Threshold  │  │ - Log Intel  │  │ - Terminal   │ │
│  │ - Patterns   │  │ - Root Cause │  │ - HTML       │ │
│  │ - Best       │  │ - Predictive │  │ - Markdown   │ │
│  │   Practices  │  │ - Natural    │  │ - API        │ │
│  │              │  │   Language   │  │              │ │
│  └──────────────┘  └──────────────┘  └──────────────┘ │
│         │                 │                  │         │
│         └─────────────────┴──────────────────┘         │
│                           │                            │
│                    ┌──────────────┐                    │
│                    │ Data Loader  │                    │
│                    │              │                    │
│                    │ Reads scan   │                    │
│                    │ folder data  │                    │
│                    └──────────────┘                    │
└─────────────────────────────────────────────────────────┘
```

### LLM Provider Options

1. **OpenAI API** (GPT-4/GPT-3.5)
   - Best for: Cloud deployments, advanced reasoning
   - Consideration: Requires API key, data leaves premises

2. **Local LLMs** (Ollama/llama.cpp)
   - Best for: Air-gapped environments, data privacy
   - Models: Mixtral, Llama 3, CodeLlama
   - Consideration: Requires GPU for optimal performance

3. **Hybrid Approach**
   - Sensitive data analysis with local LLM
   - General analysis with cloud LLM
   - User choice via configuration

### Implementation Priority

1. **Phase 1**: Log intelligence and natural language reporting
2. **Phase 2**: Root cause analysis and correlation
3. **Phase 3**: Predictive analysis and capacity planning
4. **Phase 4**: Interactive troubleshooting interface
5. **Phase 5**: Continuous learning and pattern library
