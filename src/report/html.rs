use super::{ReportGenerator, ReportResult, ReportError};
use crate::snapshot::format::{Finding, Severity, Snapshot, Category};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use chrono::Utc;

/// HTML report generator with static assets support
pub struct HtmlReporter {
    include_interactive_filters: bool,
    include_dark_mode: bool,
}

impl HtmlReporter {
    pub fn new() -> Self {
        Self {
            include_interactive_filters: true,
            include_dark_mode: true,
        }
    }

    pub fn with_interactive_filters(mut self, include: bool) -> Self {
        self.include_interactive_filters = include;
        self
    }

    pub fn with_dark_mode(mut self, include: bool) -> Self {
        self.include_dark_mode = include;
        self
    }

    /// Generate HTML report and save it along with assets to a directory
    pub fn save_report(&self, snapshot: &Snapshot, findings: &[Finding], output_path: &Path) -> ReportResult<()> {
        // Determine if output_path is a directory or file
        let (report_dir, report_file) = if output_path.extension().is_some() {
            // It's a file path, use parent directory
            let dir = output_path.parent()
                .ok_or_else(|| ReportError::Other("Invalid output path".to_string()))?;
            (dir.to_path_buf(), output_path.file_name().unwrap().to_str().unwrap().to_string())
        } else {
            // It's a directory path
            (output_path.to_path_buf(), "index.html".to_string())
        };

        // Create report directory if it doesn't exist
        fs::create_dir_all(&report_dir)?;

        // Create assets directory
        let assets_dir = report_dir.join("assets");
        fs::create_dir_all(&assets_dir)?;

        // Copy static assets
        self.copy_static_assets(&assets_dir)?;

        // Generate HTML content
        let html_content = self.generate_html(snapshot, findings)?;

        // Write HTML file
        let html_path = report_dir.join(&report_file);
        let mut file = File::create(&html_path)?;
        file.write_all(html_content.as_bytes())?;

        Ok(())
    }

    /// Copy static CSS and JS files to the assets directory
    fn copy_static_assets(&self, assets_dir: &Path) -> ReportResult<()> {
        // Create CSS file
        let css_content = self.generate_css();
        let css_path = assets_dir.join("kafkapilot.css");
        let mut css_file = File::create(css_path)?;
        css_file.write_all(css_content.as_bytes())?;

        // Create JavaScript file
        let js_content = self.generate_javascript();
        let js_path = assets_dir.join("kafkapilot.js");
        let mut js_file = File::create(js_path)?;
        js_file.write_all(js_content.as_bytes())?;

        Ok(())
    }

