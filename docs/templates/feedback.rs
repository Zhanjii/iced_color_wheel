//! feedback.rs
//!
//! Feedback submission module for GitHub issue creation with Ralph WigGUIm integration.
//!
//! Features:
//! - GitHub issue creation with ralph-wiggum label
//! - Formatted feedback with category and priority
//! - CLI-based submission using `gh` command
//!
//! # Example
//!
//! ```rust
//! use feedback::{Feedback, FeedbackCategory, FeedbackPriority};
//!
//! let feedback = Feedback::new(
//!     FeedbackCategory::Feature,
//!     FeedbackPriority::Medium,
//!     "Add dark mode support",
//!     "The app should support a dark theme for better night-time usage.",
//! );
//!
//! match feedback.submit() {
//!     Ok(url) => println!("Issue created: {}", url),
//!     Err(e) => eprintln!("Failed to create issue: {}", e),
//! }
//! ```
//!
//! # Configuration
//!
//! Update the constants at the top of this file:
//! - `GITHUB_ISSUES_REPO`: Your GitHub repository in "owner/repo" format
//! - `RALPH_WIGGUM_LABEL`: The label for Ralph WigGUIm automation
//! - `APP_NAME`: Your application name

use std::fmt;
use std::process::Command;

// =============================================================================
// CONFIGURATION - Update these for your project
// =============================================================================

/// GitHub repository for issue creation (format: "owner/repo")
const GITHUB_ISSUES_REPO: &str = "YourUsername/YourRepo";

/// Label that triggers Ralph WigGUIm automation
const RALPH_WIGGUM_LABEL: &str = "ralph-wiggum";

/// Application name for issue metadata
const APP_NAME: &str = "YourApp";

// =============================================================================
// ENUMS
// =============================================================================

/// Feedback category type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedbackCategory {
    Bug,
    Feature,
    Feedback,
}

impl FeedbackCategory {
    /// Get the display name for this category
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Bug => "Bug Report",
            Self::Feature => "Feature Request",
            Self::Feedback => "General Feedback",
        }
    }

    /// Get the GitHub label for this category
    pub fn label(&self) -> &'static str {
        match self {
            Self::Bug => "bug",
            Self::Feature => "enhancement",
            Self::Feedback => "feedback",
        }
    }

    /// Get the title prefix for this category
    pub fn prefix(&self) -> &'static str {
        match self {
            Self::Bug => "[Bug]",
            Self::Feature => "[Feature]",
            Self::Feedback => "[Feedback]",
        }
    }
}

impl fmt::Display for FeedbackCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Feedback priority level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedbackPriority {
    Low,
    Medium,
    High,
    Critical,
}

impl FeedbackPriority {
    /// Get the display name for this priority
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
            Self::Critical => "Critical",
        }
    }
}

impl fmt::Display for FeedbackPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

// =============================================================================
// FEEDBACK STRUCT
// =============================================================================

/// Feedback submission data
#[derive(Debug, Clone)]
pub struct Feedback {
    /// Category of the feedback
    pub category: FeedbackCategory,
    /// Priority level
    pub priority: FeedbackPriority,
    /// Short title/summary
    pub title: String,
    /// Detailed description
    pub description: String,
    /// Optional additional context from Q&A
    pub additional_context: Option<Vec<(String, String)>>,
}

impl Feedback {
    /// Create a new feedback submission
    pub fn new(
        category: FeedbackCategory,
        priority: FeedbackPriority,
        title: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            category,
            priority,
            title: title.into(),
            description: description.into(),
            additional_context: None,
        }
    }

    /// Add additional Q&A context
    pub fn with_context(mut self, question: impl Into<String>, answer: impl Into<String>) -> Self {
        self.additional_context
            .get_or_insert_with(Vec::new)
            .push((question.into(), answer.into()));
        self
    }

    /// Get the formatted issue title
    pub fn issue_title(&self) -> String {
        let prefix = self.category.prefix();
        let mut full_title = format!("{} {}", prefix, self.title);

        // Truncate to 70 chars
        if full_title.len() > 70 {
            full_title.truncate(67);
            full_title.push_str("...");
        }

        full_title
    }

    /// Format the issue body in markdown
    pub fn issue_body(&self) -> String {
        let mut lines = Vec::new();

        // Header info
        lines.push(format!("**Category:** {}", self.category));
        lines.push(format!("**Priority:** {}", self.priority));
        lines.push(format!("**Source:** {}", APP_NAME));
        lines.push(format!(
            "**Submitted:** {}",
            chrono::Local::now().format("%Y-%m-%d %H:%M")
        ));
        lines.push(String::new());

        // Description
        lines.push("## Description".to_string());
        lines.push(String::new());
        lines.push(self.description.clone());
        lines.push(String::new());

        // Additional context if present
        if let Some(ref context) = self.additional_context {
            if !context.is_empty() {
                lines.push("## Additional Context".to_string());
                lines.push(String::new());
                for (question, answer) in context {
                    lines.push(format!("**Q:** {}", question));
                    lines.push(format!("**A:** {}", answer));
                    lines.push(String::new());
                }
            }
        }

        // Footer
        lines.push("---".to_string());
        lines.push(format!(
            "*Generated by {} Feedback System*",
            APP_NAME
        ));

        lines.join("\n")
    }

    /// Submit the feedback as a GitHub issue
    ///
    /// Requires `gh` CLI to be installed and authenticated.
    ///
    /// # Returns
    ///
    /// The URL of the created issue on success.
    ///
    /// # Errors
    ///
    /// Returns an error if `gh` CLI is not available or issue creation fails.
    pub fn submit(&self) -> Result<String, FeedbackError> {
        // Build labels
        let labels = format!("{},{}", RALPH_WIGGUM_LABEL, self.category.label());

        // Create issue using gh CLI
        let output = Command::new("gh")
            .args([
                "issue",
                "create",
                "--repo",
                GITHUB_ISSUES_REPO,
                "--title",
                &self.issue_title(),
                "--body",
                &self.issue_body(),
                "--label",
                &labels,
            ])
            .output()
            .map_err(|e| FeedbackError::CommandFailed(e.to_string()))?;

        if output.status.success() {
            let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Ok(url)
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Err(FeedbackError::IssueFailed(stderr))
        }
    }
}

