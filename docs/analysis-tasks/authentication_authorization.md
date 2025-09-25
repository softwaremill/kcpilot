---
layout: page
title: Authentication and Authorization Configuration Check
permalink: /analysis-tasks/authentication_authorization/
---

**Task ID:** `authentication_authorization`  
**Category:** configuration

## Description

Verifies that authentication and authorization are enabled and anonymous access is blocked to prevent unauthorized access

## How to Run

Execute this specific analysis task on your Kafka cluster snapshot:

```bash
# Run the task
kafkapilot task test authentication_authorization <snapshot-path>

# Example
kafkapilot task test authentication_authorization ./kafka-scan-2024-01-15

# With debug logging
RUST_LOG=kafkapilot=debug kafkapilot task test authentication_authorization <snapshot-path>
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
analysis_tasks/configuration/general/authentication_authorization.yaml
```

## Related Tasks

See also these related security tasks:
- [authentication_authorization](../authentication_authorization)
- [in_transit_encryption](../in_transit_encryption)

## Need Help?

- Check the [main analysis tasks documentation](../)
- See more [examples](/examples#analysis-tasks)

