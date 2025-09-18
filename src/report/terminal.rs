use super::{ReportGenerator, ReportResult};
use crate::snapshot::format::{Finding, Severity, Snapshot};
use colored::Colorize;
use std::path::Path;

/// Terminal formatting constants
const TERMINAL_WIDTH: usize = 80;
const SEPARATOR_WIDTH: usize = 40;

/// Terminal report generator for console output
pub struct TerminalReporter {
    verbose: bool,
    use_colors: bool,
}

impl Default for TerminalReporter {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalReporter {
    pub fn new() -> Self {
        Self {
            verbose: false,
            use_colors: true,
        }
    }
    
    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }
    
    pub fn with_colors(mut self, use_colors: bool) -> Self {
        self.use_colors = use_colors;
        self
    }
    
    pub fn print_snapshot(&self, snapshot: &Snapshot) -> ReportResult<()> {
        self.print_header()?;
        self.print_cluster_info(snapshot)?;
        self.print_summary(snapshot)?;
        self.print_findings(snapshot)?;
        self.print_footer()?;
        Ok(())
    }
    
    /// Report with external findings (e.g., from LLM analyzer)
    pub fn report(&self, snapshot: &Snapshot, findings: &[Finding]) -> ReportResult<()> {
        self.print_header()?;
        self.print_cluster_info(snapshot)?;
        self.print_summary_with_findings(snapshot, findings)?;
        self.print_findings_list(findings)?;
        self.print_footer()?;
        Ok(())
    }
    
    fn print_header(&self) -> ReportResult<()> {
        println!("\n{}", "═".repeat(TERMINAL_WIDTH).bright_blue());
        println!("{}", "KAFKAPILOT HEALTH REPORT".bright_white().bold());
        println!("{}", "═".repeat(TERMINAL_WIDTH).bright_blue());
        Ok(())
    }
    
    fn print_cluster_info(&self, snapshot: &Snapshot) -> ReportResult<()> {
        println!("\n{}", "📊 Cluster Information".bright_white().bold());
        println!("{}", "─".repeat(SEPARATOR_WIDTH).bright_black());
        
        if let Some(id) = &snapshot.cluster.id {
            println!("  Cluster ID:      {}", id.bright_cyan());
        }
        if let Some(name) = &snapshot.cluster.name {
            println!("  Cluster Name:    {}", name.bright_cyan());
        }
        if let Some(version) = &snapshot.cluster.version {
            println!("  Kafka Version:   {}", version.bright_cyan());
        }
        
        let mode_display = match &snapshot.cluster.mode {
            crate::snapshot::format::ClusterMode::Kraft => "KRaft (modern, Zookeeper-free)".bright_green(),
            crate::snapshot::format::ClusterMode::Zookeeper => "Zookeeper (legacy)".bright_yellow(),
            crate::snapshot::format::ClusterMode::Unknown => "Unknown".bright_red(),
        };
        println!("  Mode:            {}", mode_display);
        println!("  Timestamp:       {}", snapshot.timestamp.format("%Y-%m-%d %H:%M:%S UTC"));
        println!("  Tool Version:    {}", snapshot.metadata.tool_version);
        
        Ok(())
    }
    
    fn print_summary(&self, snapshot: &Snapshot) -> ReportResult<()> {
        println!("\n{}", "📈 Analysis Summary".bright_white().bold());
        println!("{}", "─".repeat(SEPARATOR_WIDTH).bright_black());
        
        let total_findings = snapshot.findings.len();
        let critical = snapshot.findings.iter().filter(|f| f.severity == Severity::Critical).count();
        let high = snapshot.findings.iter().filter(|f| f.severity == Severity::High).count();
        let medium = snapshot.findings.iter().filter(|f| f.severity == Severity::Medium).count();
        let low = snapshot.findings.iter().filter(|f| f.severity == Severity::Low).count();
        let info = snapshot.findings.iter().filter(|f| f.severity == Severity::Info).count();
        
        println!("  Total Findings:  {}", total_findings.to_string().bright_yellow());
        
        if critical > 0 {
            println!("  🔴  Critical:    {}", critical.to_string().bright_red().bold());
        }
        if high > 0 {
            println!("  🟠  High:        {}", high.to_string().bright_red());
        }
        if medium > 0 {
            println!("  🟡  Medium:      {}", medium.to_string().bright_yellow());
        }
        if low > 0 {
            println!("  🟢  Low:         {}", low.to_string().bright_green());
        }
        if info > 0 {
            println!("  ℹ️  Info:        {}", info.to_string().bright_blue());
        }
        
        // Calculate health score
        let health_score = self.calculate_health_score(snapshot);
        let score_color = if health_score >= 80.0 {
            "green"
        } else if health_score >= 60.0 {
            "yellow"
        } else {
            "red"
        };
        
        println!("\n  Health Score:    {}/100", 
                 format!("{:.0}", health_score).color(score_color).bold());
        
        Ok(())
    }
    
