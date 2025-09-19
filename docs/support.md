---
layout: page
title: Support
permalink: /support/
---

# Support & Community

Get help with KafkaPilot and connect with the community.

## Community Support

### GitHub Resources

- **📚 [Documentation](https://softwaremill.github.io/kafkapilot)** - Complete user guide and API reference
- **🐛 [Issues](https://github.com/softwaremill/kafkapilot/issues)** - Report bugs and request features
- **💬 [Discussions](https://github.com/softwaremill/kafkapilot/discussions)** - Ask questions and share knowledge
- **🔄 [Pull Requests](https://github.com/softwaremill/kafkapilot/pulls)** - Contribute code and improvements

### Getting Help

**Before asking for help:**
1. Check the [documentation](../) and [examples](examples.html)
2. Search [existing issues](https://github.com/softwaremill/kafkapilot/issues)
3. Review [frequently asked questions](#frequently-asked-questions)

**When reporting issues:**
```bash
# Include version information
kafkapilot --version

# Include debug output
RUST_LOG=kafkapilot=debug kafkapilot scan --bastion kafka-prod 2>&1 | tee debug.log
```

### Community Guidelines

- **Be respectful** - Everyone is here to learn and help
- **Search first** - Check if your question has been answered
- **Provide context** - Include relevant configuration and error messages
- **Give back** - Help others when you can

---

## Professional Support

[SoftwareMill](https://softwaremill.com) offers commercial support and consulting services for KafkaPilot and Kafka infrastructure.

### 🏢 Enterprise Support

**What's included:**
- Priority bug fixes and feature requests
- Custom analysis tasks development
- Integration assistance with your infrastructure
- Training and knowledge transfer
- SLA-backed response times

**Best for:** Organizations using KafkaPilot in production environments.

### 🚀 Kafka Consulting Services

Our Kafka experts help with:

#### Infrastructure & Architecture
- **Kafka Cluster Design** - Scalable, resilient architectures
- **Migration Planning** - ZooKeeper to KRaft, version upgrades
- **Multi-Region Setup** - Cross-datacenter replication strategies
- **Security Implementation** - Authentication, authorization, encryption

#### Performance & Operations
- **Performance Tuning** - Optimize throughput and latency
- **Capacity Planning** - Right-size your infrastructure
- **Monitoring & Alerting** - Comprehensive observability setup
- **Disaster Recovery** - Backup and recovery procedures

#### Development & Integration
- **Client Application Guidance** - Best practices for producers/consumers
- **Schema Management** - Schema Registry setup and governance
- **Stream Processing** - Kafka Streams and ksqlDB implementations
- **Connector Development** - Custom Kafka Connect solutions

### 🎓 Training Programs

**Kafka Fundamentals**
- Core concepts and architecture
- Producer and consumer development
- Operations and monitoring
- *Duration: 2-3 days*

**Advanced Kafka Operations**
- Performance tuning and troubleshooting
- Security and compliance
- Advanced configuration management
- *Duration: 2-3 days*

**Custom Training**
- Tailored to your specific use cases
- Hands-on labs with your data
- Team-specific best practices
- *Duration: Flexible*

### 📞 Contact Information

**Get in touch for:**
- Enterprise support plans
- Consulting engagements
- Training programs
- Custom development

**Contact methods:**
- **Website:** [softwaremill.com/contact](https://softwaremill.com/contact/?topic=kafka)
- **Email:** [hello@softwaremill.com](mailto:hello@softwaremill.com?subject=KafkaPilot%20Support)
- **Form:** [Kafka Consulting Inquiry](https://softwaremill.com/contact/?topic=kafka)

**Response times:**
- General inquiries: 1 business day
- Enterprise customers: 4 hours (business hours)
- Critical issues: 1 hour (24/7 for enterprise)

---

## Frequently Asked Questions

### Installation & Setup

**Q: Do I need to install anything on my Kafka brokers?**  
A: No, KafkaPilot only requires SSH access. All tools are run remotely through SSH connections.

**Q: What permissions does KafkaPilot need?**  
A: Read access to configuration files, log files, and ability to run system commands like `ps`, `df`, `free`. Most operations work with standard user permissions.

**Q: Can I run KafkaPilot on Windows?**  
A: Yes, through WSL (Windows Subsystem for Linux). Native Windows support is planned for future releases.

### Usage & Troubleshooting

**Q: Why isn't KafkaPilot discovering my brokers?**  
A: Check that `kafkactl` is installed on your bastion host and can connect to your cluster. Run with `--debug` for detailed logs.

**Q: How much data does KafkaPilot collect?**  
A: Typically 10-50MB per broker, depending on log file sizes and retention settings. Large clusters may generate 100-500MB total.

**Q: Is it safe to run in production?**  
A: Yes, KafkaPilot only performs read operations and doesn't modify any Kafka configurations or data.

**Q: How often should I run health checks?**  
A: For production clusters, daily automated checks are recommended, with on-demand scans during incidents.

### AI Analysis

**Q: Do I need an OpenAI API key?**  
A: Only for AI-powered analysis. Basic data collection and static analysis work without it.

**Q: What data is sent to OpenAI?**  
A: Configuration files, log excerpts, and metrics. No sensitive data like topic contents or user credentials.

**Q: Can I use other LLM providers?**  
A: Currently only OpenAI is supported. Support for other providers (Claude, local models) is planned.

### Integration & Automation

**Q: Can I integrate KafkaPilot with my monitoring system?**  
A: Yes, use the JSON report format to extract metrics and findings for your monitoring stack.

**Q: How do I automate KafkaPilot scans?**  
A: See our [CI/CD integration tutorial](tutorials.html#9-cicd-integration) for examples with GitHub Actions, Jenkins, and other platforms.

**Q: Can I create custom analysis tasks?**  
A: Yes, see the [custom analysis tasks tutorial](tutorials.html#8-custom-analysis-tasks) for details.

---

## Contributing

KafkaPilot is open source and welcomes contributions!

### Ways to Contribute

**🐛 Bug Reports**
- Use the [issue template](https://github.com/softwaremill/kafkapilot/issues/new?template=bug_report.md)
- Include debug logs and environment details
- Provide minimal reproduction steps

**💡 Feature Requests**
- Describe the use case and expected behavior
- Consider implementation complexity
- Check if similar features exist

**📝 Documentation**
- Fix typos and improve clarity
- Add examples and tutorials
- Translate to other languages

**💻 Code Contributions**
- Check [good first issues](https://github.com/softwaremill/kafkapilot/labels/good%20first%20issue)
- Follow the coding standards
- Include tests for new features

### Development Setup

```bash
# Clone and setup
git clone https://github.com/softwaremill/kafkapilot.git
cd kafkapilot

# Install development dependencies
cargo build

# Run tests
cargo test

# Check formatting
cargo fmt --check

# Run linter
cargo clippy
```

### Pull Request Process

1. **Fork** the repository
2. **Create** a feature branch
3. **Make** your changes with tests
4. **Run** the full test suite
5. **Submit** a pull request with description

---

## License & Legal

KafkaPilot is licensed under the **Apache 2.0 License**.

**What this means:**
- ✅ Free for commercial use
- ✅ Modify and distribute
- ✅ Private use
- ✅ Patent grant included
- ❗ Must include license notice
- ❗ Must include copyright notice

**Full license:** [Apache 2.0](https://github.com/softwaremill/kafkapilot/blob/main/LICENSE)

### Data Privacy

- KafkaPilot processes configuration and log data locally
- AI analysis sends anonymized data to OpenAI (when enabled)
- No telemetry or usage data is collected by default
- See our [Privacy Policy](https://softwaremill.com/privacy-policy/) for details

---

## Related Projects

**Other SoftwareMill Tools:**
- **[Tapir](https://github.com/softwaremill/tapir)** - Type-safe web API library for Scala
- **[Ox](https://github.com/softwaremill/ox)** - Safe direct-style concurrency library
- **[Elasticmq](https://github.com/softwaremill/elasticmq)** - Message queueing system

**Kafka Ecosystem:**
- **[Kafdrop](https://github.com/obsidiandynamics/kafdrop)** - Kafka web UI
- **[Kafka Manager](https://github.com/yahoo/CMAK)** - Cluster management tool
- **[Confluent Platform](https://www.confluent.io/)** - Complete Kafka platform

---

*Last updated: {{ site.time | date: "%B %d, %Y" }}*