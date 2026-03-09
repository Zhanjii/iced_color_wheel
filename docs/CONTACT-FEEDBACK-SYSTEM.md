# Contact/Feedback System with Ralph WigGUIm Integration

This document describes the Contact/Feedback system that allows users to submit feature requests, bug reports, and feedback directly from the app. Submissions are automatically processed by Ralph WigGUIm for automated implementation.

## Overview

The system provides a complete feedback loop:

1. **User submits feedback** via iced popup window, CLI subcommand, or API
2. **Claude AI refines** the request with clarifying questions
3. **GitHub issue created** with `ralph-wiggum` label
4. **Ralph WigGUIm picks up** the issue and creates a task
5. **Automation runs** Claude in Docker to implement changes
6. **PR created** with changes for review
7. **User tests** changes via Launch button
8. **User merges** when satisfied

## Architecture

```
+-------------------------------------------------------------------+
|                         Your App                                    |
|  +--------------+    +--------------+    +--------------+          |
|  |   Contact    |--->|   Feedback   |--->|    Claude    |          |
|  |   Button     |    |   Dialog     |    |   Client     |          |
|  +--------------+    +--------------+    +--------------+          |
|                              |                    |                 |
|                              v                    v                 |
|                      +--------------+    +--------------+          |
|                      |  Feedback    |    |  Clarifying  |          |
|                      |  Submitter   |    |  Questions   |          |
|                      +--------------+    +--------------+          |
|                              |                                      |
+------------------------------+--------------------------------------+
                               |
                               v
                      +--------------+
                      |   GitHub     |
                      |   Issue      |
                      | (ralph-wiggum|
                      |   label)     |
                      +--------------+
                               |
                               v
+-------------------------------------------------------------------+
|                      Ralph WigGUIm                                  |
|  +--------------+    +--------------+    +--------------+          |
|  |   Trigger    |--->|  Automation  |--->|   Docker     |          |
|  |   Manager    |    |   Service    |    |  Container   |          |
|  +--------------+    +--------------+    +--------------+          |
|                              |                    |                 |
|                              v                    v                 |
|                      +--------------+    +--------------+          |
|                      |  Worktree    |    |   Claude     |          |
|                      |  Manager     |    |   (in Docker)|          |
|                      +--------------+    +--------------+          |
|                              |                                      |
|                              v                                      |
|                      +--------------+                              |
|                      |   Pull       |                              |
|                      |   Request    |                              |
|                      +--------------+                              |
|                              |                                      |
|                              v                                      |
|                      +--------------+    +--------------+          |
|                      |   Launch     |--->|   Merge      |          |
|                      |   Button     |    |   Button     |          |
|                      +--------------+    +--------------+          |
+-------------------------------------------------------------------+
```

## Components

### 1. Feedback Data Model (feedback.rs)

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FeedbackCategory {
    Bug,
    Feature,
    Feedback,
}