    /// Generate the HTML content
    fn generate_html(&self, snapshot: &Snapshot, findings: &[Finding]) -> ReportResult<String> {
        let mut html = String::new();

        // HTML header
        html.push_str("<!DOCTYPE html>\n");
        html.push_str("<html lang=\"en\">\n");
        html.push_str("<head>\n");
        html.push_str("    <meta charset=\"UTF-8\">\n");
        html.push_str("    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n");
        html.push_str("    <title>Kafka Cluster Health Report - KafkaPilot</title>\n");
        html.push_str("    <link rel=\"stylesheet\" href=\"assets/kafkapilot.css\">\n");
        html.push_str("</head>\n");
        html.push_str("<body>\n");

        // Main container
        html.push_str("    <div class=\"container\">\n");

        // Header section
        html.push_str("        <header>\n");
        html.push_str("            <h1>Kafka Cluster Health Report</h1>\n");
        html.push_str(&format!("            <div class=\"meta-info\">\n"));
        html.push_str(&format!("                <span class=\"tool-version\">KafkaPilot v{}</span>\n", snapshot.metadata.tool_version));
        html.push_str(&format!("                <span class=\"report-date\">📅 {}</span>\n", Utc::now().format("%Y-%m-%d %H:%M:%S UTC")));
        if self.include_dark_mode {
            html.push_str("                <button id=\"theme-toggle\" class=\"theme-toggle\" aria-label=\"Toggle dark mode\">🌙</button>\n");
        }
        html.push_str("            </div>\n");
        html.push_str("        </header>\n");

        // Navigation
        html.push_str("        <nav class=\"navigation\">\n");
        html.push_str("            <ul>\n");
        html.push_str("                <li><a href=\"#summary\">Summary</a></li>\n");
        html.push_str("                <li><a href=\"#cluster-info\">Cluster Info</a></li>\n");
        html.push_str("                <li><a href=\"#health-score\">Health Score</a></li>\n");
        html.push_str("                <li><a href=\"#findings\">Findings</a></li>\n");
        html.push_str("                <li><a href=\"#recommendations\">Recommendations</a></li>\n");
        html.push_str("            </ul>\n");
        html.push_str("        </nav>\n");

        // Executive Summary
        html.push_str("        <section id=\"summary\" class=\"card\">\n");
        html.push_str("            <h2>Executive Summary</h2>\n");
        
        let (critical, high, medium, low, info) = self.count_severities(findings);
        let total = findings.len();
        
        if total == 0 {
            html.push_str("            <div class=\"alert alert-success\">\n");
            html.push_str("                ✅ <strong>No issues found!</strong> Your Kafka cluster appears to be healthy.\n");
            html.push_str("            </div>\n");
        } else {
            html.push_str("            <div class=\"alert alert-warning\">\n");
            html.push_str(&format!("                ⚠️ <strong>{} issue(s) detected</strong> in your Kafka cluster requiring attention.\n", total));
            html.push_str("            </div>\n");
            
            if critical > 0 || high > 0 {
                html.push_str("            <div class=\"alert alert-danger\">\n");
                html.push_str(&format!("                🚨 <strong>Immediate action required:</strong> {} critical and {} high severity issues found.\n", critical, high));
                html.push_str("            </div>\n");
            }
        }

        // Severity breakdown
        html.push_str("            <div class=\"severity-breakdown\">\n");
        html.push_str("                <h3>Severity Breakdown</h3>\n");
        html.push_str("                <div class=\"severity-stats\">\n");
        html.push_str(&format!("                    <div class=\"stat critical\"><span class=\"count\">{}</span><span class=\"label\">Critical</span></div>\n", critical));
        html.push_str(&format!("                    <div class=\"stat high\"><span class=\"count\">{}</span><span class=\"label\">High</span></div>\n", high));
        html.push_str(&format!("                    <div class=\"stat medium\"><span class=\"count\">{}</span><span class=\"label\">Medium</span></div>\n", medium));
        html.push_str(&format!("                    <div class=\"stat low\"><span class=\"count\">{}</span><span class=\"label\">Low</span></div>\n", low));
        html.push_str(&format!("                    <div class=\"stat info\"><span class=\"count\">{}</span><span class=\"label\">Info</span></div>\n", info));
        html.push_str("                </div>\n");
        html.push_str("            </div>\n");
        html.push_str("        </section>\n");

        // Cluster Information
        html.push_str("        <section id=\"cluster-info\" class=\"card\">\n");
        html.push_str("            <h2>Cluster Information</h2>\n");
        html.push_str("            <table class=\"info-table\">\n");
        html.push_str("                <tbody>\n");
        
        if let Some(id) = &snapshot.cluster.id {
            html.push_str(&format!("                    <tr><td><strong>Cluster ID</strong></td><td>{}</td></tr>\n", id));
        }
        
        // Add broker count if available from admin data
        if let Some(admin) = &snapshot.collectors.admin {
            if let Some(brokers) = admin.get("brokers") {
                if let Some(brokers_array) = brokers.as_array() {
                    html.push_str(&format!("                    <tr><td><strong>Total Brokers</strong></td><td>{}</td></tr>\n", 
                        brokers_array.len()));
                }
            }
        }
        
        if let Some(admin) = &snapshot.collectors.admin {
            if let Some(topics) = admin.get("topics") {
                if let Some(topics_array) = topics.as_array() {
                    html.push_str(&format!("                    <tr><td><strong>Total Topics</strong></td><td>{}</td></tr>\n", 
                        topics_array.len()));
                    
                    // Count total partitions
                    let mut total_partitions = 0;
                    for topic in topics_array {
                        if let Some(partitions) = topic.get("partitions") {
                            if let Some(partitions_array) = partitions.as_array() {
                                total_partitions += partitions_array.len();
                            }
                        }
                    }
                    html.push_str(&format!("                    <tr><td><strong>Total Partitions</strong></td><td>{}</td></tr>\n", 
                        total_partitions));
                }
            }
        }
        
        html.push_str(&format!("                    <tr><td><strong>Collection Date</strong></td><td>{}</td></tr>\n", 
            snapshot.timestamp));
        html.push_str(&format!("                    <tr><td><strong>Tool Version</strong></td><td>{}</td></tr>\n",
            snapshot.metadata.tool_version));
        
        html.push_str("                </tbody>\n");
        html.push_str("            </table>\n");
        html.push_str("        </section>\n");

        // Health Score
        html.push_str("        <section id=\"health-score\" class=\"card\">\n");
        html.push_str("            <h2>Health Score</h2>\n");
        
        let health_score = self.calculate_health_score(findings);
        let score_class = if health_score >= 90.0 { "excellent" }
            else if health_score >= 75.0 { "good" }
            else if health_score >= 50.0 { "fair" }
            else { "poor" };
        
        html.push_str(&format!("            <div class=\"health-score {}\">\n", score_class));
        html.push_str(&format!("                <div class=\"score-value\">{:.1}%</div>\n", health_score));
        html.push_str(&format!("                <div class=\"score-label\">{}</div>\n", 
            if health_score >= 90.0 { "Excellent" }
            else if health_score >= 75.0 { "Good" }
            else if health_score >= 50.0 { "Fair" }
            else { "Poor" }
        ));
        html.push_str("            </div>\n");
        
        html.push_str("            <div class=\"health-details\">\n");
        html.push_str("                <p>The health score is calculated based on the severity and number of issues found:</p>\n");
        html.push_str("                <ul>\n");
        html.push_str("                    <li>Critical issues: -20 points each</li>\n");
        html.push_str("                    <li>High severity: -10 points each</li>\n");
        html.push_str("                    <li>Medium severity: -5 points each</li>\n");
        html.push_str("                    <li>Low severity: -2 points each</li>\n");
        html.push_str("                    <li>Info: -0.5 points each</li>\n");
        html.push_str("                </ul>\n");
        html.push_str("            </div>\n");
        html.push_str("        </section>\n");

        // Findings
        html.push_str("        <section id=\"findings\" class=\"card\">\n");
        html.push_str("            <h2>Detailed Findings</h2>\n");
        
        if self.include_interactive_filters {
            html.push_str("            <div class=\"filters\">\n");
            html.push_str("                <label>Filter by severity:</label>\n");
            html.push_str("                <select id=\"severity-filter\">\n");
            html.push_str("                    <option value=\"all\">All</option>\n");
            html.push_str("                    <option value=\"critical\">Critical</option>\n");
            html.push_str("                    <option value=\"high\">High</option>\n");
            html.push_str("                    <option value=\"medium\">Medium</option>\n");
            html.push_str("                    <option value=\"low\">Low</option>\n");
            html.push_str("                    <option value=\"info\">Info</option>\n");
            html.push_str("                </select>\n");
            
            html.push_str("                <label>Filter by category:</label>\n");
            html.push_str("                <select id=\"category-filter\">\n");
            html.push_str("                    <option value=\"all\">All</option>\n");
            html.push_str("                    <option value=\"cluster-hygiene\">Cluster Hygiene</option>\n");
            html.push_str("                    <option value=\"performance\">Performance</option>\n");
            html.push_str("                    <option value=\"configuration\">Configuration</option>\n");
            html.push_str("                    <option value=\"security\">Security</option>\n");
            html.push_str("                    <option value=\"availability\">Availability</option>\n");
            html.push_str("                    <option value=\"client\">Client</option>\n");
            html.push_str("                    <option value=\"capacity\">Capacity</option>\n");
            html.push_str("                </select>\n");
            html.push_str("            </div>\n");
        }
        
        html.push_str("            <div class=\"findings-list\">\n");
        
        for finding in findings {
            let severity_class = match finding.severity {
                Severity::Critical => "critical",
                Severity::High => "high",
                Severity::Medium => "medium",
                Severity::Low => "low",
                Severity::Info => "info",
            };
            
            let category_class = self.format_category_class(finding.category.clone());
            
            html.push_str(&format!("                <div class=\"finding {} {}\" data-severity=\"{}\" data-category=\"{}\">\n", 
                severity_class, category_class, severity_class, category_class));
            html.push_str(&format!("                    <div class=\"finding-header\">\n"));
            html.push_str(&format!("                        <span class=\"severity-badge {}\">{}</span>\n", 
                severity_class, self.format_severity(finding.severity.clone())));
            html.push_str(&format!("                        <span class=\"category-badge\">{}</span>\n", 
                self.format_category(finding.category.clone())));
            html.push_str(&format!("                        <h3>{}</h3>\n", finding.title));
            html.push_str("                    </div>\n");
            html.push_str(&format!("                    <p class=\"description\">{}</p>\n", finding.description));
            
            // Show impact if available
            if !finding.impact.is_empty() {
                html.push_str("                    <div class=\"impact\">\n");
                html.push_str(&format!("                        <strong>Impact:</strong> {}\n", finding.impact));
                html.push_str("                    </div>\n");
            }
            
            // Show remediation steps
            if !finding.remediation.steps.is_empty() {
                html.push_str("                    <div class=\"remediation\">\n");
                html.push_str("                        <strong>Remediation:</strong>\n");
                html.push_str("                        <ol>\n");
                for step in &finding.remediation.steps {
                    html.push_str(&format!("                            <li>{}</li>\n", step.description));
                }
                html.push_str("                        </ol>\n");
                html.push_str("                    </div>\n");
            }
            
            // Show evidence if available
            let has_evidence = !finding.evidence.metrics.is_empty() || 
                               !finding.evidence.logs.is_empty() || 
                               !finding.evidence.configs.is_empty();
            
            if has_evidence {
                html.push_str("                    <details class=\"evidence\">\n");
                html.push_str("                        <summary>Evidence</summary>\n");
                html.push_str("                        <div class=\"evidence-content\">\n");
                
                if !finding.evidence.configs.is_empty() {
                    html.push_str("                            <strong>Configuration Issues:</strong>\n");
                    html.push_str("                            <ul>\n");
                    for config in &finding.evidence.configs {
                        html.push_str(&format!("                                <li>{}: {} = {} ({})</li>\n", 
                            config.resource_name, config.config_key, config.current_value, config.reason));
                    }
                    html.push_str("                            </ul>\n");
                }
                
                if !finding.evidence.logs.is_empty() {
                    html.push_str("                            <strong>Log Evidence:</strong>\n");
                    html.push_str("                            <ul>\n");
                    for log in &finding.evidence.logs {
                        html.push_str(&format!("                                <li>[{}] {} ({}x)</li>\n", 
                            log.level, log.message, log.count));
                    }
                    html.push_str("                            </ul>\n");
                }
                
                if !finding.evidence.metrics.is_empty() {
                    html.push_str("                            <strong>Metrics:</strong>\n");
                    html.push_str("                            <ul>\n");
                    for metric in &finding.evidence.metrics {
                        let unit = metric.unit.as_ref().map(|u| u.as_str()).unwrap_or("");
                        html.push_str(&format!("                                <li>{}: {:.2} {}</li>\n", 
                            metric.name, metric.value, unit));
                    }
                    html.push_str("                            </ul>\n");
                }
                
                html.push_str("                        </div>\n");
                html.push_str("                    </details>\n");
            }
            
            html.push_str("                </div>\n");
        }
        
        html.push_str("            </div>\n");
        html.push_str("        </section>\n");

        // Recommendations
        html.push_str("        <section id=\"recommendations\" class=\"card\">\n");
        html.push_str("            <h2>Recommendations</h2>\n");
        
        let recommendations = self.generate_recommendations(findings);
        if recommendations.is_empty() {
            html.push_str("            <p>No specific recommendations at this time.</p>\n");
        } else {
            html.push_str("            <ol>\n");
            for rec in recommendations {
                html.push_str(&format!("                <li>{}</li>\n", rec));
            }
            html.push_str("            </ol>\n");
        }
        
        html.push_str("        </section>\n");

        // Footer
        html.push_str("        <footer>\n");
        html.push_str(&format!("            <p>Generated by KafkaPilot v{} | {}</p>\n", 
            snapshot.metadata.tool_version, 
            Utc::now().format("%Y-%m-%d %H:%M:%S UTC")));
        html.push_str("        </footer>\n");

        html.push_str("    </div>\n");

        // JavaScript
        html.push_str("    <script src=\"assets/kafkapilot.js\"></script>\n");

        html.push_str("</body>\n");
        html.push_str("</html>\n");

        Ok(html)
    }

