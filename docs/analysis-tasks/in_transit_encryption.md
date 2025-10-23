---
layout: page
title: In-Transit Encryption Configuration Check
permalink: /analysis-tasks/in_transit_encryption/
---

**Task ID:** `in_transit_encryption`  
**Category:** configuration

## Description

Verifies that any form of in-transit encryption is configured for Kafka brokers to ensure secure communication

## How to Run

Execute this specific analysis task on your Kafka cluster snapshot:

```bash
# Run the task
kcpilot task test in_transit_encryption <snapshot-path>

# Example
kcpilot task test in_transit_encryption ./kafka-scan-2024-01-15

# With debug logging
RUST_LOG=kcpilot=debug kcpilot task test in_transit_encryption <snapshot-path>
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
analysis_tasks/configuration/general/in_transit_encryption.yaml
```

## Related Tasks

See also these related security tasks:
- [authentication_authorization](../authentication_authorization)
- [in_transit_encryption](../in_transit_encryption)



