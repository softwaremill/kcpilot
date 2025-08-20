# KafkaPilot Design Document

## Executive Summary

KafkaPilot is a CLI-first Kafka health diagnostics tool that automatically collects cluster signals, identifies issues, and provides actionable remediation guidance. The tool serves dual purposes: providing immediate value to users while serving as a lead generation channel for Kafka consulting services.

### Core Value Proposition
- **One-command health assessment** with zero configuration
- **Plain-language explanations** of complex Kafka issues
- **Actionable remediation scripts** with safe defaults
- **Enterprise-ready** with air-gapped mode and sensitive data redaction

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                        CLI Interface                        │
│                     (clap-based commands)                   │
└─────────────────────────────────────────────────────────────┘
                               │
┌─────────────────────────────────────────────────────────────┐
│                         Core Engine                         │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │
│  │   Snapshot   │  │    Rules     │  │   Reporting  │     │
│  │    Engine    │  │    Engine    │  │    Engine    │     │
│  └──────────────┘  └──────────────┘  └──────────────┘     │
└─────────────────────────────────────────────────────────────┘
                               │
┌─────────────────────────────────────────────────────────────┐
│                         Collectors                          │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐  │
│  │  Admin   │  │   JMX/   │  │   Logs   │  │  Cloud   │  │
│  │  Client  │  │   Prom   │  │          │  │ Metadata │  │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## Current Implementation Status

### Phase 1: MVP (Completed)
- ✅ Basic CLI structure with clap
- ✅ Command definitions (scan, watch, analyze, fix)
- ✅ SSH-based scan functionality for remote clusters
- ✅ Comprehensive data collection (configs, logs, metrics, system info)
- ✅ Output to timestamped directories
- ✅ Collection summary reports
- ⏳ Snapshot format (.kafsnap)
- ⏳ Analysis engine
- ⏳ Automated remediation

## Command Interface

### Implemented Commands

```bash
# Scan locally when running directly on bastion
kafkapilot scan

# Scan remotely via SSH bastion from ~/.ssh/config
kafkapilot scan --bastion kafka-poligon

# Scan with custom output directory
kafkapilot scan --output /path/to/output

# Analyze existing snapshot (planned)
kafkapilot analyze ./snapshot.kafsnap \
  --report terminal,html,markdown

# Watch cluster continuously (planned)
kafkapilot watch --interval 60 --alert

# Generate remediation scripts (planned)
kafkapilot fix ./snapshot.kafsnap --dry-run
```

## Project Structure

```
kafkapilot/
├── src/
│   ├── main.rs              # Entry point
│   ├── lib.rs               # Library exports
│   ├── cli/                 # Command-line interface
│   │   ├── mod.rs
│   │   └── commands.rs      # Command definitions
│   ├── scan/                # SSH-based scanning
│   │   ├── mod.rs           # Scanner orchestration
│   │   └── collector.rs     # Bastion/broker collectors
│   ├── collectors/          # Data collectors (legacy)
│   │   ├── mod.rs
│   │   ├── admin.rs         # Kafka AdminClient
│   │   └── logs.rs          # Log collection
│   ├── analyzers/           # Analysis engine
│   │   ├── mod.rs
│   │   └── rules.rs         # Rule engine
│   ├── snapshot/            # Snapshot management
│   │   ├── mod.rs
│   │   └── format.rs        # Data structures
│   └── report/              # Report generation
│       ├── mod.rs
│       └── terminal.rs      # Terminal output
```
