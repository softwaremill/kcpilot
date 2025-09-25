---
layout: page
title: JVM Heap Size Limit Check
permalink: /analysis-tasks/jvm_heap_size_limit/
---

**Task ID:** `jvm_heap_size_limit`  
**Category:** configuration

## Description

Verifies that JVM heap memory settings are not set above 8GB to prevent performance degradation

## How to Run

Execute this specific analysis task on your Kafka cluster snapshot:

```bash
# Run the task
kafkapilot task test jvm_heap_size_limit <snapshot-path>

# Example
kafkapilot task test jvm_heap_size_limit ./kafka-scan-2024-01-15

# With debug logging
RUST_LOG=kafkapilot=debug kafkapilot task test jvm_heap_size_limit <snapshot-path>
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
analysis_tasks/configuration/general/jvm_heap_size_limit.yaml
```

## Related Tasks

See also these related JVM configuration tasks:
- [jvm_heap_memory_ratio](../jvm_heap_memory_ratio)
- [jvm_heap_preallocation](../jvm_heap_preallocation)
- [jvm_heap_size_limit](../jvm_heap_size_limit)

## Need Help?

- Check the [main analysis tasks documentation](../)
- Learn about [creating custom tasks](/how-to#custom-analysis-tasks)
- See more [examples](/examples#analysis-tasks)