    /// Print summary with external findings (avoiding snapshot clone)
    fn print_summary_with_findings(&self, _snapshot: &Snapshot, findings: &[Finding]) -> ReportResult<()> {
        println!("\n{}", "📈 Analysis Summary".bright_white().bold());
        println!("{}", "─".repeat(SEPARATOR_WIDTH).bright_black());
        
        let total_findings = findings.len();
        let critical = findings.iter().filter(|f| f.severity == Severity::Critical).count();
        let high = findings.iter().filter(|f| f.severity == Severity::High).count();
        let medium = findings.iter().filter(|f| f.severity == Severity::Medium).count();
        let low = findings.iter().filter(|f| f.severity == Severity::Low).count();
        let info = findings.iter().filter(|f| f.severity == Severity::Info).count();
        
        println!("  Total Findings:  {}", total_findings.to_string().bright_yellow());
        
        if critical > 0 {
            println!("  🔴  Critical:    {}", critical.to_string().bright_red().bold());
        }
        if high > 0 {
            println!("  🟠  High:        {}", high.to_string().bright_red());
        }
        if medium > 0 {
            println!("  🟡  Medium:      {}", medium.to_string().bright_yellow());
        }
        if low > 0 {
            println!("  🟢  Low:         {}", low.to_string().bright_green());
        }
        if info > 0 {
            println!("  ℹ️  Info:        {}", info.to_string().bright_blue());
        }
        
        // Calculate health score based on findings
        let health_score = self.calculate_health_score_from_findings(findings);
        let score_color = if health_score >= 80.0 {
            "green"
        } else if health_score >= 60.0 {
            "yellow"
        } else {
            "red"
        };
        
        println!("\n  Health Score:    {}/100", 
                 format!("{:.0}", health_score).color(score_color).bold());
        
        Ok(())
    }
    
    /// Print findings list (avoiding snapshot clone)
    fn print_findings_list(&self, findings: &[Finding]) -> ReportResult<()> {
        if findings.is_empty() {
            println!("\n✅ {}", "No issues found! Your cluster appears healthy.".bright_green());
            return Ok(());
        }
        
        println!("\n{}", "🔍 Findings".bright_white().bold());
        println!("{}", "═".repeat(TERMINAL_WIDTH).bright_black());
        
        for (idx, finding) in findings.iter().enumerate() {
            self.print_finding(idx + 1, finding)?;
        }
        
        Ok(())
    }
    
    fn print_findings(&self, snapshot: &Snapshot) -> ReportResult<()> {
        if snapshot.findings.is_empty() {
            println!("\n✅ {}", "No issues found! Your cluster appears healthy.".bright_green());
            return Ok(());
        }
        
        println!("\n{}", "🔍 Findings".bright_white().bold());
        println!("{}", "═".repeat(TERMINAL_WIDTH).bright_black());
        
        for (idx, finding) in snapshot.findings.iter().enumerate() {
            self.print_finding(idx + 1, finding)?;
        }
        
        Ok(())
    }
    
