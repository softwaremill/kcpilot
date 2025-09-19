---
layout: page
title: How-To Guides
permalink: /how-to/
---

# How-To Guides

Quick solutions for specific KafkaPilot tasks and common problems.

## Quick Tasks

### Check Cluster Health Status

```bash
# Basic health check
kafkapilot scan --bastion kafka-prod --output health-check
cat health-check/COLLECTION_SUMMARY.md

# Quick status with AI analysis
export OPENAI_API_KEY=your_key
kafkapilot analyze health-check --report terminal | head -20
```

### Find Disk Space Issues

```bash
# Scan and check disk usage
kafkapilot scan --bastion kafka-prod --output disk-check

# Look for disk space warnings
grep -i "disk\|space\|full" disk-check/brokers/*/system/df.txt
grep -i "disk" disk-check/COLLECTION_SUMMARY.md
```

### Verify Replication Factor

```bash
# Check replication configuration
kafkapilot task test isr_replication_margin ./kafka-scan-data

# Manual verification
grep "default.replication.factor" kafka-scan-data/brokers/*/configs/server.properties
```

### Monitor JVM Memory Usage

```bash
# Check heap memory configuration
kafkapilot task test jvm_heap_memory ./kafka-scan-data
kafkapilot task test jvm_heap_size_limit ./kafka-scan-data

# View raw JVM metrics
ls kafka-scan-data/brokers/*/metrics/
cat kafka-scan-data/brokers/broker_1/metrics/jstat_gc.txt
```

---

## Configuration Tasks

### Compare Broker Configurations

```bash
# Scan cluster
kafkapilot scan --bastion kafka-prod --output config-check

# Compare server.properties across brokers
diff config-check/brokers/broker_1/configs/server.properties \
     config-check/brokers/broker_2/configs/server.properties

# Check for configuration inconsistencies
for broker in config-check/brokers/*/; do
    echo "=== $(basename $broker) ==="
    grep "num.network.threads" $broker/configs/server.properties
done
```

### Validate Security Settings

```bash
# Check encryption and authentication
kafkapilot task test in_transit_encryption ./kafka-scan-data
kafkapilot task test authentication_authorization ./kafka-scan-data
kafkapilot task test separate_listeners ./kafka-scan-data

# Manual security check
grep -r "security\|ssl\|sasl\|auth" kafka-scan-data/brokers/*/configs/
```

### Review Log4j Configuration

```bash
# Collect log configuration
kafkapilot scan --bastion kafka-prod --output log-config

# Check log4j settings
find log-config/brokers/ -name "*log4j*" -exec cat {} \;

# Verify log rotation
ls -la kafka-scan-data/brokers/*/logs/
```

---

## Performance Tasks

### Identify Performance Bottlenecks

```bash
# Comprehensive performance scan
kafkapilot scan --bastion kafka-prod --output perf-scan

# Check key performance indicators
kafkapilot task test thread_configuration ./perf-scan
kafkapilot task test minimum_cpu_cores ./perf-scan
kafkapilot task test multiple_log_dirs ./perf-scan

# CPU and memory analysis
grep -r "load average\|memory" perf-scan/brokers/*/system/
```

### Analyze Network Connectivity

```bash
# Check network configuration
kafkapilot task test separate_listeners ./kafka-scan-data

# Review network connections
cat kafka-scan-data/brokers/*/system/netstat.txt

# Check inter-broker communication
grep -r "9092\|9093" kafka-scan-data/brokers/*/system/netstat.txt
```

### Optimize JVM Settings

```bash
# Current JVM analysis
kafkapilot task test jvm_heap_preallocation ./kafka-scan-data

# View current JVM configuration
grep -r "Xmx\|Xms\|XX:" kafka-scan-data/brokers/*/system/

# Check GC performance
cat kafka-scan-data/brokers/*/metrics/jstat_gc.txt
```

---

## Troubleshooting Tasks

### Debug SSH Connection Issues

```bash
# Test SSH connectivity manually
ssh -v kafka-prod "echo 'SSH test successful'"

# Check SSH agent
ssh-add -l
echo $SSH_AUTH_SOCK

# Debug KafkaPilot SSH issues
RUST_LOG=kafkapilot=debug kafkapilot scan --bastion kafka-prod
```

### Fix Collection Errors

```bash
# Run with maximum debugging
RUST_LOG=debug kafkapilot scan --bastion kafka-prod --output debug-scan

# Check for permission issues
grep -i "permission\|denied\|error" debug-scan/scan_metadata.json

# Verify required tools on bastion
ssh kafka-prod "which kafkactl ps df free"
```

