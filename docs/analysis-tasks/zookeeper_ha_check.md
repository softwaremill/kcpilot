---
layout: page
title: Zookeeper High Availability Configuration Check
permalink: /analysis-tasks/zookeeper_ha_check/
---

**Task ID:** `zookeeper_ha_check`  
**Category:** configuration

## Description

Checks if Zookeeper cluster is configured for high availability and avoids split-brain scenarios

## How to Run

Execute this specific analysis task on your Kafka cluster snapshot:

```bash
# Run the task
kafkapilot task test zookeeper_ha_check <snapshot-path>

# Example
kafkapilot task test zookeeper_ha_check ./kafka-scan-2024-01-15

# With debug logging
RUST_LOG=kafkapilot=debug kafkapilot task test zookeeper_ha_check <snapshot-path>
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
analysis_tasks/configuration/zookeeper/zookeeper_ha_check.yaml
```

## Related Tasks

See also these related ZooKeeper tasks:
- [zookeeper_ha_check](../zookeeper_ha_check)
- [zookeeper_heap_memory](../zookeeper_heap_memory)
- [zookeeper_heap_preallocation](../zookeeper_heap_preallocation)

## Need Help?

- Check the [main analysis tasks documentation](../)
- Learn about [creating custom tasks](/how-to#custom-analysis-tasks)
- See more [examples](/examples#analysis-tasks)

