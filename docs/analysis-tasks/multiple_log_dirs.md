---
layout: page
title: Multiple Log Directories Configuration Check
permalink: /analysis-tasks/multiple_log_dirs/
---

**Task ID:** `multiple_log_dirs`  
**Category:** configuration

## Description

Detects brokers with multiple log.dirs configured which can complicate rebalancing operations

## How to Run

Execute this specific analysis task on your Kafka cluster snapshot:

```bash
# Run the task
kafkapilot task test multiple_log_dirs <snapshot-path>

# Example
kafkapilot task test multiple_log_dirs ./kafka-scan-2024-01-15

# With debug logging
RUST_LOG=kafkapilot=debug kafkapilot task test multiple_log_dirs <snapshot-path>
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
kafkapilot analyze <snapshot-path> --report terminal,markdown
```

## Task Configuration

The task is defined in the YAML file at:
```
analysis_tasks/configuration/general/multiple_log_dirs.yaml
```

## Related Tasks

- [View all tasks](../)

## Need Help?

- Check the [main analysis tasks documentation](../)
- Learn about [creating custom tasks](/how-to#custom-analysis-tasks)
- See more [examples](/examples#analysis-tasks)
- Get [support](/support)
