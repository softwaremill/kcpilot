---
layout: page
title: Separate Client and Cluster Listeners Check
permalink: /analysis-tasks/separate_listeners/
---

**Task ID:** `separate_listeners`  
**Category:** configuration

## Description

Verifies that brokers have separate listeners configured for client and inter-broker communication for optimal performance and security

## How to Run

Execute this specific analysis task on your Kafka cluster snapshot:

```bash
# Run the task
kafkapilot task test separate_listeners <snapshot-path>

# Example
kafkapilot task test separate_listeners ./kafka-scan-2024-01-15

# With debug logging
RUST_LOG=kafkapilot=debug kafkapilot task test separate_listeners <snapshot-path>
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
analysis_tasks/configuration/general/separate_listeners.yaml
```

## Related Tasks

- [View all tasks](../)

## Need Help?

- Check the [main analysis tasks documentation](../)
- Learn about [creating custom tasks](/how-to#custom-analysis-tasks)
- See more [examples](/examples#analysis-tasks)