// =============================================================================
// ERROR TYPE
// =============================================================================

/// Error type for feedback operations
#[derive(Debug)]
pub enum FeedbackError {
    /// GitHub CLI command failed to execute
    CommandFailed(String),
    /// Issue creation failed
    IssueFailed(String),
    /// GitHub CLI not available
    GhNotFound,
    /// Not authenticated
    NotAuthenticated,
}

impl fmt::Display for FeedbackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CommandFailed(msg) => write!(f, "Command failed: {}", msg),
            Self::IssueFailed(msg) => write!(f, "Issue creation failed: {}", msg),
            Self::GhNotFound => write!(f, "GitHub CLI (gh) not found. Install from: https://cli.github.com"),
            Self::NotAuthenticated => write!(f, "GitHub CLI not authenticated. Run: gh auth login"),
        }
    }
}

impl std::error::Error for FeedbackError {}

// =============================================================================
// UTILITIES
// =============================================================================

/// Check if GitHub CLI is installed and authenticated
pub fn check_gh_cli() -> Result<(), FeedbackError> {
    // Check if gh is installed
    let version = Command::new("gh")
        .arg("--version")
        .output()
        .map_err(|_| FeedbackError::GhNotFound)?;

    if !version.status.success() {
        return Err(FeedbackError::GhNotFound);
    }

    // Check authentication
    let auth = Command::new("gh")
        .args(["auth", "status"])
        .output()
        .map_err(|_| FeedbackError::GhNotFound)?;

    if !auth.status.success() {
        return Err(FeedbackError::NotAuthenticated);
    }

    Ok(())
}

/// Create a fallback report when GitHub CLI is unavailable
pub fn create_fallback_report(feedback: &Feedback) -> String {
    let mut lines = Vec::new();

    lines.push("=".repeat(60));
    lines.push("FEEDBACK REPORT".to_string());
    lines.push("=".repeat(60));
    lines.push(String::new());
    lines.push(format!("Title: {}", feedback.issue_title()));
    lines.push(String::new());
    lines.push("Please submit this as a GitHub issue at:".to_string());
    lines.push(format!(
        "https://github.com/{}/issues/new",
        GITHUB_ISSUES_REPO
    ));
    lines.push(String::new());
    lines.push("-".repeat(60));
    lines.push(feedback.issue_body());
    lines.push("-".repeat(60));

    lines.join("\n")
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feedback_title_truncation() {
        let feedback = Feedback::new(
            FeedbackCategory::Feature,
            FeedbackPriority::Medium,
            "This is a very long title that should be truncated to fit within seventy characters",
            "Description",
        );

        let title = feedback.issue_title();
        assert!(title.len() <= 70);
        assert!(title.ends_with("..."));
    }

    #[test]
    fn test_feedback_body_format() {
        let feedback = Feedback::new(
            FeedbackCategory::Bug,
            FeedbackPriority::High,
            "Test bug",
            "This is a test description.",
        );

        let body = feedback.issue_body();
        assert!(body.contains("**Category:** Bug Report"));
        assert!(body.contains("**Priority:** High"));
        assert!(body.contains("## Description"));
        assert!(body.contains("This is a test description."));
    }

    #[test]
    fn test_feedback_with_context() {
        let feedback = Feedback::new(
            FeedbackCategory::Feature,
            FeedbackPriority::Medium,
            "Test feature",
            "Description",
        )
        .with_context("What color?", "Blue")
        .with_context("What size?", "Large");

        let body = feedback.issue_body();
        assert!(body.contains("## Additional Context"));
        assert!(body.contains("**Q:** What color?"));
        assert!(body.contains("**A:** Blue"));
    }
}