impl FeedbackCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Bug => "Bug Report",
            Self::Feature => "Feature Request",
            Self::Feedback => "General Feedback",
        }
    }

    pub fn github_label(&self) -> &'static str {
        match self {
            Self::Bug => "bug",
            Self::Feature => "enhancement",
            Self::Feedback => "feedback",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FeedbackPriority {
    Low,
    Medium,
    High,
    Critical,
}

impl FeedbackPriority {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
            Self::Critical => "Critical",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackSubmission {
    pub category: FeedbackCategory,
    pub priority: FeedbackPriority,
    pub title: String,
    pub description: String,
    pub action_plan: Option<String>,
    pub attachments: Vec<String>,  // Base64-encoded images
}
```

### 2. GitHub Issue Submitter (submitter.rs)

Submit feedback as a GitHub issue using the `gh` CLI or the GitHub API via `reqwest`:

```rust
use std::process::Command;
use tracing::{error, info};

use crate::feedback::{FeedbackCategory, FeedbackSubmission};

const GITHUB_REPO: &str = "YourUsername/YourRepo";
const RALPH_WIGGUM_LABEL: &str = "ralph-wiggum";

/// Submit feedback as a GitHub issue using the `gh` CLI.
pub fn submit_via_gh_cli(
    submission: &FeedbackSubmission,
) -> Result<String, Box<dyn std::error::Error>> {
    let body = format_issue_body(submission);

    let mut labels = vec![RALPH_WIGGUM_LABEL.to_string()];
    labels.push(submission.category.github_label().to_string());

    let output = Command::new("gh")
        .args([
            "issue",
            "create",
            "--repo",
            GITHUB_REPO,
            "--title",
            &submission.title,
            "--body",
            &body,
            "--label",
            &labels.join(","),
        ])
        .output()?;

    if output.status.success() {
        let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
        info!("Created GitHub issue: {url}");
        Ok(url)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!("Failed to create issue: {stderr}");
        Err(stderr.into_owned().into())
    }
}

/// Submit feedback via the GitHub API using reqwest.
pub async fn submit_via_api(
    client: &reqwest::Client,
    github_token: &str,
    submission: &FeedbackSubmission,
) -> Result<String, reqwest::Error> {
    let body = format_issue_body(submission);

    let mut labels = vec![RALPH_WIGGUM_LABEL.to_string()];
    labels.push(submission.category.github_label().to_string());

    #[derive(serde::Serialize)]
    struct CreateIssue {
        title: String,
        body: String,
        labels: Vec<String>,
    }

    let payload = CreateIssue {
        title: submission.title.clone(),
        body,
        labels,
    };

    let response = client
        .post(format!(
            "https://api.github.com/repos/{GITHUB_REPO}/issues"
        ))
        .header("Authorization", format!("Bearer {github_token}"))
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION")))
        .json(&payload)
        .send()
        .await?
        .error_for_status()?;

    #[derive(serde::Deserialize)]
    struct IssueResponse {
        html_url: String,
    }

    let issue: IssueResponse = response.json().await?;
    info!("Created GitHub issue: {}", issue.html_url);
    Ok(issue.html_url)
}

fn format_issue_body(submission: &FeedbackSubmission) -> String {
    let mut body = format!(
        "**Category:** {}\n\
         **Priority:** {}\n\
         **App Version:** {}\n\n\
         ## Description\n\n\
         {}\n",
        submission.category.as_str(),
        submission.priority.as_str(),
        env!("CARGO_PKG_VERSION"),
        submission.description,
    );

    if let Some(plan) = &submission.action_plan {
        body.push_str(&format!("\n## Action Plan\n\n{plan}\n"));
    }

    if !submission.attachments.is_empty() {
        body.push_str(&format!(
            "\n## Attachments\n\n{} image(s) attached.\n",
            submission.attachments.len()
        ));
    }

    body
}
```

### 3. Claude Client (claude_client.rs)

Handles AI integration for analyzing user requests, generating clarifying questions, and creating structured action plans:

```rust
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::warn;

const CLAUDE_API_URL: &str = "https://api.anthropic.com/v1/messages";

#[derive(Serialize)]
struct ClaudeRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<ClaudeMessage>,
}

#[derive(Serialize, Deserialize)]
struct ClaudeMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ClaudeResponse {
    content: Vec<ClaudeContent>,
}

#[derive(Deserialize)]
struct ClaudeContent {
    text: String,
}

pub struct ClaudeClient {
    client: Client,
    api_key: String,
    model: String,
    max_tokens: u32,
}

impl ClaudeClient {
    pub fn new(api_key: &str) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.to_string(),
            model: "claude-sonnet-4-20250514".to_string(),
            max_tokens: 1024,
        }
    }

    /// Generate clarifying questions for a user's feedback.
    pub async fn get_clarifying_questions(
        &self,
        category: &str,
        description: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let prompt = format!(
            "A user submitted a {category}. Their description:\n\n\
             \"{description}\"\n\n\
             Ask 2-3 short clarifying questions to understand the request better. \
             Format as a numbered list."
        );

        self.send_message(&prompt).await
    }

    /// Generate an action plan from the feedback and Q&A.
    pub async fn create_action_plan(
        &self,
        category: &str,
        description: &str,
        qa_context: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let prompt = format!(
            "Create a concise action plan for this {category}.\n\n\
             Description: {description}\n\n\
             Additional context from Q&A:\n{qa_context}\n\n\
             Include:\n\
             1. Summary of changes\n\
             2. Files likely affected\n\
             3. Acceptance criteria (testable)"
        );

        self.send_message(&prompt).await
    }

    async fn send_message(
        &self,
        content: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let request = ClaudeRequest {
            model: self.model.clone(),
            max_tokens: self.max_tokens,
            messages: vec![ClaudeMessage {
                role: "user".to_string(),
                content: content.to_string(),
            }],
        };

        let response = self
            .client
            .post(CLAUDE_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await?
            .error_for_status()?;

        let claude_response: ClaudeResponse = response.json().await?;

        claude_response
            .content
            .first()
            .map(|c| c.text.clone())
            .ok_or_else(|| "Empty response from Claude".into())
    }
}
```

### 4. Feedback Dialog (Iced Popup Window)

Open a feedback popup from your iced daemon app. The dialog state lives in your
`App` struct and renders via `view()` dispatch on `WindowKind::Feedback`.

```rust
use iced::widget::{button, column, container, pick_list, row, text, text_input};
use iced::{Element, Length, Size, Task};
use iced::window;

