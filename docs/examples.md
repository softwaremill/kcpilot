---
layout: page
title: Examples
permalink: /examples/
---

# Real-World Examples

Practical examples of using KafkaPilot to solve common Kafka operational challenges.

## Production Incident Examples

### 1. Consumer Lag Investigation

**Scenario:** Consumers are lagging by hours, affecting real-time data processing.

**Problem Identification:**
```bash
# Emergency scan
kafkapilot scan --bastion kafka-prod --broker kafka-broker-1.internal:9092 --output consumer-lag-incident

# Check the summary for immediate insights
cat consumer-lag-incident/COLLECTION_SUMMARY.md

# Look for consumer group data
ls consumer-lag-incident/cluster/kafkactl/
```

**Analysis:**
```bash
# Check broker health
kafkapilot task test jvm_heap_memory ./consumer-lag-incident
kafkapilot task test minimum_cpu_cores ./consumer-lag-incident

# Generate comprehensive analysis
export OPENAI_API_KEY=your_key
kafkapilot analyze ./consumer-lag-incident --report terminal
```

**Key Findings:**
- Brokers showing 90%+ heap utilization
- Consumer groups stuck on specific partitions
- Network connectivity issues between certain brokers

**Resolution:** Restart problematic brokers during maintenance window, tune JVM heap settings.

---

### 2. Disk Space Crisis

**Scenario:** Kafka cluster running out of disk space, threatening data retention.

**Investigation:**
```bash
# Urgent data collection
kafkapilot scan --bastion kafka-prod --broker kafka-broker-1.internal:9092 --output disk-space-crisis

# Check disk utilization immediately
grep -r "disk\|space\|full" disk-space-crisis/brokers/*/system/
```

**Analysis Results:**
```
Broker 1: 95% disk utilization on /kafka-logs
Broker 2: 87% disk utilization on /kafka-logs  
Broker 3: 92% disk utilization on /kafka-logs
```

**AI Analysis Output:**
```bash
kafkapilot analyze ./disk-space-crisis --report terminal

# Output:
# CRITICAL: Disk utilization approaching 95% on multiple brokers
# RECOMMENDATION: 
# 1. Reduce log retention from 7 days to 3 days immediately
# 2. Enable log compression
# 3. Plan disk expansion within 24 hours
```

**Immediate Actions:**
```bash
# Reduce retention temporarily
kafka-configs --bootstrap-server kafka-prod:9092 --entity-type topics --alter \
  --add-config retention.ms=259200000  # 3 days
```

---

### 3. Network Partition Recovery

**Scenario:** Kafka cluster split-brain after network partition, leaders not accessible.

**Data Collection:**
```bash
# Collect data from all accessible brokers
kafkapilot scan --bastion kafka-prod --broker kafka-broker-1.internal:9092 --output network-partition

# Check broker connectivity
kafkapilot task test broker_connectivity ./network-partition
```

**Analysis Findings:**
- Brokers 1-3: Accessible, serving as leaders
- Brokers 4-6: Network isolated, attempting leader election
- ISR (In-Sync Replicas) dramatically reduced

**Recovery Actions:**
1. Identify isolated brokers from scan data
2. Restore network connectivity
3. Monitor ISR recovery through subsequent scans

---

## Performance Optimization Examples

### 4. Throughput Optimization

**Initial State:**
```bash
# Baseline performance scan
kafkapilot scan --bastion kafka-prod --broker kafka-broker-1.internal:9092 --output baseline-performance

# Check thread configuration
kafkapilot task test thread_configuration ./baseline-performance
```

**Findings:**
- `num.io.threads=8` (default) insufficient for workload
- Single log directory per broker
- Batch size too small for high-throughput topics

**Optimization Process:**
```bash
# After configuration changes
kafkapilot scan --bastion kafka-prod --broker kafka-broker-1.internal:9092 --output optimized-performance

# Compare configurations
diff -r baseline-performance/brokers/broker_1/configs/ \
         optimized-performance/brokers/broker_1/configs/
```

**Results:**
- 3x throughput improvement
- Reduced CPU utilization
- Better partition distribution

---

### 5. JVM Tuning

**Problem:** Frequent garbage collection pauses causing request timeouts.

**Investigation:**
```bash
kafkapilot scan --bastion kafka-prod --broker kafka-broker-1.internal:9092 --output jvm-tuning

# Analyze heap usage patterns
kafkapilot task test jvm_heap_preallocation ./jvm-tuning
kafkapilot task test jvm_heap_size_limit ./jvm-tuning
```

**JVM Configuration Analysis:**
```
Current: -Xmx4G -Xms4G
Recommendation: -Xmx8G -Xms8G -XX:+UseG1GC -XX:MaxGCPauseMillis=100
```

**Results After Tuning:**
- GC pause time reduced from 2000ms to 50ms
- Throughput increased by 40%
- Eliminated timeout errors

---

## Security Assessment Examples

### 6. Encryption Audit

**Requirement:** Ensure all data is encrypted in transit for compliance.

**Assessment:**
```bash
kafkapilot scan --bastion kafka-prod --broker kafka-broker-1.internal:9092 --output security-audit

# Check encryption configuration
kafkapilot task test in_transit_encryption ./security-audit
kafkapilot task test separate_listeners ./security-audit
```