    fn print_finding(&self, num: usize, finding: &Finding) -> ReportResult<()> {
        let severity_icon = finding.severity.icon();
        let severity_text = format!("{:?}", finding.severity);
        let severity_colored = match finding.severity {
            Severity::Critical => severity_text.bright_red().bold(),
            Severity::High => severity_text.bright_red(),
            Severity::Medium => severity_text.bright_yellow(),
            Severity::Low => severity_text.bright_green(),
            Severity::Info => severity_text.bright_blue(),
        };
        
        println!("\n{} Finding #{}: {}", severity_icon, num, finding.title.bright_white().bold());
        println!("  Severity:  {}", severity_colored);
        println!("  Category:  {:?}", finding.category);
        println!("  ID:        {}", finding.id.bright_black());
        
        println!("\n  {}", "Description:".underline());
        for line in finding.description.lines() {
            println!("    {}", line);
        }
        
        println!("\n  {}", "Impact:".underline());
        for line in finding.impact.lines() {
            println!("    {}", line.bright_yellow());
        }
        
        if let Some(root_cause) = &finding.root_cause {
            println!("\n  {}", "Root Cause:".underline());
            for line in root_cause.lines() {
                println!("    {}", line);
            }
        }
        
        // Print evidence summary
        if !finding.evidence.metrics.is_empty() || !finding.evidence.logs.is_empty() || !finding.evidence.configs.is_empty() {
            println!("\n  {}", "Evidence:".underline());
            
            // Print config evidence with source files
            for config in &finding.evidence.configs {
                println!("    • Config {}: {} = {}", 
                         config.config_key.bright_cyan(),
                         config.resource_name,
                         config.current_value.bright_yellow());
                if let Some(recommended) = &config.recommended_value {
                    println!("      Recommended: {}", recommended.bright_green());
                }
                if !config.source_files.is_empty() {
                    println!("      {}", "Affected files:".bright_red().underline());
                    for file in &config.source_files {
                        println!("        • {}", file.bright_white());
                    }
                }
            }
            
            for metric in &finding.evidence.metrics {
                println!("    • {}: {} {}", 
                         metric.name.bright_cyan(), 
                         metric.value, 
                         metric.unit.as_deref().unwrap_or(""));
            }
            for log in finding.evidence.logs.iter().take(2) {
                println!("    • {} ({}x): {}", 
                         log.level.bright_red(), 
                         log.count,
                         log.message.chars().take(80).collect::<String>());
            }
        }
        
        // Print remediation steps
        if !finding.remediation.steps.is_empty() {
            println!("\n  {}", "Remediation Steps:".underline().bright_green());
            for step in &finding.remediation.steps {
                println!("    {}. {}", step.order, step.description);
                if self.verbose {
                    if let Some(cmd) = &step.command {
                        println!("       Command: {}", cmd.bright_black());
                    }
                }
            }
            
            println!("\n  Risk Level: {:?} | Downtime Required: {}", 
                     finding.remediation.risk_level,
                     if finding.remediation.requires_downtime { "Yes".red() } else { "No".green() });
            
            if let Some(duration) = finding.remediation.estimated_duration_minutes {
                println!("  Estimated Duration: {} minutes", duration);
            }
        }
        
        println!("\n{}", "─".repeat(80).bright_black());
        
        Ok(())
    }
    
    fn print_footer(&self) -> ReportResult<()> {
        println!("\n{}", "💡 Next Steps".bright_white().bold());
        println!("{}", "─".repeat(SEPARATOR_WIDTH).bright_black());
        println!("  1. Address critical and high severity findings first");
        println!("  2. Review remediation scripts before applying");
        println!("  3. Test changes in a non-production environment");
        println!("  4. Monitor cluster after applying fixes");
        
        println!("\n{}", "═".repeat(TERMINAL_WIDTH).bright_blue());
        println!("{}", "Report generated by KafkaPilot".bright_black());
        println!("{}", "For support, visit: https://kafkapilot.io".bright_black());
        println!();
        
        Ok(())
    }
    
    fn calculate_health_score(&self, snapshot: &Snapshot) -> f64 {
        self.calculate_health_score_from_findings(&snapshot.findings)
    }
    
    fn calculate_health_score_from_findings(&self, findings: &[Finding]) -> f64 {
        let mut score: f64 = 100.0;
        
        for finding in findings {
            let penalty = match finding.severity {
                Severity::Critical => 25.0,
                Severity::High => 15.0,
                Severity::Medium => 8.0,
                Severity::Low => 3.0,
                Severity::Info => 0.0,
            };
            score -= penalty;
        }
        
        score.max(0.0)
    }
}

impl ReportGenerator for TerminalReporter {
    fn generate(&self, snapshot: &Snapshot, _output_path: &Path) -> ReportResult<()> {
        self.print_snapshot(snapshot)
    }
    
    fn name(&self) -> &'static str {
        "terminal"
    }
}
