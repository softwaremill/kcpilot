---
layout: home
title: KCPilot
---

# KCPilot

**CLI-first Kafka health diagnostics tool for production environments**

> ⚠️ **Innovation Hub Project**: This project is part of SoftwareMill's Innovation Hub and is currently in MVP stage. While functional, it may contain bugs and has significant room for improvement. We welcome feedback and contributions!
> 
> Treat this project as a base, invent your own tasks and share them with the world by opening a pull request.

## Intro

We're building KCPilot. A tool that proactively diagnoses and resolves common issues in Apache Kafka. We're starting with ~17 scenarios covering typical configuration, availability, and performance faults.

<div class="feedback-cta">

<h3>What do we need from you?</h3>

<p>Your feedback. Tell us what your most common Kafka pain points are and help us create a tool that will make your work easier in the future. It will only take a minute.</p>

<a href="https://forms.gle/2pEXQeNFr3Pw1gb56" class="cta-button" target="_blank">Share Your Feedback</a>

<div class="alternative-contact">
Or reach out directly via <a href="mailto:kafka-pilot@softwaremill.com">email</a>
</div>

</div>


### Why is it worth your time?

Sharing your input with us you can:

- Gain early access to the private KCPilot beta.
- Receive priority treatment by having your responses mapped directly to our backlog.
- Transform tribal knowledge into automated checks to scale expertise across teams with diagnostics.
- Test scenarios across diverse environments for higher reliability.
- Automate compliance with codified security and operational rules.
- Reduce operational overhead through smart automation.
- Accelerate onboarding with embedded best practices.

### Why is KCPilot a Game-Changer?

Our goal is to provide you with actionable insights when every minute counts. That's why KCPilot is more than just a monitoring tool; it's your expert companion for Kafka health.

- Zero-Impact Diagnostics: Our SSH-based system performs read-only operations to give deep insights without touching your production cluster.
- Comprehensive Analysis: We collect and analyze everything, including configurations, logs, metrics, and system information, giving you the full picture.
- Flexible Reporting: Get the data you need, in the format you want—from a quick terminal overview to detailed Markdown reports and structured JSON for automated workflows.
- Built for Security: We integrate with your existing SSH infrastructure, ensuring your data stays secure.

### Examples we already cover:

**JVM & Memory Configuration**

- JVM Heap Memory Preallocation Check \- Ensures Xms equals Xmx for optimal performance  
- JVM Heap vs System Memory Ratio Check \- Verifies heap ≤ 25% of system RAM for page cache  
- JVM Heap Size Limit Check \- Prevents heap settings above 8GB to avoid performance issues

**High Availability & Clustering**

- Broker Count High Availability Check \- Analyzes broker count for HA considerations  
- ISR vs Replication Factor Margin Check \- Ensures adequate margin over min.insync.replicas  
- Zookeeper High Availability Configuration Check \- Prevents split-brain scenarios in ZK clusters  
- KRaft Controller Quorum High Availability Check \- Ensures proper KRaft controller quorum setup

**Security & Access Control**

- Authentication and Authorization Configuration Check \- Blocks anonymous access  
- In-Transit Encryption Configuration Check \- Ensures secure communication  
- Rack Awareness Configuration Check \- Validates failure zone distribution

**Performance & Resource Management**

- Thread Configuration Validation \- Validates network, I/O, and replication thread settings  
- Separate Client and Cluster Listeners Check \- Ensures separate listeners for optimal performance  
- Multiple Log Directories Configuration Check \- Detects complex log.dirs configurations  
- Minimum CPU Core Count Check \- Ensures at least 4 CPU cores per broker

**Operational Health**

- Recent Log Error Detection \- Scans logs for ERROR/FATAL messages in last 24 hours

**Zookeeper-Specific**

- Zookeeper Heap Memory Size Check \- Validates ZK heap ≤ 2GB for typical deployments  
- Zookeeper Heap Memory Preallocation Check \- Ensures ZK heap preallocation

What’s next? Your priceless feedback\!

## Professional Kafka Services

**Experiencing Kafka challenges?** [SoftwareMill](https://softwaremill.com) offers professional Kafka consulting, implementation, and support services:

- **Kafka Architecture & Migration** - Design scalable, resilient Kafka infrastructures
- **Performance Optimization** - Tune your clusters for maximum throughput and reliability  
- **24/7 Production Support** - Expert support when you need it most
- **Training & Knowledge Transfer** - Empower your team with Kafka expertise

[Contact our Kafka experts →](https://softwaremill.com/services/apache-kafka-services/)

## Code Preview

```bash
# Quick cluster health check
kcpilot scan --bastion kafka-prod --broker kafka-broker-1.internal:9092

# Comprehensive analysis with AI insights
kcpilot analyze ./kafka-scan-2024-01-15 --report terminal

# Test specific configuration issues
kcpilot task test replication_factor ./kafka-scan-2024-01-15

# List all available analysis tasks
kcpilot task list --detailed
```

## Getting Started

Depending on your role and needs, this documentation offers multiple learning paths:

1. **Quick Start** - [Get up and running in 5 minutes](quickstart/) with basic cluster scanning
2. **Production Examples** - [Real-world use cases](https://softwaremill.github.io/kcpilot/analysis-tasks/) and troubleshooting workflows
3. **Advanced Configuration** - Deep dive into [AI analysis tasks](https://softwaremill.github.io/kcpilot/api/#configuration-files) and custom reporting

## Installation & Support

- **Installation Guide** - [Multiple installation options](https://softwaremill.github.io/kcpilot/installation/) for different environments
- **API Reference** - Complete [command-line interface documentation](https://softwaremill.github.io/kcpilot/api/#configuration-files)
- **Community Support** - [GitHub issues](https://github.com/softwaremill/kcpilot/issues) and discussions
- **Professional Support** - [Enterprise support options](https://softwaremill.com/services/apache-kafka-services/) from SoftwareMill

KCPilot is open source (Apache 2.0 license) and [available on GitHub](https://github.com/softwaremill/kcpilot).

## Other SoftwareMill Projects

SoftwareMill is a leader in Scala and Kafka ecosystem projects:

- **[Tapir](https://github.com/softwaremill/tapir)** - Type-safe, declarative web API library for Scala
- **[Ox](https://github.com/softwaremill/ox)** - Safe direct-style concurrency and resiliency library
- **[Quicklens](https://github.com/softwaremill/quicklens)** - Modify deeply nested case class fields
- **[Elasticmq](https://github.com/softwaremill/elasticmq)** - Message queueing system with an Amazon SQS-compatible interface

---

*Built with ❤️ by the [SoftwareMill](https://softwaremill.com) team*