### Handle Large Log Files

```bash
# Limit log collection size
export KAFKAPILOT_MAX_LOG_SIZE=100MB
kafkapilot scan --bastion kafka-prod --output limited-logs

# Skip log collection entirely
# (Future feature - manual configuration needed currently)
```

---

## Analysis Tasks

### Generate Custom Reports

```bash
# Multiple report formats
kafkapilot analyze ./kafka-scan-data --report terminal,markdown,json

# Terminal-only for quick viewing
kafkapilot analyze ./kafka-scan-data --report terminal

# JSON for automation
kafkapilot analyze ./kafka-scan-data --report json > analysis.json
```

### Extract Specific Metrics

```bash
# Get critical findings only
kafkapilot analyze ./kafka-scan-data --report json | \
  jq '.findings[] | select(.severity == "critical")'

# Count findings by category
kafkapilot analyze ./kafka-scan-data --report json | \
  jq '.findings | group_by(.category) | map({category: .[0].category, count: length})'

# Export for monitoring
kafkapilot analyze ./kafka-scan-data --report json | \
  jq '.findings | length' > /tmp/kafka_findings_count.txt
```

### Run Specific Analysis Tasks

```bash
# List all available tasks
kafkapilot task list --detailed

# Test individual tasks
kafkapilot task test jvm_heap_memory ./kafka-scan-data
kafkapilot task test zookeeper_ha_check ./kafka-scan-data
kafkapilot task test kraft_controller_ha_check ./kafka-scan-data

# Batch test multiple tasks
for task in jvm_heap_memory thread_configuration minimum_cpu_cores; do
    echo "Testing $task..."
    kafkapilot task test $task ./kafka-scan-data
done
```

---

## Automation Tasks

### Schedule Regular Health Checks

```bash
# Cron job for daily health checks
# Add to crontab: 0 2 * * * /path/to/daily-kafka-check.sh

#!/bin/bash
# daily-kafka-check.sh
DATE=$(date +%Y%m%d)
OUTPUT_DIR="/var/kafka-health/daily-$DATE"

kafkapilot scan --bastion kafka-prod --output "$OUTPUT_DIR"

if [ -n "$OPENAI_API_KEY" ]; then
    kafkapilot analyze "$OUTPUT_DIR" --report json > "$OUTPUT_DIR/analysis.json"
    
    # Send alert if critical issues found
    CRITICAL_COUNT=$(jq '.findings[] | select(.severity == "critical") | length' "$OUTPUT_DIR/analysis.json")
    if [ "$CRITICAL_COUNT" -gt 0 ]; then
        echo "ALERT: $CRITICAL_COUNT critical issues found in Kafka cluster" | \
          mail -s "Kafka Health Alert" ops-team@company.com
    fi
fi
```

### CI/CD Pipeline Integration

```yaml
# GitHub Actions example
name: Kafka Health Check
on:
  schedule:
    - cron: '0 */6 * * *'  # Every 6 hours

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
          
      - name: Check for Critical Issues
        run: |
          CRITICAL=$(jq '.findings[] | select(.severity == "critical") | length' health-report.json)
          if [ "$CRITICAL" -gt 0 ]; then
            echo "::error::Found $CRITICAL critical issues in Kafka cluster"
            exit 1
          fi
          
      - name: Upload Report
        uses: actions/upload-artifact@v3
        with:
          name: kafka-health-report
          path: health-report.json
```

### Monitoring System Integration

```bash
# Prometheus metrics generation
#!/bin/bash
# kafka-metrics-exporter.sh

kafkapilot scan --bastion kafka-prod --output /tmp/kafka-scan
kafkapilot analyze /tmp/kafka-scan --report json > /tmp/kafka-analysis.json

# Extract metrics
CRITICAL_ISSUES=$(jq '.findings[] | select(.severity == "critical") | length' /tmp/kafka-analysis.json)
WARNING_ISSUES=$(jq '.findings[] | select(.severity == "warning") | length' /tmp/kafka-analysis.json)
HEALTHY_CHECKS=$(jq '.findings[] | select(.severity == "info") | length' /tmp/kafka-analysis.json)

# Push to Pushgateway
cat << EOF | curl --data-binary @- http://pushgateway:9091/metrics/job/kafkapilot/instance/prod
kafka_critical_issues $CRITICAL_ISSUES
kafka_warning_issues $WARNING_ISSUES  
kafka_healthy_checks $HEALTHY_CHECKS
kafka_last_scan_timestamp $(date +%s)
EOF
```

