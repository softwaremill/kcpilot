---
layout: page
title: Thread Configuration Validation
permalink: /analysis-tasks/thread_configuration/
---

# Thread Configuration Validation

**Task ID:** `thread_configuration`  
**Category:** configuration

## Description

Validates network, I/O, and replication thread settings to prevent performance issues or broker failures

## How to Run

Execute this specific analysis task on your Kafka cluster snapshot:

```bash
# Run the task
kafkapilot task test thread_configuration <snapshot-path>

# Example
kafkapilot task test thread_configuration ./kafka-scan-2024-01-15

# With debug logging
RUST_LOG=kafkapilot=debug kafkapilot task test thread_configuration <snapshot-path>
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
analysis_tasks/configuration/general/thread_configuration.yaml
```

## Related Tasks

- [View all tasks](../)

## Need Help?

- Check the [main analysis tasks documentation](../)
- Learn about [creating custom tasks](/how-to#custom-analysis-tasks)
- See more [examples](/examples#analysis-tasks)
- Get [support](/support)
