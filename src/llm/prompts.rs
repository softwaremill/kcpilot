/// Predefined prompts for various LLM analysis tasks
pub const SYSTEM_PROMPT_BASE: &str = "You are KCPilot, an expert Kafka administrator and \
    distributed systems engineer with deep knowledge of Apache Kafka, JVM tuning, and \
    production operations. You provide accurate, actionable insights based on evidence.";

pub const LOG_ANALYSIS_PROMPT: &str = "Analyze the provided Kafka logs for:
1. Critical errors and warnings
2. Performance issues (GC pauses, slow requests, timeouts)
3. Replication problems (ISR changes, under-replicated partitions)
4. Resource constraints (memory, disk, network)
5. Security issues (authentication failures, SSL errors)
6. Unusual patterns or anomalies

Provide a structured analysis with:
- Summary of findings
- List of specific issues with severity
- Root cause analysis where possible
- Actionable recommendations
- Risk assessment";

pub const METRICS_ANALYSIS_PROMPT: &str = "Analyze the provided Kafka metrics for:
1. Performance bottlenecks
2. Resource utilization patterns
3. Throughput and latency trends
4. Consumer lag issues
5. Broker imbalances
6. Capacity planning needs

Focus on:
- Identifying anomalies
- Correlating related metrics
- Predicting future issues
- Optimization opportunities";

pub const CONFIG_REVIEW_PROMPT: &str = "Review the Kafka configuration for:
1. Security best practices
2. Performance optimization opportunities
3. High availability settings
4. Resource allocation appropriateness
5. Consistency across brokers
6. Common misconfigurations

Provide:
- Critical issues that need immediate attention
- Performance tuning recommendations
- Security hardening suggestions
- Best practices alignment";

pub const ROOT_CAUSE_PROMPT: &str = "Perform root cause analysis:
1. Analyze all symptoms and evidence
2. Identify correlations and patterns
3. Determine the most likely root cause
4. List contributing factors
5. Provide confidence level (0-100%)
6. Suggest verification steps
7. Recommend remediation approach";

pub const REMEDIATION_PROMPT: &str = "Generate a remediation plan that is:
1. Safe and idempotent
2. Minimally disruptive
3. Includes validation steps
4. Has rollback procedures
5. Estimates time and risk

Format as executable script with:
- Pre-flight checks
- Step-by-step commands
- Validation after each step
- Rollback instructions
- Post-remediation verification";

pub const CAPACITY_PLANNING_PROMPT: &str = "Analyze capacity and growth:
1. Current resource utilization
2. Growth trends and patterns
3. Bottleneck identification
4. Scaling recommendations
5. Timeline for action

Provide:
- Current capacity percentages
- Projected exhaustion dates
- Scaling strategy (vertical vs horizontal)
- Cost-benefit analysis
- Risk assessment";

pub const INTERACTIVE_TROUBLESHOOTING_PROMPT: &str = "Help troubleshoot the issue by:
1. Understanding the problem description
2. Asking clarifying questions if needed
3. Analyzing available data
4. Providing step-by-step diagnosis
5. Suggesting solutions with trade-offs
6. Explaining in clear, non-technical language when requested";

/// Generate a context-aware prompt
pub fn build_analysis_prompt(
    analysis_type: &str,
    context: &str,
    constraints: Option<&str>,
) -> String {
    let base_prompt = match analysis_type {
        "logs" => LOG_ANALYSIS_PROMPT,
        "metrics" => METRICS_ANALYSIS_PROMPT,
        "config" => CONFIG_REVIEW_PROMPT,
        "root_cause" => ROOT_CAUSE_PROMPT,
        "remediation" => REMEDIATION_PROMPT,
        "capacity" => CAPACITY_PLANNING_PROMPT,
        "interactive" => INTERACTIVE_TROUBLESHOOTING_PROMPT,
        _ => "Analyze the provided data and give insights.",
    };
    
    let mut prompt = format!("{}\n\nContext: {}", base_prompt, context);
    
    if let Some(constraints) = constraints {
        prompt.push_str(&format!("\n\nConstraints: {}", constraints));
    }
    
    prompt.push_str("\n\nProvide your analysis in a clear, structured format.");
    
    prompt
}

/// Prompts for specific Kafka issues
pub mod issue_specific {
    pub const UNDER_REPLICATED_PARTITIONS: &str = "Analyze under-replicated partitions:
    - Identify affected topics and partitions
    - Determine root cause (broker failure, network, disk, etc.)
    - Assess data loss risk
    - Provide recovery steps
    - Suggest preventive measures";
    
    pub const HIGH_CONSUMER_LAG: &str = "Analyze consumer lag issue:
    - Identify lagging consumer groups
    - Determine if it's producer-side or consumer-side
    - Check for rebalancing issues
    - Analyze processing bottlenecks
    - Provide tuning recommendations";
    
    pub const DISK_SPACE_ISSUES: &str = "Analyze disk space problem:
    - Current usage and growth rate
    - Identify large topics/partitions
    - Review retention policies
    - Check for log segment issues
    - Provide cleanup and optimization steps";
    
    pub const JVM_MEMORY_ISSUES: &str = "Analyze JVM memory problem:
    - Review GC patterns and pause times
    - Check heap usage and allocation
    - Identify memory leaks or pressure
    - Analyze workload characteristics
    - Provide JVM tuning recommendations";
    
    pub const NETWORK_ISSUES: &str = "Analyze network problems:
    - Check for timeouts and connection errors
    - Review network thread pool saturation
    - Analyze bandwidth utilization
    - Identify network partitions
    - Provide network tuning suggestions";
}

/// Templates for structured output
pub mod output_templates {
    pub const JSON_FINDING_TEMPLATE: &str = r#"{
        "id": "ISSUE-001",
        "severity": "high|medium|low|critical",
        "category": "performance|security|configuration|availability",
        "title": "Brief issue description",
        "description": "Detailed explanation",
        "impact": "Business impact description",
        "evidence": {
            "metrics": [],
            "logs": [],
            "configs": []
        },
        "root_cause": "Root cause analysis",
        "remediation": {
            "steps": [],
            "script": "Optional automation script",
            "risk_level": "low|medium|high",
            "estimated_duration_minutes": 30
        }
    }"#;
    
    pub const MARKDOWN_REPORT_TEMPLATE: &str = r#"# Kafka Cluster Analysis Report

## Executive Summary
[High-level summary of cluster health]

## Critical Issues
[List of critical issues requiring immediate attention]

## Warnings
[List of warnings that need attention soon]

## Recommendations
[Optimization and best practice recommendations]

## Detailed Findings
[Detailed analysis of each issue]

## Remediation Plan
[Step-by-step remediation instructions]
"#;
}