    /// Generate CSS content
    fn generate_css(&self) -> String {
        r#":root {
    --primary-color: #2563eb;
    --secondary-color: #10b981;
    --danger-color: #ef4444;
    --warning-color: #f59e0b;
    --info-color: #3b82f6;
    --background: #ffffff;
    --surface: #f9fafb;
    --text-primary: #1f2937;
    --text-secondary: #6b7280;
    --border-color: #e5e7eb;
    --shadow: 0 1px 3px 0 rgba(0, 0, 0, 0.1);
}

body.dark-mode {
    --background: #0f172a;
    --surface: #1e293b;
    --text-primary: #f1f5f9;
    --text-secondary: #94a3b8;
    --border-color: #334155;
    --shadow: 0 1px 3px 0 rgba(0, 0, 0, 0.5);
}

* {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
}

body {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif;
    background-color: var(--background);
    color: var(--text-primary);
    line-height: 1.6;
    transition: background-color 0.3s, color 0.3s;
}

.container {
    max-width: 1200px;
    margin: 0 auto;
    padding: 2rem;
}

header {
    background: var(--surface);
    padding: 2rem;
    border-radius: 8px;
    margin-bottom: 2rem;
    box-shadow: var(--shadow);
}

header h1 {
    font-size: 2.5rem;
    margin-bottom: 1rem;
    color: var(--primary-color);
}

.meta-info {
    display: flex;
    gap: 2rem;
    align-items: center;
    color: var(--text-secondary);
}

.theme-toggle {
    margin-left: auto;
    background: none;
    border: 1px solid var(--border-color);
    padding: 0.5rem 1rem;
    border-radius: 4px;
    cursor: pointer;
    font-size: 1.2rem;
    transition: all 0.3s;
}

.theme-toggle:hover {
    background: var(--primary-color);
    color: white;
    border-color: var(--primary-color);
}

.navigation {
    background: var(--surface);
    padding: 1rem;
    border-radius: 8px;
    margin-bottom: 2rem;
    box-shadow: var(--shadow);
}

.navigation ul {
    list-style: none;
    display: flex;
    gap: 2rem;
    justify-content: center;
    flex-wrap: wrap;
}

.navigation a {
    color: var(--text-primary);
    text-decoration: none;
    padding: 0.5rem 1rem;
    border-radius: 4px;
    transition: background 0.3s;
}

.navigation a:hover {
    background: var(--primary-color);
    color: white;
}

.card {
    background: var(--surface);
    padding: 2rem;
    border-radius: 8px;
    margin-bottom: 2rem;
    box-shadow: var(--shadow);
}

.card h2 {
    font-size: 1.8rem;
    margin-bottom: 1.5rem;
    color: var(--primary-color);
}

.alert {
    padding: 1rem 1.5rem;
    border-radius: 4px;
    margin-bottom: 1rem;
}

.alert-success {
    background: #d1fae5;
    color: #065f46;
    border-left: 4px solid #10b981;
}

.alert-warning {
    background: #fed7aa;
    color: #7c2d12;
    border-left: 4px solid #f59e0b;
}

.alert-danger {
    background: #fee2e2;
    color: #7f1d1d;
    border-left: 4px solid #ef4444;
}

body.dark-mode .alert-success {
    background: #064e3b;
    color: #6ee7b7;
}

body.dark-mode .alert-warning {
    background: #78350f;
    color: #fde68a;
}

body.dark-mode .alert-danger {
    background: #7f1d1d;
    color: #fca5a5;
}

.severity-breakdown {
    margin-top: 2rem;
}

.severity-stats {
    display: flex;
    gap: 2rem;
    margin: 1.5rem 0;
    flex-wrap: wrap;
}

.stat {
    flex: 1;
    min-width: 100px;
    text-align: center;
    padding: 1rem;
    border-radius: 8px;
    background: var(--background);
    border: 2px solid var(--border-color);
}

.stat .count {
    display: block;
    font-size: 2rem;
    font-weight: bold;
    margin-bottom: 0.5rem;
}

.stat.critical {
    border-color: #ef4444;
    color: #ef4444;
}

.stat.high {
    border-color: #f59e0b;
    color: #f59e0b;
}

.stat.medium {
    border-color: #eab308;
    color: #eab308;
}

.stat.low {
    border-color: #3b82f6;
    color: #3b82f6;
}

.stat.info {
    border-color: #6b7280;
    color: #6b7280;
}

.info-table {
    width: 100%;
    border-collapse: collapse;
}

.info-table td {
    padding: 0.75rem;
    border-bottom: 1px solid var(--border-color);
}

.info-table td:first-child {
    font-weight: 600;
    width: 40%;
}

.health-score {
    text-align: center;
    padding: 3rem;
    border-radius: 12px;
    margin: 2rem 0;
}

.health-score.excellent {
    background: linear-gradient(135deg, #10b981, #34d399);
    color: white;
}

.health-score.good {
    background: linear-gradient(135deg, #3b82f6, #60a5fa);
    color: white;
}

.health-score.fair {
    background: linear-gradient(135deg, #f59e0b, #fbbf24);
    color: white;
}

.health-score.poor {
    background: linear-gradient(135deg, #ef4444, #f87171);
    color: white;
}

.score-value {
    font-size: 4rem;
    font-weight: bold;
    margin-bottom: 0.5rem;
}

.score-label {
    font-size: 1.5rem;
    font-weight: 500;
}

.health-details {
    margin-top: 2rem;
    padding: 1.5rem;
    background: var(--background);
    border-radius: 8px;
}

.health-details ul {
    margin-left: 2rem;
    margin-top: 1rem;
}

.filters {
    display: flex;
    gap: 2rem;
    margin-bottom: 2rem;
    padding: 1rem;
    background: var(--background);
    border-radius: 8px;
    align-items: center;
    flex-wrap: wrap;
}

.filters label {
    font-weight: 600;
}

.filters select {
    padding: 0.5rem 1rem;
    border: 1px solid var(--border-color);
    border-radius: 4px;
    background: var(--surface);
    color: var(--text-primary);
    cursor: pointer;
}

.findings-list {
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
}

.finding {
    padding: 1.5rem;
    border-radius: 8px;
    border-left: 4px solid;
    background: var(--background);
    transition: all 0.3s;
}

.finding.hidden {
    display: none;
}

.finding.critical {
    border-color: #ef4444;
}

.finding.high {
    border-color: #f59e0b;
}

.finding.medium {
    border-color: #eab308;
}

.finding.low {
    border-color: #3b82f6;
}

.finding.info {
    border-color: #6b7280;
}

.finding-header {
    display: flex;
    align-items: center;
    gap: 1rem;
    margin-bottom: 1rem;
}

.finding-header h3 {
    flex: 1;
    font-size: 1.2rem;
    margin: 0;
}

.severity-badge {
    padding: 0.25rem 0.75rem;
    border-radius: 4px;
    font-size: 0.875rem;
    font-weight: 600;
    text-transform: uppercase;
}

.severity-badge.critical {
    background: #ef4444;
    color: white;
}

.severity-badge.high {
    background: #f59e0b;
    color: white;
}

.severity-badge.medium {
    background: #eab308;
    color: white;
}

.severity-badge.low {
    background: #3b82f6;
    color: white;
}

.severity-badge.info {
    background: #6b7280;
    color: white;
}

.category-badge {
    padding: 0.25rem 0.75rem;
    border-radius: 4px;
    font-size: 0.875rem;
    background: var(--surface);
    border: 1px solid var(--border-color);
}

.description {
    color: var(--text-secondary);
    margin-bottom: 1rem;
}

.impact,
.remediation {
    margin-top: 1rem;
    padding: 1rem;
    background: var(--surface);
    border-radius: 4px;
}

.impact,
.remediation {
    margin-top: 1rem;
    padding: 1rem;
    background: var(--surface);
    border-radius: 4px;
}

.remediation ol {
    margin-left: 1.5rem;
    margin-top: 0.5rem;
}

.evidence {
    margin-top: 1rem;
}

.evidence summary {
    cursor: pointer;
    padding: 0.5rem;
    background: var(--surface);
    border-radius: 4px;
    user-select: none;
}

.evidence-content {
    margin-top: 1rem;
    padding: 1rem;
    background: var(--background);
    border-radius: 4px;
}

.evidence-content ul {
    margin-left: 1.5rem;
    margin-top: 0.5rem;
}

.evidence-content strong {
    display: block;
    margin-top: 0.5rem;
}

footer {
    text-align: center;
    padding: 2rem;
    color: var(--text-secondary);
    border-top: 1px solid var(--border-color);
    margin-top: 3rem;
}

@media (max-width: 768px) {
    .container {
        padding: 1rem;
    }
    
    header h1 {
        font-size: 1.8rem;
    }
    
    .meta-info {
        flex-direction: column;
        align-items: flex-start;
        gap: 0.5rem;
    }
    
    .theme-toggle {
        margin-left: 0;
        margin-top: 1rem;
    }
    
    .navigation ul {
        flex-direction: column;
        gap: 0.5rem;
    }
    
    .severity-stats {
        flex-direction: column;
    }
    
    .stat {
        min-width: auto;
    }
    
    .filters {
        flex-direction: column;
        align-items: stretch;
    }
    
    .filters select {
        width: 100%;
    }
}

@media print {
    .theme-toggle,
    .navigation,
    .filters {
        display: none;
    }
    
    body {
        background: white;
        color: black;
    }
    
    .card {
        box-shadow: none;
        border: 1px solid #ddd;
    }
}"#.to_string()
    }

    /// Generate JavaScript content
    fn generate_javascript(&self) -> String {
        r#"// Dark mode toggle
const themeToggle = document.getElementById('theme-toggle');
if (themeToggle) {
    // Check for saved theme preference
    const savedTheme = localStorage.getItem('theme');
    if (savedTheme === 'dark' || (!savedTheme && window.matchMedia('(prefers-color-scheme: dark)').matches)) {
        document.body.classList.add('dark-mode');
        themeToggle.textContent = '☀️';
    }
    
    themeToggle.addEventListener('click', () => {
        document.body.classList.toggle('dark-mode');
        const isDark = document.body.classList.contains('dark-mode');
        themeToggle.textContent = isDark ? '☀️' : '🌙';
        localStorage.setItem('theme', isDark ? 'dark' : 'light');
    });
}

// Filtering functionality
const severityFilter = document.getElementById('severity-filter');
const categoryFilter = document.getElementById('category-filter');
const findings = document.querySelectorAll('.finding');

function filterFindings() {
    const selectedSeverity = severityFilter ? severityFilter.value : 'all';
    const selectedCategory = categoryFilter ? categoryFilter.value : 'all';
    
    findings.forEach(finding => {
        const severity = finding.dataset.severity;
        const category = finding.dataset.category;
        
        const severityMatch = selectedSeverity === 'all' || severity === selectedSeverity;
        const categoryMatch = selectedCategory === 'all' || category === selectedCategory;
        
        if (severityMatch && categoryMatch) {
            finding.classList.remove('hidden');
        } else {
            finding.classList.add('hidden');
        }
    });
    
    // Update count
    const visibleCount = document.querySelectorAll('.finding:not(.hidden)').length;
    updateFindingsCount(visibleCount);
}

function updateFindingsCount(count) {
    const heading = document.querySelector('#findings h2');
    if (heading) {
        const baseText = 'Detailed Findings';
        heading.textContent = `${baseText} (${count})`;
    }
}

if (severityFilter) {
    severityFilter.addEventListener('change', filterFindings);
}

if (categoryFilter) {
    categoryFilter.addEventListener('change', filterFindings);
}

// Smooth scrolling for navigation links
document.querySelectorAll('.navigation a').forEach(link => {
    link.addEventListener('click', function(e) {
        e.preventDefault();
        const targetId = this.getAttribute('href').substring(1);
        const targetElement = document.getElementById(targetId);
        if (targetElement) {
            targetElement.scrollIntoView({ behavior: 'smooth', block: 'start' });
        }
    });
});

// Highlight current section in navigation
function highlightCurrentSection() {
    const sections = document.querySelectorAll('section[id]');
    const navLinks = document.querySelectorAll('.navigation a');
    
    const observer = new IntersectionObserver((entries) => {
        entries.forEach(entry => {
            if (entry.isIntersecting) {
                const id = entry.target.getAttribute('id');
                navLinks.forEach(link => {
                    if (link.getAttribute('href') === `#${id}`) {
                        link.classList.add('active');
                    } else {
                        link.classList.remove('active');
                    }
                });
            }
        });
    }, {
        rootMargin: '-100px 0px -70% 0px'
    });
    
    sections.forEach(section => {
        observer.observe(section);
    });
}

// Initialize on page load
document.addEventListener('DOMContentLoaded', () => {
    highlightCurrentSection();
    updateFindingsCount(findings.length);
});

// Print functionality
window.addEventListener('beforeprint', () => {
    // Expand all details elements before printing
    document.querySelectorAll('details').forEach(details => {
        details.setAttribute('open', '');
    });
});

window.addEventListener('afterprint', () => {
    // Collapse details elements after printing
    document.querySelectorAll('details').forEach(details => {
        details.removeAttribute('open');
    });
});"#.to_string()
    }

    // Helper methods
    fn count_severities(&self, findings: &[Finding]) -> (usize, usize, usize, usize, usize) {
        let critical = findings.iter().filter(|f| f.severity == Severity::Critical).count();
        let high = findings.iter().filter(|f| f.severity == Severity::High).count();
        let medium = findings.iter().filter(|f| f.severity == Severity::Medium).count();
        let low = findings.iter().filter(|f| f.severity == Severity::Low).count();
        let info = findings.iter().filter(|f| f.severity == Severity::Info).count();
        (critical, high, medium, low, info)
    }

    fn calculate_health_score(&self, findings: &[Finding]) -> f64 {
        let mut score = 100.0;
        
        for finding in findings {
            match finding.severity {
                Severity::Critical => score -= 20.0,
                Severity::High => score -= 10.0,
                Severity::Medium => score -= 5.0,
                Severity::Low => score -= 2.0,
                Severity::Info => score -= 0.5,
            }
        }
        
        if score < 0.0 {
            0.0
        } else {
            score
        }
    }

    fn format_severity(&self, severity: Severity) -> &'static str {
        match severity {
            Severity::Critical => "Critical",
            Severity::High => "High",
            Severity::Medium => "Medium",
            Severity::Low => "Low",
            Severity::Info => "Info",
        }
    }

    fn format_category(&self, category: Category) -> &'static str {
        match category {
            Category::ClusterHygiene => "Cluster Hygiene",
            Category::Performance => "Performance",
            Category::Configuration => "Configuration",
            Category::Security => "Security",
            Category::Availability => "Availability",
            Category::Client => "Client",
            Category::Capacity => "Capacity",
            Category::Other => "Other",
        }
    }

    fn format_category_class(&self, category: Category) -> &'static str {
        match category {
            Category::ClusterHygiene => "cluster-hygiene",
            Category::Performance => "performance",
            Category::Configuration => "configuration",
            Category::Security => "security",
            Category::Availability => "availability",
            Category::Client => "client",
            Category::Capacity => "capacity",
            Category::Other => "other",
        }
    }