/// Feedback wizard step.
#[derive(Debug, Clone, PartialEq)]
enum FeedbackStep {
    CategorySelection,
    Description,
    Submitting,
    Done(String),
    Error(String),
}

/// Messages emitted by the feedback dialog.
#[derive(Debug, Clone)]
pub enum FeedbackDialogMsg {
    CategorySelected(FeedbackCategory),
    TitleChanged(String),
    DescriptionChanged(String),
    Back,
    Submit,
    SubmitResult(Result<String, String>),
    Close,
}

/// State for the feedback popup window.
pub struct FeedbackDialogState {
    step: FeedbackStep,
    category: Option<FeedbackCategory>,
    priority: FeedbackPriority,
    title: String,
    description: String,
}

impl FeedbackDialogState {
    pub fn new() -> Self {
        Self {
            step: FeedbackStep::CategorySelection,
            category: None,
            priority: FeedbackPriority::Medium,
            title: String::new(),
            description: String::new(),
        }
    }

    /// Window settings for the feedback popup.
    pub fn window_settings() -> window::Settings {
        window::Settings {
            size: Size::new(500.0, 400.0),
            min_size: Some(Size::new(400.0, 300.0)),
            ..window::Settings::default()
        }
    }

    /// Handle a feedback dialog message.
    pub fn update(&mut self, msg: FeedbackDialogMsg) -> Task<FeedbackDialogMsg> {
        match msg {
            FeedbackDialogMsg::CategorySelected(cat) => {
                self.category = Some(cat);
                self.step = FeedbackStep::Description;
                Task::none()
            }
            FeedbackDialogMsg::TitleChanged(t) => {
                self.title = t;
                Task::none()
            }
            FeedbackDialogMsg::DescriptionChanged(d) => {
                self.description = d;
                Task::none()
            }
            FeedbackDialogMsg::Back => {
                self.step = FeedbackStep::CategorySelection;
                Task::none()
            }
            FeedbackDialogMsg::Submit => {
                self.step = FeedbackStep::Submitting;
                let submission = FeedbackSubmission {
                    category: self.category.clone().unwrap_or(FeedbackCategory::Feedback),
                    priority: self.priority.clone(),
                    title: self.title.clone(),
                    description: self.description.clone(),
                    action_plan: None,
                    attachments: vec![],
                };
                Task::perform(
                    async move { submit_via_gh_cli(&submission) },
                    FeedbackDialogMsg::SubmitResult,
                )
            }
            FeedbackDialogMsg::SubmitResult(result) => {
                self.step = match result {
                    Ok(url) => FeedbackStep::Done(url),
                    Err(e) => FeedbackStep::Error(e),
                };
                Task::none()
            }
            FeedbackDialogMsg::Close => {
                // Parent handles closing the window
                Task::none()
            }
        }
    }

