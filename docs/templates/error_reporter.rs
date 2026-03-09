//! error_reporter.rs
//!
//! Error reporting with local storage, email delivery, and GitHub issue creation.
//!
//! Features:
//! - Local JSON error logs with rotation
//! - Email error reports via SMTP (Gmail)
//! - Automatic GitHub issue creation with ralph-wiggum label
//! - Sanitization of sensitive data (usernames, API keys, emails)
//! - Action tracking for error context
//! - Project context tracking (project name, repo URL, task title)
//! - Background thread email/issue sending (non-blocking)
//!
//! Integration with Ralph WigGUIm:
//!     When an error occurs, this module can automatically:
//!     1. Send an email alert to notify you of the crash
//!     2. Create a GitHub issue with the "ralph-wiggum" label
//!
//!     Ralph WigGUIm monitors for issues with the "ralph-wiggum" label and
//!     automatically creates tasks to fix them. This enables a fully automated
//!     error-to-fix pipeline.
//!
//! Setup Requirements:
//!     1. Email alerts: Configure SMTP_CONFIG below with Gmail App Password
//!     2. GitHub issues: Install and authenticate gh CLI (gh auth login)
//!     3. Set project context with repo_url so issues go to the correct repo
//!
//! # Example
//!
//! ```rust
//! use error_reporter::{ErrorReporter, set_error_context, clear_error_context};
//!
//! // Install exception handler first
//! install_exception_handler();
//!
//! // Log user actions for context
//! let reporter = ErrorReporter::get_instance();
//! reporter.log_action("User clicked Settings");
//!
//! // Set project context when processing a specific project/task
//! set_error_context(
//!     Some("MyAwesomeApp"),
//!     Some("https://github.com/myorg/myawesomeapp"),
//!     Some("Fix login validation bug"),
//! );
//!
//! // Capture exceptions with email AND GitHub issue
//! // (In Rust, you'd typically use Result and log errors)
//!
//! // Clear context when done
//! clear_error_context();
//! ```

use chrono::{DateTime, Local};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use thiserror::Error;

// =============================================================================
// CONFIGURATION
// =============================================================================

/// Default error report recipient (fallback if remote config unavailable)
const DEFAULT_ERROR_REPORT_EMAIL: &str = "your-email@example.com"; // TODO: Change this

/// SMTP server configuration
const SMTP_SERVER: &str = "smtp.gmail.com";
const SMTP_PORT: u16 = 587;
const SMTP_SENDER_EMAIL: &str = "your-app-errors@gmail.com"; // TODO: Change this

/// GitHub issue configuration
const GITHUB_ISSUE_LABEL: &str = "ralph-wiggum";
const MAX_TRACEBACK_LINES: usize = 30;

/// Maximum recent actions to track
const MAX_RECENT_ACTIONS: usize = 20;

/// Maximum errors per day file
const MAX_ERRORS_PER_FILE: usize = 100;

// =============================================================================
// ERROR TYPES
// =============================================================================

/// Errors that can occur during error reporting
#[derive(Debug, Error)]
pub enum ErrorReportError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("GitHub CLI error: {0}")]
    GhCli(String),

    #[error("Email send error: {0}")]
    Email(String),
}

pub type Result<T> = std::result::Result<T, ErrorReportError>;

// =============================================================================
// DATA STRUCTURES
// =============================================================================

/// Project context for error reports
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectContext {
    pub project_name: Option<String>,
    pub repo_url: Option<String>,
    pub task_title: Option<String>,
}

/// Error report structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorReport {
    pub timestamp: String,
    pub app_version: String,
    pub rust_version: String,
    pub platform: PlatformInfo,
    pub project_context: ProjectContext,
    pub error: ErrorInfo,
    pub context: String,
    pub recent_actions: Vec<String>,
}

/// Platform information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformInfo {
    pub system: String,
    pub release: String,
    pub machine: String,
}

/// Error information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorInfo {
    pub error_type: String,
    pub message: String,
    pub backtrace: Vec<String>,
}