    fn generate_recommendations(&self, findings: &[Finding]) -> Vec<String> {
        let mut recommendations = Vec::new();
        
        let (critical, high, _, _, _) = self.count_severities(findings);
        
        if critical > 0 {
            recommendations.push("Address all critical issues immediately as they pose significant risk to cluster stability.".to_string());
        }
        
        if high > 0 {
            recommendations.push("Schedule maintenance to resolve high severity issues within the next maintenance window.".to_string());
        }
        
        // Check for specific category patterns
        let security_issues = findings.iter()
            .filter(|f| matches!(f.category, Category::Security))
            .count();
        
        if security_issues > 0 {
            recommendations.push("Review and strengthen security configurations to prevent unauthorized access.".to_string());
        }
        
        let performance_issues = findings.iter()
            .filter(|f| matches!(f.category, Category::Performance))
            .count();
        
        if performance_issues > 0 {
            recommendations.push("Analyze performance bottlenecks and consider scaling or optimization strategies.".to_string());
        }
        
        recommendations
    }
}

impl ReportGenerator for HtmlReporter {
    fn generate(&self, snapshot: &Snapshot, output_path: &Path) -> ReportResult<()> {
        self.save_report(snapshot, &snapshot.findings, output_path)
    }
    
    fn name(&self) -> &'static str {
        "HTML Report Generator"
    }
}

