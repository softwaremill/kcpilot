---
layout: page
title: ISR vs Replication Factor Margin Check
permalink: /analysis-tasks/isr_replication_margin/
---

**Task ID:** `isr_replication_margin`  
**Category:** configuration

## Description

Ensures replication factor provides adequate margin over min.insync.replicas for high availability

## How to Run

Execute this specific analysis task on your Kafka cluster snapshot:

```bash
# Run the task
kafkapilot task test isr_replication_margin <snapshot-path>

# Example
kafkapilot task test isr_replication_margin ./kafka-scan-2024-01-15

# With debug logging
RUST_LOG=kafkapilot=debug kafkapilot task test isr_replication_margin <snapshot-path>
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
analysis_tasks/configuration/general/isr_replication_margin.yaml
```

## Related Tasks

See also these related high availability tasks:
- [broker_count_ha](../broker_count_ha)
- [isr_replication_margin](../isr_replication_margin)
- [rack_awareness](../rack_awareness)