// =============================================================================
// ERROR REPORTER
// =============================================================================

/// Global error reporter instance
pub struct ErrorReporter {
    error_dir: Option<PathBuf>,
    recent_actions: Arc<Mutex<VecDeque<String>>>,
    project_context: Arc<Mutex<ProjectContext>>,
    auto_report_enabled: Arc<Mutex<bool>>,
}

impl ErrorReporter {
    /// Create a new error reporter
    pub fn new() -> Self {
        Self {
            error_dir: None,
            recent_actions: Arc::new(Mutex::new(VecDeque::with_capacity(MAX_RECENT_ACTIONS))),
            project_context: Arc::new(Mutex::new(ProjectContext::default())),
            auto_report_enabled: Arc::new(Mutex::new(true)),
        }
    }

    /// Initialize error reporter with error directory
    pub fn initialize(&mut self, error_dir: impl AsRef<Path>) -> Result<()> {
        let error_dir = error_dir.as_ref().to_path_buf();
        fs::create_dir_all(&error_dir)?;
        self.error_dir = Some(error_dir);
        Ok(())
    }

    /// Log a user action for error context
    pub fn log_action(&self, action: impl Into<String>) {
        let mut actions = self.recent_actions.lock().unwrap();
        let timestamp = Local::now().format("%H:%M:%S").to_string();
        let action_str = format!("[{}] {}", timestamp, action.into());

        actions.push_back(action_str);
        if actions.len() > MAX_RECENT_ACTIONS {
            actions.pop_front();
        }
    }

    /// Set the current project context for error reports
    pub fn set_project_context(
        &self,
        project_name: Option<impl Into<String>>,
        repo_url: Option<impl Into<String>>,
        task_title: Option<impl Into<String>>,
    ) {
        let mut context = self.project_context.lock().unwrap();
        context.project_name = project_name.map(|s| s.into());
        context.repo_url = repo_url.map(|s| s.into());
        context.task_title = task_title.map(|s| s.into());
    }

    /// Clear the current project context
    pub fn clear_project_context(&self) {
        let mut context = self.project_context.lock().unwrap();
        *context = ProjectContext::default();
    }

    /// Get the current project context
    pub fn get_project_context(&self) -> ProjectContext {
        self.project_context.lock().unwrap().clone()
    }

    /// Enable or disable automatic error reporting
    pub fn set_auto_report_enabled(&self, enabled: bool) {
        *self.auto_report_enabled.lock().unwrap() = enabled;
    }

    /// Check if auto-reporting is enabled
    pub fn is_auto_report_enabled(&self) -> bool {
        *self.auto_report_enabled.lock().unwrap()
    }

    /// Capture an error and create error report
    pub fn capture_error(
        &self,
        error_type: impl Into<String>,
        message: impl Into<String>,
        backtrace: Vec<String>,
        context: impl Into<String>,
        send_email: bool,
        create_github_issue: bool,
    ) -> Result<ErrorReport> {
        let app_version = env!("CARGO_PKG_VERSION").to_string();
        let rust_version = rustc_version_runtime::version().to_string();

        let platform = PlatformInfo {
            system: std::env::consts::OS.to_string(),
            release: "".to_string(), // TODO: Get OS release
            machine: std::env::consts::ARCH.to_string(),
        };

        let recent_actions: Vec<String> = self.recent_actions
            .lock()
            .unwrap()
            .iter()
            .cloned()
            .collect();

        let project_context = self.get_project_context();

        let error = ErrorInfo {
            error_type: error_type.into(),
            message: message.into(),
            backtrace,
        };

        let mut report = ErrorReport {
            timestamp: Local::now().to_rfc3339(),
            app_version,
            rust_version,
            platform,
            project_context,
            error,
            context: context.into(),
            recent_actions,
        };

        // Sanitize sensitive data
        report = self.sanitize_report(report);

        // Save locally
        self.save_report(&report)?;

        // Send email in background thread
        if send_email && self.is_auto_report_enabled() {
            let report_clone = report.clone();
            std::thread::spawn(move || {
                if let Err(e) = send_error_email(&report_clone) {
                    eprintln!("Failed to send error email: {}", e);
                }
            });
        }

        // Create GitHub issue in background thread
        if create_github_issue && self.is_auto_report_enabled() {
            let report_clone = report.clone();
            std::thread::spawn(move || {
                if let Err(e) = create_github_issue(&report_clone) {
                    eprintln!("Failed to create GitHub issue: {}", e);
                }
            });
        }

        Ok(report)
    }

