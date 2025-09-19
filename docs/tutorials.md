---
layout: page
title: Tutorials
permalink: /tutorials/
---

# Tutorials

Step-by-step guides for common KafkaPilot use cases and diagnostic workflows.

## Getting Started Tutorials

### 1. First Health Check

Learn to perform your first comprehensive Kafka cluster health assessment.

**What you'll learn:**
- Basic cluster scanning
- Understanding output structure
- Reading collection summaries
- Identifying immediate issues

```bash
# Start with a basic scan
kafkapilot scan --bastion kafka-prod --output first-health-check

# Review the summary
cat first-health-check/COLLECTION_SUMMARY.md

# Check for immediate red flags
grep -i "error\|fail\|critical" first-health-check/COLLECTION_SUMMARY.md
```

**Time required:** 10 minutes  
**Prerequisites:** SSH access to Kafka cluster

[📖 Full Tutorial: First Health Check](tutorials/01-first-health-check.html)

---

### 2. Performance Troubleshooting

Diagnose performance bottlenecks in your Kafka cluster.

**What you'll learn:**
- Collecting performance metrics
- Analyzing JVM heap usage
- Identifying disk I/O issues
- Network connectivity problems

```bash
# Comprehensive performance scan
kafkapilot scan --bastion kafka-prod --output perf-analysis

# Focus on JVM metrics
kafkapilot task test jvm_heap_memory ./perf-analysis
kafkapilot task test jvm_heap_size_limit ./perf-analysis

# Check disk utilization
kafkapilot task test log_dirs_disk_size_uniformity ./perf-analysis
```

**Time required:** 20 minutes  
**Prerequisites:** Understanding of JVM basics

[📖 Full Tutorial: Performance Troubleshooting](tutorials/02-performance-troubleshooting.html)

---

### 3. AI-Powered Analysis

Use AI to automatically identify and resolve cluster issues.

**What you'll learn:**
- Setting up OpenAI API integration
- Running comprehensive AI analysis
- Interpreting AI-generated insights
- Acting on remediation recommendations

```bash
# Set up AI analysis
export OPENAI_API_KEY=your_api_key_here

# Run comprehensive analysis
kafkapilot analyze ./kafka-scan-data --report terminal

# Generate detailed report
kafkapilot analyze ./kafka-scan-data --report markdown,json
```

**Time required:** 15 minutes  
**Prerequisites:** OpenAI API key

[📖 Full Tutorial: AI-Powered Analysis](tutorials/03-ai-analysis.html)

---

## Production Use Cases

### 4. Incident Response Workflow

Rapid diagnostic workflow for production incidents.

**What you'll learn:**
- Emergency data collection
- Priority-based analysis
- Escalation procedures
- Documentation practices

**Scenario:** "Our Kafka cluster is experiencing high latency and some consumers are lagging significantly."

```bash
# Emergency scan with debug logging
RUST_LOG=kafkapilot=debug kafkapilot scan --bastion kafka-prod --output incident-$(date +%Y%m%d-%H%M)

# Quick analysis for immediate issues
kafkapilot task test consumer_lag ./incident-*/
kafkapilot task test broker_connectivity ./incident-*/

# Generate incident report
kafkapilot analyze ./incident-*/ --report markdown > incident-report.md
```

**Time required:** 30 minutes  
**Prerequisites:** Production access, incident response training

[📖 Full Tutorial: Incident Response](tutorials/04-incident-response.html)

---

### 5. Capacity Planning

Analyze cluster resource utilization for capacity planning.

**What you'll learn:**
- Resource utilization analysis
- Growth trend identification
- Scaling recommendations
- Cost optimization opportunities

```bash
# Collect comprehensive metrics
kafkapilot scan --bastion kafka-prod --output capacity-planning-$(date +%Y%m%d)

# Analyze resource utilization
kafkapilot task test minimum_cpu_cores ./capacity-planning-*/
kafkapilot task test thread_configuration ./capacity-planning-*/
kafkapilot task test multiple_log_dirs ./capacity-planning-*/

# Generate capacity report
kafkapilot analyze ./capacity-planning-*/ --report json > capacity-report.json
```

**Time required:** 45 minutes  
**Prerequisites:** Access to historical metrics

[📖 Full Tutorial: Capacity Planning](tutorials/05-capacity-planning.html)

---

### 6. Security Audit

Perform comprehensive security assessment of your Kafka cluster.

**What you'll learn:**
- Security configuration analysis
- Authentication and authorization checks
- Encryption validation
- Access control review

```bash
# Security-focused scan
kafkapilot scan --bastion kafka-prod --output security-audit-$(date +%Y%m%d)

# Check security configurations
kafkapilot task test authentication_authorization ./security-audit-*/
kafkapilot task test in_transit_encryption ./security-audit-*/
kafkapilot task test separate_listeners ./security-audit-*/

# Generate security report
kafkapilot analyze ./security-audit-*/ --report markdown > security-audit-report.md
```

**Time required:** 30 minutes  
**Prerequisites:** Understanding of Kafka security concepts

[📖 Full Tutorial: Security Audit](tutorials/06-security-audit.html)

---