---

## Data Management

### Archive Old Scan Data

```bash
# Archive scans older than 30 days
find /var/kafka-scans -name "kafka-scan-*" -mtime +30 -exec tar -czf {}.tar.gz {} \; -exec rm -rf {} \;

# Keep only last 10 scans
ls -dt /var/kafka-scans/kafka-scan-* | tail -n +11 | xargs rm -rf
```

### Export Scan Data

```bash
# Export specific broker data
tar -czf broker-1-data.tar.gz kafka-scan-data/brokers/broker_1/

# Export only configurations
find kafka-scan-data -name "*.properties" -o -name "*.conf" | \
  tar -czf kafka-configs.tar.gz -T -

# Export analysis results
kafkapilot analyze ./kafka-scan-data --report json | \
  jq '.' > kafka-analysis-$(date +%Y%m%d).json
```

### Clean Up Temporary Files

```bash
# Clean up failed scans
find /tmp -name "kafka-scan-*" -mtime +1 -exec rm -rf {} \;

# Remove old debug logs
find . -name "kafkapilot-debug-*.log" -mtime +7 -delete
```

---

## Advanced Usage

### Custom Analysis Task Development

```yaml
# Create custom task file: analysis_tasks/custom_topic_check.yaml
id: custom_topic_naming
name: "Topic Naming Convention Check"
description: "Validates topic names follow company standards"
category: "compliance"
include_data:
  - cluster
prompt: |
  Review the following Kafka topics and check if they follow our naming convention:
  - Must start with environment prefix (dev/staging/prod)
  - Must include team name
  - Must use kebab-case
  
  Topics: {{cluster.topics}}
  
  Report any non-compliant topics and suggest corrections.
severity_keywords:
  critical: ["violates", "non-compliant", "critical"]
  warning: ["recommend", "should", "consider"]
  info: ["compliant", "follows", "correct"]
```

```bash
# Test custom task
kafkapilot task test custom_topic_naming ./kafka-scan-data
```

### Multi-Environment Comparison

```bash
# Scan multiple environments
for env in dev staging prod; do
    kafkapilot scan --bastion kafka-$env --output env-$env
done

# Compare configurations
for config in server.properties log4j.properties; do
    echo "=== Comparing $config ==="
    diff -u env-dev/brokers/broker_1/configs/$config \
            env-prod/brokers/broker_1/configs/$config
done

# Generate comparison report
for env in dev staging prod; do
    kafkapilot analyze env-$env --report json > comparison-$env.json
done
```

### Performance Benchmarking

```bash
# Before optimization
kafkapilot scan --bastion kafka-prod --output baseline-performance

# After optimization
kafkapilot scan --bastion kafka-prod --output optimized-performance

# Compare key metrics
echo "=== JVM Heap Usage Comparison ==="
grep -h "heap" baseline-performance/brokers/*/metrics/* | sort
grep -h "heap" optimized-performance/brokers/*/metrics/* | sort

echo "=== CPU Usage Comparison ==="
grep "load average" baseline-performance/brokers/*/system/uptime.txt
grep "load average" optimized-performance/brokers/*/system/uptime.txt
```

---

## Troubleshooting Common Issues

### "No brokers discovered"

```bash
# Check kafkactl installation
ssh kafka-prod "kafkactl get brokers"

# Check network connectivity
ssh kafka-prod "telnet kafka-broker-1 9092"

# Manual broker list (future feature)
# kafkapilot scan --bastion kafka-prod --brokers kafka-1,kafka-2,kafka-3
```

### "Permission denied" errors

```bash
# Check file permissions
ssh kafka-prod "ls -la /opt/kafka/config/"
ssh kafka-prod "ls -la /var/log/kafka/"

# Test with sudo (if available)
ssh kafka-prod "sudo ls /var/log/kafka/"
```

### Large output directories

```bash
# Check scan size before analysis
du -sh kafka-scan-*/

# Compress old scans
tar -czf kafka-scan-archive.tar.gz kafka-scan-*/ && rm -rf kafka-scan-*/

# Limit collection scope (manual configuration)
# Future feature: --skip-logs, --skip-metrics flags
```

---

**Need more specific help?** Check our [examples](examples.html) or [ask the community](support.html#community-support).