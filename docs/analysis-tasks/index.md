---
layout: page
title: Analysis Tasks
permalink: /analysis-tasks/
---

# Analysis Tasks

KafkaPilot's analysis tasks are AI-powered diagnostic checks that examine your Kafka cluster's configuration, performance, and health. Each task is defined as a YAML file containing prompts and rules that guide the AI analysis engine to identify specific issues and provide remediation guidance.

## Overview

Analysis tasks leverage Large Language Models (LLMs) to intelligently analyze collected cluster data. Unlike static rule-based checks, these tasks can understand context, identify patterns, and provide nuanced recommendations based on your specific cluster configuration.

### Key Features

- **AI-Powered Analysis**: Uses OpenAI or compatible LLMs to analyze complex cluster states
- **Configurable Severity**: Automatically maps findings to critical, warning, or info levels
- **Data Filtering**: Each task specifies which data types it needs (logs, configs, metrics, admin)
- **Actionable Remediation**: Provides specific steps to resolve identified issues

## How to Use Analysis Tasks

### List Available Tasks

To see all available analysis tasks with their descriptions:

```bash
# List all tasks
kafkapilot task list

# List with detailed information
kafkapilot task list --detailed
```

### Execute a Single Task

To run a specific analysis task on collected scan data:

```bash
# Test a single task
kafkapilot task test <task-id> <snapshot-path>

# Example: Test JVM heap configuration
kafkapilot task test jvm_heap_memory_ratio ./scan-2024-01-15

# With debug logging
RUST_LOG=kafkapilot=debug kafkapilot task test <task-id> <snapshot-path>
```

### Run Full Analysis

To execute all analysis tasks on a snapshot:

```bash
# Analyze with terminal and markdown reports
kafkapilot analyze <snapshot-path> --report terminal,markdown

# Example
kafkapilot analyze ./scan-2024-01-15 --report terminal,markdown --output analysis-report.md
```

## Creating Custom Tasks

Analysis tasks are YAML files stored in the `analysis_tasks/` directory. Each task includes:

1. **Metadata**: ID, name, description, and category
2. **Prompt**: The analysis instruction sent to the LLM
3. **Data Selection**: Which data types to include via `include_data`
4. **Severity Mapping**: Keywords that determine finding severity levels

### Task Template

```yaml
id: your_task_id
name: Task Display Name
description: Brief description of what this task checks
category: configuration|performance|security

prompt: |
  Your analysis prompt here with placeholders:
  Configuration: {config}
  Logs: {logs}
  Metrics: {metrics}
  
  Analysis instructions...

include_data:
  - config
  - logs
  - metrics
  - admin

severity_keywords:
  critical:
    - "data loss"
    - "cluster down"
  warning:
    - "performance degraded"
    - "misconfiguration"
  info:
    - "recommendation"
    - "optimization"
```

## Available Analysis Tasks

Below is a comprehensive list of all available analysis tasks, organized by category:

### Configuration - General

- **[authentication_authorization](./authentication_authorization)** - Verifies authentication and authorization configuration
- **[broker_count_ha](./broker_count_ha)** - Analyzes broker count for high availability
- **[in_transit_encryption](./in_transit_encryption)** - Checks for in-transit encryption configuration
- **[isr_replication_margin](./isr_replication_margin)** - Validates ISR replication settings
- **[jvm_heap_memory_ratio](./jvm_heap_memory_ratio)** - Analyzes JVM heap memory ratio configuration
- **[jvm_heap_preallocation](./jvm_heap_preallocation)** - Checks JVM heap preallocation settings
- **[jvm_heap_size_limit](./jvm_heap_size_limit)** - Validates JVM heap size limits
- **[minimum_cpu_cores](./minimum_cpu_cores)** - Verifies minimum CPU core requirements
- **[multiple_log_dirs](./multiple_log_dirs)** - Checks for multiple log directory configuration
- **[rack_awareness](./rack_awareness)** - Validates rack awareness configuration
- **[recent_log_errors](./recent_log_errors)** - Analyzes recent log errors and warnings
- **[separate_listeners](./separate_listeners)** - Checks for separate listener configuration
- **[thread_configuration](./thread_configuration)** - Analyzes thread pool configuration

### Configuration - KRaft

- **[kraft_controller_ha_check](./kraft_controller_ha_check)** - Validates KRaft controller high availability

### Configuration - ZooKeeper

- **[zookeeper_ha_check](./zookeeper_ha_check)** - Checks ZooKeeper ensemble high availability
- **[zookeeper_heap_memory](./zookeeper_heap_memory)** - Analyzes ZooKeeper heap memory configuration
- **[zookeeper_heap_preallocation](./zookeeper_heap_preallocation)** - Validates ZooKeeper heap preallocation

## Environment Configuration

To use AI-powered analysis tasks, you need to configure your LLM API key:

```bash
# OpenAI API
export OPENAI_API_KEY=your_openai_api_key_here

# Alternative LLM API
export LLM_API_KEY=your_alternative_llm_api_key

# Enable debug logging
export LLM_DEBUG=true
```

## Troubleshooting

### Common Issues

1. **No LLM API Key**: Tasks will fail without a configured API key
2. **Timeout Issues**: Increase timeout with `--llm-timeout <seconds>`
3. **Data Not Found**: Ensure snapshot contains required data types
4. **Task Not Found**: Verify task ID matches file name in `analysis_tasks/`

### Debug Mode

Enable detailed logging to troubleshoot task execution:

```bash
# Debug specific task
RUST_LOG=kafkapilot=debug kafkapilot task test <task-id> <snapshot>

# Debug LLM interactions
kafkapilot analyze <snapshot> --llmdbg
```

## Next Steps

- Review individual [task documentation](./authentication_authorization) for specific checks
- Learn about [creating custom tasks](/how-to#custom-analysis-tasks)
- See [examples](/examples#analysis-tasks) of task execution