## Advanced Workflows

### 7. Multi-Environment Comparison

Compare configurations and health across development, staging, and production.

**What you'll learn:**
- Multi-environment scanning
- Configuration drift detection
- Environment-specific optimizations
- Migration planning

```bash
# Scan multiple environments
kafkapilot scan --bastion kafka-dev --output env-comparison/dev
kafkapilot scan --bastion kafka-staging --output env-comparison/staging  
kafkapilot scan --bastion kafka-prod --output env-comparison/prod

# Compare configurations
diff -r env-comparison/dev/brokers/broker_1/configs/ env-comparison/prod/brokers/broker_1/configs/

# Analyze each environment
for env in dev staging prod; do
    kafkapilot analyze env-comparison/$env --report json > env-comparison/$env-analysis.json
done
```

**Time required:** 60 minutes  
**Prerequisites:** Access to multiple environments

[📖 Full Tutorial: Multi-Environment Comparison](tutorials/07-multi-environment.html)

---

### 8. Custom Analysis Tasks

Create and deploy custom analysis tasks for organization-specific requirements.

**What you'll learn:**
- Analysis task YAML structure
- Custom prompt engineering
- Data filtering techniques
- Task testing and validation

```yaml
# Example custom task: check for custom naming conventions
id: custom_topic_naming
name: "Topic Naming Convention Compliance"
description: "Validates topic names follow organization standards"
category: "compliance"
include_data:
  - cluster
prompt: |
  Analyze the topic names and verify they follow our naming convention:
  - Environment prefix (dev/staging/prod)
  - Team identifier
  - Service name
  - Version suffix
  
  Topics: {{cluster.topics}}
  
  Report any non-compliant topic names and suggest corrections.
```

**Time required:** 45 minutes  
**Prerequisites:** YAML knowledge, understanding of LLM prompts

[📖 Full Tutorial: Custom Analysis Tasks](tutorials/08-custom-tasks.html)

---

## Integration Tutorials

### 9. CI/CD Integration

Integrate KafkaPilot into your deployment pipeline for automated health checks.

**What you'll learn:**
- Automated health validation
- Pipeline integration patterns
- Report generation for teams
- Failure handling strategies

```yaml
# Example GitHub Actions workflow
name: Kafka Health Check
on:
  schedule:
    - cron: '0 */6 * * *'  # Every 6 hours
  workflow_dispatch:

jobs:
  kafka-health:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Build KafkaPilot
        run: cargo build --release
      - name: Run Health Check
        env:
          OPENAI_API_KEY: ${{ secrets.OPENAI_API_KEY }}
        run: |
          ./target/release/kafkapilot scan --bastion kafka-prod --output health-check
          ./target/release/kafkapilot analyze health-check --report json > health-report.json
      - name: Upload Results
        uses: actions/upload-artifact@v3
        with:
          name: kafka-health-report
          path: health-report.json
```

**Time required:** 30 minutes  
**Prerequisites:** CI/CD platform knowledge

[📖 Full Tutorial: CI/CD Integration](tutorials/09-cicd-integration.html)

---

### 10. Monitoring Integration

Integrate KafkaPilot findings with your monitoring and alerting systems.

**What you'll learn:**
- Prometheus metrics integration
- Grafana dashboard creation
- PagerDuty/Slack alerting
- Automated remediation triggers

```bash
# Generate monitoring-friendly output
kafkapilot analyze ./kafka-scan --report json | jq '.findings[] | select(.severity == "critical")' > critical-findings.json

# Push metrics to Prometheus pushgateway
curl -X POST http://pushgateway:9091/metrics/job/kafkapilot/instance/prod \
  --data "kafka_critical_findings $(cat critical-findings.json | jq length)"
```

**Time required:** 45 minutes  
**Prerequisites:** Monitoring stack knowledge

[📖 Full Tutorial: Monitoring Integration](tutorials/10-monitoring-integration.html)

---

## Tutorial Series Navigation

**Beginner Track:**
1. [First Health Check](tutorials/01-first-health-check.html) → 
2. [Performance Troubleshooting](tutorials/02-performance-troubleshooting.html) →
3. [AI-Powered Analysis](tutorials/03-ai-analysis.html)

**Production Track:**
4. [Incident Response](tutorials/04-incident-response.html) →
5. [Capacity Planning](tutorials/05-capacity-planning.html) →
6. [Security Audit](tutorials/06-security-audit.html)

**Advanced Track:**
7. [Multi-Environment Comparison](tutorials/07-multi-environment.html) →
8. [Custom Analysis Tasks](tutorials/08-custom-tasks.html)

**Integration Track:**
9. [CI/CD Integration](tutorials/09-cicd-integration.html) →
10. [Monitoring Integration](tutorials/10-monitoring-integration.html)

---

## Next Steps

- **[Examples](examples.html)** - See real-world troubleshooting scenarios
- **[API Reference](api.html)** - Complete command documentation
- **[How-To Guides](how-to.html)** - Quick solutions for specific problems

**Need help with tutorials?** Check our [support options](support.html) or ask in [GitHub Discussions](https://github.com/softwaremill/kafkapilot/discussions).