    /// Render the feedback dialog content.
    pub fn view(&self) -> Element<FeedbackDialogMsg> {
        let content: Element<FeedbackDialogMsg> = match &self.step {
            FeedbackStep::CategorySelection => {
                column![
                    text("What type of feedback?").size(18),
                    button("Bug Report")
                        .on_press(FeedbackDialogMsg::CategorySelected(FeedbackCategory::Bug))
                        .width(Length::Fill),
                    button("Feature Request")
                        .on_press(FeedbackDialogMsg::CategorySelected(FeedbackCategory::Feature))
                        .width(Length::Fill),
                    button("General Feedback")
                        .on_press(FeedbackDialogMsg::CategorySelected(FeedbackCategory::Feedback))
                        .width(Length::Fill),
                    button("Cancel").on_press(FeedbackDialogMsg::Close),
                ]
                .spacing(8)
                .into()
            }
            FeedbackStep::Description => {
                let can_submit = !self.title.is_empty() && !self.description.is_empty();
                column![
                    text("Describe your feedback").size(18),
                    text("Title:").size(13),
                    text_input("Brief summary...", &self.title)
                        .on_input(FeedbackDialogMsg::TitleChanged),
                    text("Description:").size(13),
                    text_input("Details...", &self.description)
                        .on_input(FeedbackDialogMsg::DescriptionChanged),
                    row![
                        button("Back").on_press(FeedbackDialogMsg::Back),
                        if can_submit {
                            button("Submit").on_press(FeedbackDialogMsg::Submit)
                        } else {
                            button("Submit")
                        },
                    ]
                    .spacing(8),
                ]
                .spacing(8)
                .into()
            }
            FeedbackStep::Submitting => {
                column![
                    text("Submitting feedback...").size(16),
                ]
                .into()
            }
            FeedbackStep::Done(url) => {
                column![
                    text("Feedback submitted successfully!").size(16),
                    text(url).size(12),
                    button("Close").on_press(FeedbackDialogMsg::Close),
                ]
                .spacing(8)
                .into()
            }
            FeedbackStep::Error(msg) => {
                column![
                    text(format!("Error: {msg}")).size(14),
                    row![
                        button("Try Again").on_press(FeedbackDialogMsg::Back),
                        button("Close").on_press(FeedbackDialogMsg::Close),
                    ]
                    .spacing(8),
                ]
                .spacing(8)
                .into()
            }
        };

        container(content).padding(20).into()
    }
}
```

### 6. CI Workflow (.github/workflows/ci.yml)

Required for Ralph WigGUIm to verify changes:

```yaml
name: CI

on:
  pull_request:
    branches: [master, main]
  push:
    branches: [master, main]

env:
  CARGO_TERM_COLOR: always

jobs:
  check:
    name: Check
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - name: Check formatting
        run: cargo fmt --check
      - name: Run clippy
        run: cargo clippy -- -D warnings
      - name: Run tests
        run: cargo test
```

## Setup Instructions

### Prerequisites

1. **GitHub CLI** (`gh`) installed and authenticated
2. **Ralph WigGUIm** configured with your repository
3. **Rust toolchain** installed
4. **Anthropic API key** (bundled or user-provided) for Claude Q&A

### Step 1: Add Required Files

Copy the template files to your project:

```
your_app/
├── src/
│   ├── feedback/
│   │   ├── mod.rs              # Re-exports
│   │   ├── model.rs            # FeedbackSubmission, Category, Priority
│   │   ├── submitter.rs        # GitHub issue creation
│   │   └── claude_client.rs    # Claude API integration
│   ├── ui/
│   │   └── feedback_dialog.rs  # iced popup window
│   └── main.rs
├── Cargo.toml
└── .github/
    └── workflows/
        └── ci.yml
```

### Step 2: Add Dependencies

In your `Cargo.toml`:

```toml
[dependencies]
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
tracing = "0.1"
```

### Step 3: Add Constants

In a config module:

```rust
// src/config.rs
pub const GITHUB_ISSUES_REPO: &str = "YourUsername/YourRepo";
pub const RALPH_WIGGUM_LABEL: &str = "ralph-wiggum";

