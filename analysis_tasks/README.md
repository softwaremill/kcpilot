# AI-Powered Analysis Tasks

This directory contains YAML task definitions for KCPilot's AI-powered analysis system. Each YAML file defines a specific analysis task that will be executed by the AI.

## 🚀 Quick Start

1. **View available tasks:**
   ```bash
   kcpilot task list
   ```

2. **Test a specific task:**
   ```bash
   kcpilot task test <task_id> <snapshot_path>
   ```

3. **Create a new task:**
   ```bash
   kcpilot task new my_custom_check --name "My Custom Check"
   ```

## 📝 Task Structure

Each task is defined in a YAML file with the following structure:

```yaml
id: unique_task_id           # Unique identifier
name: Human Readable Name    # Display name
description: What this checks # Detailed description
category: performance        # Category (performance/security/availability/etc.)

# The prompt sent to the AI
prompt: |
  Your analysis prompt here...
  
  Available data placeholders:
  {logs}     - Log data
  {config}   - Configuration files
  {metrics}  - Metrics data
  {admin}    - Admin/topic metadata
  {topics}   - Topic information
  
  Instructions for the AI...

# Optional: specify which data to include
include_data:
  - logs
  - config

# Keywords that map to severity levels
severity_keywords:
  "critical error": "critical"
  "warning": "medium"
  
default_severity: medium
enabled: true
```

## 🎯 Available Tasks (All File-Based)

The system is now 100% file-based with NO hardcoded tasks. All tasks are defined in YAML files (18 total).

### Core Health & Overview
- **cluster_overview.yaml** - Quick high-level cluster health scan
- **root_cause_analysis.yaml** - Intelligent root cause detection by correlating all data

### Critical Availability (4 tasks)
- **offline_partitions.yaml** - Detect partitions with no leader
- **under_replicated_partitions.yaml** - Find partitions with insufficient replicas
- **isr_shrinkage.yaml** - Detect dangerously small ISR sets (ISR < 2)
- **network_issues.yaml** - Network connectivity and communication problems

### Performance & Optimization (4 tasks)
- **performance_analysis.yaml** - Comprehensive performance analysis
- **jvm_performance.yaml** - JVM and garbage collection performance
- **leader_imbalance.yaml** - Leader distribution and load balancing
- **client_consumer_health.yaml** - Consumer groups, lag, and client health

### Configuration (3 tasks)
- **config_optimization.yaml** - Comprehensive configuration review (30+ settings)
- **topic_analysis.yaml** - Per-topic configuration and usage analysis
- **broker_id_issues.yaml** - Critical broker ID conflicts detection

### Capacity & Planning (2 tasks)
- **disk_usage.yaml** - Current disk usage and retention analysis
- **capacity_planning.yaml** - Growth projections and future capacity needs

### Security (1 task)
- **security_audit.yaml** - Security configuration and vulnerability audit

### Log Analysis (2 tasks)
- **log_quick_scan.yaml** - Quick log error scan
- **log_detailed_errors.yaml** - Detailed error pattern and rate analysis

## ✨ Creating Custom Tasks

### Simple Example

Create a file `analysis_tasks/my_check.yaml`:

```yaml
id: my_check
name: My Custom Check
description: Check for specific issues
category: cluster_hygiene

prompt: |
  Analyze this Kafka cluster for [specific issues]:
  
  {admin}
  {logs}
  
  Look for [specific patterns] and provide findings as JSON.

enabled: true
```

### Advanced Example with Data Filtering

```yaml
id: advanced_check
name: Advanced Analysis
description: Complex multi-step analysis
category: performance

prompt: |
  Perform advanced analysis:
  
  Configuration: {config}
  Metrics: {metrics}
  
  Step 1: Analyze X
  Step 2: Check Y
  Step 3: Calculate Z
  
  Provide detailed JSON findings with severity levels.

include_data:
  - config
  - metrics

severity_keywords:
  "severe": "critical"
  "problem": "high"
  "issue": "medium"
  "notice": "low"

default_severity: medium
enabled: true
```

## 🏷️ Categories

- `cluster_hygiene` - General health and maintenance
- `performance` - Performance optimization
- `security` - Security issues
- `availability` - High availability concerns
- `configuration` - Configuration issues
- `capacity` - Capacity planning
- `client` - Client-related issues

## 🔍 Testing Tasks

Test a task against a snapshot:

```bash
# Test with debug output
kcpilot task test security_audit snapshot.json --debug

# Test against a scan directory
kcpilot task test disk_usage /path/to/scan/output/
```

## 💡 Tips

1. **Keep prompts focused** - Each task should check for one type of issue
2. **Use placeholders** - Use `{data_type}` to inject collected data
3. **Request JSON output** - Always ask for JSON format with a `findings` array
4. **Set appropriate severity** - Configure keywords that indicate issue severity
5. **Test iteratively** - Use `task test` to refine your prompts

## 🔧 Environment Setup

Make sure to set your OpenAI API key:

```bash
export OPENAI_API_KEY=your-api-key
```

Or create a `.env` file:

```
OPENAI_API_KEY=your-api-key
```

## 📊 Data Available in Prompts

- `{admin}` - Broker metadata, topics, partitions, consumer groups
- `{logs}` - Kafka broker logs, error logs
- `{config}` - server.properties and other config files
- `{metrics}` - JMX metrics, performance metrics
- `{topics}` - Topic-specific information
- Custom collectors - Any custom data collected during scan

## 🚨 Troubleshooting

If a task isn't working:

1. Check the YAML syntax is valid
2. Ensure the task ID is unique
3. Verify data placeholders match available data
4. Test with `--debug` flag for detailed output
5. Check the AI response format in debug logs
