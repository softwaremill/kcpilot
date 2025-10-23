---
layout: page
title: Zookeeper Heap Memory Preallocation Check
permalink: /analysis-tasks/zookeeper_heap_preallocation/
---

**Task ID:** `zookeeper_heap_preallocation`  
**Category:** configuration

## Description

Checks if Zookeeper JVM heap memory is preallocated (Xms equals Xmx) to prevent fragmentation and performance issues

## How to Run

Execute this specific analysis task on your Kafka cluster snapshot:

```bash
# Run the task
kcpilot task test zookeeper_heap_preallocation <snapshot-path>

# Example
kcpilot task test zookeeper_heap_preallocation ./kafka-scan-2024-01-15

# With debug logging
RUST_LOG=kcpilot=debug kcpilot task test zookeeper_heap_preallocation <snapshot-path>
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
analysis_tasks/configuration/zookeeper/zookeeper_heap_preallocation.yaml
```

## Related Tasks

See also these related ZooKeeper tasks:
- [zookeeper_ha_check](../zookeeper_ha_check)
- [zookeeper_heap_memory](../zookeeper_heap_memory)
- [zookeeper_heap_preallocation](../zookeeper_heap_preallocation)