pub const CLAUDE_MODEL: &str = "claude-sonnet-4-20250514";
pub const CLAUDE_MAX_TOKENS: u32 = 1024;
pub const FEEDBACK_DAILY_LIMIT: u32 = 10;

pub const FEEDBACK_MAX_DESCRIPTION_LENGTH: usize = 2000;
pub const FEEDBACK_MAX_ANSWER_LENGTH: usize = 500;

pub const APP_NAME: &str = "YourApp";
```

### Step 4: Create GitHub Label

Create the `ralph-wiggum` label in your repository:

```bash
gh label create ralph-wiggum --color "7057ff" --description "Automated by Ralph WigGUIm"
```

### Step 5: Configure Ralph WigGUIm

1. Add your repository to Ralph WigGUIm
2. Set the **Launch Command** in project settings:
   ```
   cargo run --release
   ```
3. Ralph will now poll for issues with the `ralph-wiggum` label

## Workflow Example

### User Submits Feedback

1. User clicks **Contact** button (or runs a CLI subcommand)
2. Selects "Feature Request" and "Medium" priority
3. Types: "Can we add a dark mode toggle to the settings page?"
4. Optionally attaches a screenshot
5. Claude asks: "Should this follow system theme or be independent?" "Where in settings?"
6. User answers questions
7. Claude generates action plan with acceptance criteria
8. User reviews and clicks **Submit**

### Ralph WigGUIm Processes

1. GitHub issue created with `ralph-wiggum` label
2. Ralph picks up issue on next poll
3. Creates git worktree for isolated changes
4. Starts Docker container with Claude
5. Claude reads the task and implements changes
6. Creates PR with changes
7. CI runs and passes
8. Task moves to **Review** column

### User Reviews and Merges

1. User clicks task card in Ralph
2. Clicks **Launch** to test from worktree
3. Verifies the dark mode toggle works
4. Clicks **Merge PR** to merge changes
5. Pulls latest changes to local repo
6. Done!

## Customization

### Categories

Add custom categories to the `FeedbackCategory` enum:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FeedbackCategory {
    Bug,
    Feature,
    Feedback,
    Performance,   // Custom category
    Documentation, // Custom category
}
```

### Claude Prompts

Customize the prompts in `claude_client.rs` to match your project context and coding style.

### Styling

Customize colors and layout in `feedback_dialog.rs`. Use iced's theme and style closures to match your app's appearance.

## Troubleshooting

### "gh command not found"
Install GitHub CLI: https://cli.github.com/

### "Not authenticated"
Run `gh auth login` to authenticate.

### Issues not appearing in Ralph
- Verify the `ralph-wiggum` label exists
- Check that Ralph has the repository configured
- Click "Check Now" in Ralph to force a poll

### Launch button doesn't work
- Set a custom **Launch Command** in project settings
- Verify the worktree path exists
- Check Ralph logs for errors

## Files Reference

### Template Files (docs/templates/)

| File | Purpose |
|------|---------|
| `feedback.rs` | Data model: categories, priorities, submission struct |
| `submitter.rs` | GitHub issue creation with `ralph-wiggum` label |
| `claude_client.rs` | Claude API integration for Q&A and action plans |
| `feedback_dialog.rs` | Multi-step feedback wizard (iced popup window) |
| `ci.yml` | CI workflow template for GitHub Actions |

## Quick Start Checklist

- [ ] Add feedback module with model, submitter, and Claude client
- [ ] Update `Cargo.toml` with `reqwest`, `serde`, `tokio` dependencies
- [ ] Add constants to your config module
- [ ] Add feedback dialog (iced popup window)
- [ ] Create `ralph-wiggum` label in your GitHub repo
- [ ] Add `.github/workflows/ci.yml` for CI checks
- [ ] Configure project in Ralph WigGUIm with Launch Command
- [ ] Test the full flow: Submit feedback -> Auto-process -> Review -> Merge