**Security Analysis:**
```yaml
# Findings from analysis
Encryption Status:
  - Inter-broker communication: ✅ SSL/TLS enabled
  - Client communication: ❌ PLAINTEXT enabled
  - ZooKeeper communication: ✅ SSL/TLS enabled

Recommendations:
  - Disable PLAINTEXT listener
  - Enforce SSL for all client connections
  - Update client configurations
```

**Remediation:**
```properties
# Updated server.properties
listeners=SSL://0.0.0.0:9092
security.inter.broker.protocol=SSL
ssl.keystore.location=/path/to/keystore
ssl.truststore.location=/path/to/truststore
```

---

### 7. Access Control Review

**Scenario:** Audit user access and permissions across the cluster.

```bash
kafkapilot scan --bastion kafka-prod --broker kafka-broker-1.internal:9092 --output access-audit

# Check authentication configuration  
kafkapilot task test authentication_authorization ./access-audit
```

**Access Control Findings:**
- SASL/SCRAM authentication properly configured
- ACLs need refinement for principle of least privilege
- Service accounts using overly broad permissions

---

## Capacity Planning Examples

### 8. Growth Projection

**Business Context:** Planning infrastructure for 10x message volume growth.

**Current State Analysis:**
```bash
kafkapilot scan --bastion kafka-prod --broker kafka-broker-1.internal:9092 --output capacity-baseline

# Analyze current resource utilization
kafkapilot task test log_dirs_disk_size_uniformity ./capacity-baseline
kafkapilot task test minimum_cpu_cores ./capacity-baseline
```

**Capacity Analysis:**
```
Current Load:
  - Messages/sec: 50,000
  - Data ingestion: 100 MB/sec
  - CPU utilization: 40%
  - Disk I/O: 60%

Projected Requirements (10x growth):
  - Messages/sec: 500,000
  - Data ingestion: 1 GB/sec
  - Recommended: 3x broker count
  - Storage: 5x current capacity
```

**Scaling Plan:**
1. Horizontal scaling: 6 → 18 brokers
2. Partition rebalancing strategy
3. Network bandwidth upgrade requirements

---

### 9. Cost Optimization

**Goal:** Reduce infrastructure costs while maintaining performance.

**Analysis:**
```bash
kafkapilot scan --bastion kafka-prod --broker kafka-broker-1.internal:9092 --output cost-optimization

# Generate comprehensive analysis
kafkapilot analyze ./cost-optimization --report json > cost-analysis.json
```

**Cost Optimization Opportunities:**
```json
{
  "findings": [
    {
      "category": "resource_optimization",
      "description": "Over-provisioned disk space",
      "impact": "30% cost reduction possible",
      "recommendation": "Reduce retention period from 30 days to 7 days"
    },
    {
      "category": "instance_optimization", 
      "description": "Under-utilized broker instances",
      "impact": "25% cost reduction possible",
      "recommendation": "Consolidate from 12 to 9 brokers"
    }
  ]
}
```

---

## Multi-Environment Examples

### 10. Configuration Drift Detection

**Problem:** Inconsistent configurations between staging and production.

**Comparison Process:**
```bash
# Scan both environments
kafkapilot scan --bastion kafka-staging --broker kafka-broker-1.internal:9092 --output staging-config
kafkapilot scan --bastion kafka-prod --broker kafka-broker-1.internal:9092 --output prod-config

# Compare key configurations
diff -r staging-config/brokers/broker_1/configs/server.properties \
         prod-config/brokers/broker_1/configs/server.properties
```

**Configuration Drift Report:**
```diff
< log.retention.hours=168  # staging: 7 days
> log.retention.hours=72   # production: 3 days

< num.network.threads=3    # staging: default
> num.network.threads=8    # production: tuned

< compression.type=producer
> compression.type=lz4     # production: optimized
```

**Standardization Actions:**
1. Update staging to match production performance settings
2. Implement configuration management
3. Automate drift detection in CI/CD

---

## Advanced Troubleshooting Examples

### 11. ZooKeeper Health Issues

**Scenario:** Intermittent ZooKeeper connectivity affecting cluster stability.

```bash
kafkapilot scan --bastion kafka-prod --broker kafka-broker-1.internal:9092 --output zk-health

# Check ZooKeeper-specific issues
kafkapilot task test zookeeper_ha_check ./zk-health
kafkapilot task test zookeeper_heap_memory ./zk-health
```

**ZooKeeper Analysis:**
- Heap memory at 85% (approaching threshold)
- Leader elections occurring frequently
- Network latency spikes to ZooKeeper ensemble

**Resolution Strategy:**
1. Increase ZooKeeper heap size
2. Tune network timeouts
3. Consider ZooKeeper ensemble placement

---

### 12. KRaft Migration Assessment

**Scenario:** Evaluating migration from ZooKeeper to KRaft mode.

```bash
kafkapilot scan --bastion kafka-prod --broker kafka-broker-1.internal:9092 --output kraft-assessment

# Check KRaft readiness
kafkapilot task test kraft_controller_ha_check ./kraft-assessment
```

**Migration Readiness Report:**
- Kafka version: 3.4.0 ✅ (KRaft ready)
- Current ZooKeeper health: ⚠️ (frequent issues)
- Configuration compatibility: ✅ (minor adjustments needed)
- Client compatibility: ✅ (all clients support KRaft)

**Migration Plan:**
1. Set up parallel KRaft cluster
2. Dual-write testing period
3. Gradual client migration
4. ZooKeeper decommission

---

## Next Steps
- **[Support](https://softwaremill.com/services/apache-kafka-services/)** - Get help with your specific use case