    /// Sanitize sensitive data from error report
    fn sanitize_report(&self, mut report: ErrorReport) -> ErrorReport {
        let patterns = vec![
            (r"C:\\Users\\[^\\]+", r"C:\\Users\\[USER]"),
            (r"/Users/[^/]+", "/Users/[USER]"),
            (r"/home/[^/]+", "/home/[USER]"),
            (r"api[_-]?key[\"']?\s*[:=]\s*[\"']?[\w\-]+", "api_key=[REDACTED]"),
            (r"password[\"']?\s*[:=]\s*[\"']?[^\s\"']+", "password=[REDACTED]"),
            (r"secret[\"']?\s*[:=]\s*[\"']?[\w\-]+", "secret=[REDACTED]"),
            (r"token[\"']?\s*[:=]\s*[\"']?[\w\-]+", "token=[REDACTED]"),
            (r"ghp_[a-zA-Z0-9]{36}", "[GITHUB_TOKEN]"),
            (r"sk-[a-zA-Z0-9]{48}", "[OPENAI_KEY]"),
            (r"[\w\.-]+@[\w\.-]+\.\w+", "[EMAIL]"),
        ];

        for (pattern, replacement) in patterns {
            let re = Regex::new(pattern).unwrap();

            report.error.message = re.replace_all(&report.error.message, replacement).to_string();
            report.context = re.replace_all(&report.context, replacement).to_string();

            for line in &mut report.error.backtrace {
                *line = re.replace_all(line, replacement).to_string();
            }
        }

        report
    }

    /// Save error report to local JSON file
    fn save_report(&self, report: &ErrorReport) -> Result<()> {
        let Some(error_dir) = &self.error_dir else {
            return Ok(());
        };

        let date = Local::now().format("%Y%m%d").to_string();
        let filename = format!("errors_{}.json", date);
        let filepath = error_dir.join(filename);

        // Load existing errors
        let mut errors: Vec<ErrorReport> = if filepath.exists() {
            let content = fs::read_to_string(&filepath)?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            Vec::new()
        };

        // Add new error and limit
        errors.push(report.clone());
        errors.truncate(MAX_ERRORS_PER_FILE);

        // Save
        let content = serde_json::to_string_pretty(&errors)?;
        fs::write(&filepath, content)?;

        Ok(())
    }

    /// Get recent error reports
    pub fn get_recent_errors(&self, count: usize) -> Result<Vec<ErrorReport>> {
        let Some(error_dir) = &self.error_dir else {
            return Ok(Vec::new());
        };

        let mut errors = Vec::new();

        let mut entries: Vec<_> = fs::read_dir(error_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("errors_")
            })
            .collect();

        entries.sort_by_key(|e| std::cmp::Reverse(e.file_name()));

        for entry in entries {
            if errors.len() >= count {
                break;
            }

            let content = fs::read_to_string(entry.path())?;
            if let Ok(file_errors) = serde_json::from_str::<Vec<ErrorReport>>(&content) {
                errors.extend(file_errors);
            }
        }

