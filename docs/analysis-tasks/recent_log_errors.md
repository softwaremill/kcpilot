---
layout: page
title: Recent Log Error Detection
permalink: /analysis-tasks/recent_log_errors/
---

**Task ID:** `recent_log_errors`  
**Category:** configuration

## Description

Analyzes logs from the last 24 hours to detect error messages indicating broker stability or availability issues

## How to Run

Execute this specific analysis task on your Kafka cluster snapshot:

```bash
# Run the task
kcpilot task test recent_log_errors <snapshot-path>

# Example
kcpilot task test recent_log_errors ./kafka-scan-2024-01-15

# With debug logging
RUST_LOG=kcpilot=debug kcpilot task test recent_log_errors <snapshot-path>
```

## Data Requirements

This task analyzes the following types of data from your cluster snapshot:



## Severity Levels

This task can identify issues at the following severity levels:

- **CRITICAL**: Severe issues requiring immediate attention
- **WARNING**: Problems that should be addressed soon  
- **INFO**: Recommendations and optimizations

The severity is determined based on specific patterns and thresholds identified in the cluster data.

## Integration with Full Analysis

This task is automatically included when running a complete cluster analysis:

```bash
kcpilot analyze <snapshot-path> --report terminal,markdown
```

## Task Configuration

The task is defined in the YAML file at:
```
analysis_tasks/configuration/general/recent_log_errors.yaml
```

## Related Tasks

- [View all tasks](../)



