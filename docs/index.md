---
layout: home
title: KafkaPilot
---

# KafkaPilot

**CLI-first Kafka health diagnostics tool for production environments**

> ⚠️ **Innovation Hub Project**: This project is part of [SoftwareMill's Innovation Hub](https://softwaremill.com/innovation-hub/) and is currently in MVP stage. While functional, it may contain bugs and has significant room for improvement. We welcome feedback and contributions!

## Intro

KafkaPilot is a comprehensive Kafka health diagnostics tool that automatically collects cluster signals, identifies issues, and provides actionable remediation guidance. Built for operations teams and DevOps engineers managing production Kafka clusters.

KafkaPilot is fast and production-ready. The SSH-based collection system is designed for zero-downtime diagnostics, while our AI-powered analysis engine provides intelligent insights into cluster health and performance bottlenecks.

```bash
# Scan your Kafka cluster in seconds
kafkapilot scan --bastion kafka-prod --output kafka-health-report

# Analyze with AI-powered insights
kafkapilot analyze ./kafka-health-report --report terminal
```

KafkaPilot integrates seamlessly with your existing infrastructure, requiring only SSH access and standard monitoring tools. No agents, no cluster modifications - just comprehensive diagnostics when you need them.

**Seamless integration with your Kafka operations workflow:**

- **All major SSH configurations are supported** - Works with your existing bastion hosts, SSH keys, and access patterns. This is especially useful for organizations with strict security requirements.
- **Zero cluster impact** - Read-only operations ensure your production environment remains unaffected during diagnostics.
- **Comprehensive reporting** - Generate insights in multiple formats: terminal output, markdown reports, and structured JSON for integration with monitoring systems.

## Why KafkaPilot?

- **🚀 Production-ready**: SSH-based collection with zero cluster impact
- **🔍 Comprehensive diagnostics**: Collects configs, logs, metrics, and system information
- **🤖 AI-powered analysis**: Intelligent issue identification and remediation guidance
- **📊 Multiple report formats**: Terminal, markdown, and JSON outputs for any workflow
- **🔒 Security-first**: Read-only operations, works with existing SSH infrastructure
- **⚡ Fast execution**: Parallel collection and analysis for rapid insights

## When KafkaPilot Helps Your Business

### 🚨 Production Issues
When your Kafka cluster is experiencing problems, every minute counts. KafkaPilot quickly identifies the root cause and provides specific remediation steps, reducing MTTR and minimizing business impact.

### 📈 Performance Optimization
Optimize your Kafka cluster's performance with detailed analysis of configuration, resource utilization, and architectural recommendations tailored to your workload patterns.

### 🔧 Health Monitoring
Regular health checks help prevent issues before they impact your business. KafkaPilot's comprehensive diagnostics ensure your Kafka infrastructure remains reliable.

### 💰 Cost Optimization
Identify overprovisioned resources, inefficient configurations, and optimization opportunities that can significantly reduce your infrastructure costs.

## Professional Kafka Services

**Experiencing Kafka challenges?** [SoftwareMill](https://softwaremill.com) offers professional Kafka consulting, implementation, and support services:

- **Kafka Architecture & Migration** - Design scalable, resilient Kafka infrastructures
- **Performance Optimization** - Tune your clusters for maximum throughput and reliability  
- **24/7 Production Support** - Expert support when you need it most
- **Training & Knowledge Transfer** - Empower your team with Kafka expertise

[Contact our Kafka experts →](https://softwaremill.com/contact/?topic=kafka)

## Code Preview

```bash
# Quick cluster health check
kafkapilot scan --bastion kafka-prod

# Comprehensive analysis with AI insights
kafkapilot analyze ./kafka-scan-2024-01-15 --report terminal,json

# Test specific configuration issues
kafkapilot task test replication_factor ./kafka-scan-2024-01-15

# List all available analysis tasks
kafkapilot task list --detailed
```

## Getting Started

Depending on your role and needs, this documentation offers multiple learning paths:

1. **Quick Start** - [Get up and running in 5 minutes](quickstart.html) with basic cluster scanning
2. **Comprehensive Tutorials** - Step-by-step guides for [common diagnostics scenarios](tutorials.html)
3. **Production Examples** - [Real-world use cases](examples.html) and troubleshooting workflows
4. **Advanced Configuration** - Deep dive into [AI analysis tasks](api.html) and custom reporting

## Installation & Support

- **Installation Guide** - [Multiple installation options](installation.html) for different environments
- **API Reference** - Complete [command-line interface documentation](api.html)
- **Community Support** - [GitHub issues](https://github.com/softwaremill/kafkapilot/issues) and discussions
- **Professional Support** - [Enterprise support options](support.html) from SoftwareMill

KafkaPilot is open source (Apache 2.0 license) and [available on GitHub](https://github.com/softwaremill/kafkapilot).

## Other SoftwareMill Projects

SoftwareMill is a leader in Scala and Kafka ecosystem projects:

- **[Tapir](https://github.com/softwaremill/tapir)** - Type-safe, declarative web API library for Scala
- **[Ox](https://github.com/softwaremill/ox)** - Safe direct-style concurrency and resiliency library
- **[Quicklens](https://github.com/softwaremill/quicklens)** - Modify deeply nested case class fields
- **[Elasticmq](https://github.com/softwaremill/elasticmq)** - Message queueing system with an Amazon SQS-compatible interface

---

*Built with ❤️ by the [SoftwareMill](https://softwaremill.com) team*