impl Default for HtmlReporter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::format::{Evidence, Remediation, RemediationStep, SnapshotMetadata, Snapshot};
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn create_test_snapshot() -> Snapshot {
        let metadata = SnapshotMetadata::new("1.0.0".to_string());
        let mut snapshot = Snapshot::new(metadata);
        
        // Add some test findings
        snapshot.findings = vec![
            Finding {
                id: "1".to_string(),
                severity: Severity::Critical,
                category: Category::Security,
                title: "Security vulnerability detected".to_string(),
                description: "Critical security issue found in cluster".to_string(),
                impact: "High risk of data breach".to_string(),
                evidence: Evidence {
                    metrics: vec![],
                    logs: vec![],
                    configs: vec![],
                    raw_data: None,
                },
                root_cause: Some("Misconfiguration".to_string()),
                remediation: Remediation {
                    steps: vec![
                        RemediationStep {
                            order: 1,
                            description: "Update security settings".to_string(),
                            command: Some("kafka-configs --alter".to_string()),
                            verification: None,
                            can_automate: true,
                        }
                    ],
                    script: None,
                    risk_level: crate::snapshot::format::RiskLevel::High,
                    requires_downtime: false,
                    estimated_duration_minutes: Some(30),
                    rollback_plan: None,
                },
                metadata: HashMap::new(),
            },
            Finding {
                id: "2".to_string(),
                severity: Severity::Medium,
                category: Category::Performance,
                title: "Performance degradation".to_string(),
                description: "Cluster showing signs of performance issues".to_string(),
                impact: "Reduced throughput".to_string(),
                evidence: Evidence {
                    metrics: vec![],
                    logs: vec![],
                    configs: vec![],
                    raw_data: None,
                },
                root_cause: None,
                remediation: Remediation {
                    steps: vec![],
                    script: None,
                    risk_level: crate::snapshot::format::RiskLevel::Low,
                    requires_downtime: false,
                    estimated_duration_minutes: None,
                    rollback_plan: None,
                },
                metadata: HashMap::new(),
            },
        ];
        
        snapshot
    }

    #[test]
    fn test_html_report_generation() {
        let reporter = HtmlReporter::new();
        let snapshot = create_test_snapshot();
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().join("report");
        
        // Generate report
        let result = reporter.save_report(&snapshot, &snapshot.findings, &output_path);
        assert!(result.is_ok(), "Failed to generate HTML report: {:?}", result.err());
        
        // Check that files were created
        assert!(output_path.join("index.html").exists(), "index.html was not created");
        assert!(output_path.join("assets").exists(), "assets directory was not created");
        assert!(output_path.join("assets/kafkapilot.css").exists(), "CSS file was not created");
        assert!(output_path.join("assets/kafkapilot.js").exists(), "JS file was not created");
        
        // Read and verify HTML content
        let html_content = fs::read_to_string(output_path.join("index.html")).unwrap();
        assert!(html_content.contains("Kafka Cluster Health Report"));
        assert!(html_content.contains("Security vulnerability detected"));
        assert!(html_content.contains("Performance degradation"));
    }

    #[test]
    fn test_html_with_file_path() {
        let reporter = HtmlReporter::new();
        let snapshot = create_test_snapshot();
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().join("custom_report.html");
        
        // Generate report with file path
        let result = reporter.save_report(&snapshot, &snapshot.findings, &output_path);
        assert!(result.is_ok(), "Failed to generate HTML report: {:?}", result.err());
        
        // Check that files were created in the parent directory
        assert!(temp_dir.path().join("custom_report.html").exists(), "custom_report.html was not created");
        assert!(temp_dir.path().join("assets").exists(), "assets directory was not created");
    }

    #[test]
    fn test_severity_counting() {
        let reporter = HtmlReporter::new();
        let findings = vec![
            Finding {
                severity: Severity::Critical,
                ..Default::default()
            },
            Finding {
                severity: Severity::Critical,
                ..Default::default()
            },
            Finding {
                severity: Severity::High,
                ..Default::default()
            },
            Finding {
                severity: Severity::Medium,
                ..Default::default()
            },
        ];
        
        let (critical, high, medium, low, info) = reporter.count_severities(&findings);
        assert_eq!(critical, 2);
        assert_eq!(high, 1);
        assert_eq!(medium, 1);
        assert_eq!(low, 0);
        assert_eq!(info, 0);
    }

    #[test]
    fn test_health_score_calculation() {
        let reporter = HtmlReporter::new();
        let findings = vec![
            Finding {
                severity: Severity::Critical,
                ..Default::default()
            },
            Finding {
                severity: Severity::High,
                ..Default::default()
            },
            Finding {
                severity: Severity::Medium,
                ..Default::default()
            },
        ];
        
        let score = reporter.calculate_health_score(&findings);
        // Critical: -20, High: -10, Medium: -5 = -35 from 100
        assert_eq!(score, 65.0);
    }
}