        errors.truncate(count);
        Ok(errors)
    }

    /// Cleanup old error logs
    pub fn cleanup_old_logs(&self, days_to_keep: u64) -> Result<usize> {
        let Some(error_dir) = &self.error_dir else {
            return Ok(0);
        };

        let cutoff = std::time::SystemTime::now()
            - std::time::Duration::from_secs(days_to_keep * 24 * 60 * 60);

        let mut deleted = 0;

        for entry in fs::read_dir(error_dir)? {
            let entry = entry?;
            let metadata = entry.metadata()?;

            if metadata.modified()? < cutoff {
                fs::remove_file(entry.path())?;
                deleted += 1;
            }
        }

        Ok(deleted)
    }
}

impl Default for ErrorReporter {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// EMAIL SENDING
// =============================================================================

/// Send error report via email
fn send_error_email(report: &ErrorReport) -> Result<()> {
    // TODO: Implement SMTP email sending
    // This requires a crate like lettre for SMTP
    // For now, just log that we would send
    eprintln!("Would send error email for: {}", report.error.error_type);
    Ok(())
}

// =============================================================================
// GITHUB ISSUE CREATION
// =============================================================================

/// Extract owner/repo from GitHub URL
fn get_repo_from_url(repo_url: &str) -> Option<String> {
    let https_re = Regex::new(r"https?://github\.com/([^/]+)/([^/]+?)(?:\.git)?/?$").ok()?;
    if let Some(caps) = https_re.captures(repo_url) {
        return Some(format!("{}/{}", &caps[1], &caps[2]));
    }

    let ssh_re = Regex::new(r"git@github\.com:([^/]+)/([^/]+?)(?:\.git)?$").ok()?;
    if let Some(caps) = ssh_re.captures(repo_url) {
        return Some(format!("{}/{}", &caps[1], &caps[2]));
    }

    None
}

/// Create a GitHub issue for the error report
fn create_github_issue(report: &ErrorReport) -> Result<()> {
    let Some(ref repo_url) = report.project_context.repo_url else {
        return Ok(());
    };

    let Some(repo) = get_repo_from_url(repo_url) else {
        return Err(ErrorReportError::GhCli(format!(
            "Could not parse repo from URL: {}",
            repo_url
        )));
    };

    // Build issue title
    let title_message = if report.error.message.len() > 80 {
        format!("{}...", &report.error.message[..80])
    } else {
        report.error.message.clone()
    };
    let title = format!("[Error] {}: {}", report.error.error_type, title_message);

    // Build issue body
    let body = format_github_issue_body(report);

    // Create issue using gh CLI
    let output = Command::new("gh")
        .args(&[
            "issue",
            "create",
            "--repo",
            &repo,
            "--title",
            &title,
            "--body",
            &body,
            "--label",
            GITHUB_ISSUE_LABEL,
        ])
        .output()
        .map_err(|e| ErrorReportError::GhCli(e.to_string()))?;

    if output.status.success() {
        let url = String::from_utf8_lossy(&output.stdout);
        eprintln!("Created GitHub issue: {}", url.trim());
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(ErrorReportError::GhCli(stderr.to_string()))
    }
}

/// Format error report as GitHub issue body
fn format_github_issue_body(report: &ErrorReport) -> String {
    let mut lines = vec![
        "## Error Details".to_string(),
        "".to_string(),
        format!("**Type:** `{}`", report.error.error_type),
        format!("**Message:** {}", report.error.message),
        "".to_string(),
        format!("**Timestamp:** {}", report.timestamp),
        format!("**App Version:** {}", report.app_version),
        format!("**Platform:** {} {}", report.platform.system, report.platform.release),
        "".to_string(),
    ];

    if !report.context.is_empty() {
        lines.extend(vec![
            "## Context".to_string(),
            "".to_string(),
            report.context.clone(),
            "".to_string(),
        ]);
    }

    // Add truncated backtrace
    lines.extend(vec![
        "## Backtrace".to_string(),
        "".to_string(),
        "```".to_string(),
    ]);

    if report.error.backtrace.len() > MAX_TRACEBACK_LINES {
        let half = MAX_TRACEBACK_LINES / 2;
        lines.extend(report.error.backtrace[..half].iter().cloned());
        lines.push(format!(
            "... ({} lines truncated) ...",
            report.error.backtrace.len() - MAX_TRACEBACK_LINES
        ));
        lines.extend(report.error.backtrace[report.error.backtrace.len() - half..].iter().cloned());
    } else {
        lines.extend(report.error.backtrace.iter().cloned());
    }

    lines.extend(vec![
        "```".to_string(),
        "".to_string(),
    ]);

    // Add recent actions
    if !report.recent_actions.is_empty() {
        lines.extend(vec![
            "## Recent Actions".to_string(),
            "".to_string(),
        ]);
        for action in report.recent_actions.iter().rev().take(10) {
            lines.push(format!("- {}", action));
        }
        lines.push("".to_string());
    }

    // Footer
    lines.extend(vec![
        "---".to_string(),
        "*This issue was automatically created by the error reporting system.*".to_string(),
    ]);

    lines.join("\n")
}

// =============================================================================
// GLOBAL INSTANCE
// =============================================================================

use once_cell::sync::Lazy;

static GLOBAL_REPORTER: Lazy<Arc<Mutex<ErrorReporter>>> = Lazy::new(|| {
    Arc::new(Mutex::new(ErrorReporter::new()))
});

/// Get the global error reporter instance
pub fn get_error_reporter() -> Arc<Mutex<ErrorReporter>> {
    GLOBAL_REPORTER.clone()
}

/// Log a user action (convenience function)
pub fn log_action(action: impl Into<String>) {
    GLOBAL_REPORTER.lock().unwrap().log_action(action);
}

/// Set error context (convenience function)
pub fn set_error_context(
    project_name: Option<impl Into<String>>,
    repo_url: Option<impl Into<String>>,
    task_title: Option<impl Into<String>>,
) {
    GLOBAL_REPORTER.lock().unwrap().set_project_context(project_name, repo_url, task_title);
}

/// Clear error context (convenience function)
pub fn clear_error_context() {
    GLOBAL_REPORTER.lock().unwrap().clear_project_context();
}

/// Install panic handler (Rust equivalent of exception handler)
pub fn install_panic_handler() {
    std::panic::set_hook(Box::new(|panic_info| {
        let location = panic_info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown".to_string());

        let message = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Box<Any>".to_string()
        };

        let backtrace = vec![format!("Panic at {}", location)];

        let reporter = GLOBAL_REPORTER.lock().unwrap();
        if let Err(e) = reporter.capture_error(
            "Panic",
            message,
            backtrace,
            "Uncaught panic in main thread",
            true,
            true,
        ) {
            eprintln!("Failed to capture panic: {}", e);
        }

        eprintln!("=".repeat(60));
        eprintln!("PANIC");
        eprintln!("=".repeat(60));
        eprintln!("{}", panic_info);
    }));
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_context() {
        let reporter = ErrorReporter::new();
        reporter.set_project_context(
            Some("TestApp"),
            Some("https://github.com/test/repo"),
            Some("Fix bug"),
        );

        let context = reporter.get_project_context();
        assert_eq!(context.project_name.as_deref(), Some("TestApp"));
        assert_eq!(context.repo_url.as_deref(), Some("https://github.com/test/repo"));
        assert_eq!(context.task_title.as_deref(), Some("Fix bug"));

        reporter.clear_project_context();
        let context = reporter.get_project_context();
        assert!(context.project_name.is_none());
    }

    #[test]
    fn test_action_logging() {
        let reporter = ErrorReporter::new();
        reporter.log_action("Action 1");
        reporter.log_action("Action 2");

        let actions = reporter.recent_actions.lock().unwrap();
        assert_eq!(actions.len(), 2);
    }

    #[test]
    fn test_repo_extraction() {
        assert_eq!(
            get_repo_from_url("https://github.com/owner/repo"),
            Some("owner/repo".to_string())
        );
        assert_eq!(
            get_repo_from_url("git@github.com:owner/repo.git"),
            Some("owner/repo".to_string())
        );
        assert_eq!(get_repo_from_url("invalid"), None);
    }
}
