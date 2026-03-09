//! generate_changelog.rs
//!
//! Generate CHANGELOG.md from git commit history.
//!
//! This binary parses git commits using conventional commit format and generates
//! a structured changelog following the Keep a Changelog specification.
//!
//! Commit Message Convention:
//!     feat:     New feature        → "Added" section
//!     fix:      Bug fix            → "Fixed" section
//!     perf:     Performance        → "Improved" section
//!     refactor: Code refactoring   → "Changed" section
//!     security: Security fix       → "Security" section
//!
//! Excluded from changelog (internal/maintenance):
//!     docs:, test:, style:, ci:, build:, chore:
//!     Merge commits, WIP commits
//!
//! # Usage
//!
//! ```bash
//! cargo run --bin generate_changelog              # Generate CHANGELOG.md
//! cargo run --bin generate_changelog -- --dry-run # Preview without writing
//! cargo run --bin generate_changelog -- --output custom.md  # Custom output
//! ```

use chrono::Local;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

// =============================================================================
// CONFIGURATION
// =============================================================================

/// Repository root
const REPO_ROOT: &str = ".";

/// Output path for changelog
const CHANGELOG_OUTPUT: &str = "CHANGELOG.md";

/// Application name for changelog header
const APP_NAME: &str = "Your App Name";

/// Maximum versions to include (0 = unlimited)
const MAX_VERSIONS: usize = 0;

/// Prefixes to exclude from changelog
const EXCLUDED_PREFIXES: &[&str] = &[
    r"^Merge ",
    r"^WIP",
    r"^\[skip ci\]",
    r"^chore\(deps\)",
    r"^chore\(release\)",
    r"^docs:",
    r"^style:",
    r"^test:",
    r"^ci:",
    r"^build:",
];

/// Patterns that exclude commits
const EXCLUDED_PATTERNS: &[&str] = &[
    r"Generated with Claude Code",
    r"Co-Authored-By:",
    r"^\d+\.\d+\.\d+$",
    r"^Update CHANGELOG",
    r"^Bump version",
];

// =============================================================================
// DATA STRUCTURES
// =============================================================================

/// Represents a git commit
#[derive(Debug, Clone)]
struct Commit {
    hash: String,
    date: String,
    message: String,
    category: String,
    clean_message: String,
}

/// Represents a version with its commits
#[derive(Debug, Clone)]
struct Version {
    version: String,
    date: String,
    commits: Vec<Commit>,
    is_current: bool,
}

// =============================================================================
// GIT OPERATIONS
// =============================================================================

/// Run a git command and return output
fn run_git_command(args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(REPO_ROOT)
        .output()
        .map_err(|e| format!("Git command failed: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

/// Get current version from Cargo.toml
fn get_current_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Get all version tags with their dates
fn get_git_tags() -> Vec<(String, String)> {
    let output = run_git_command(&[
        "tag",
        "-l",
        "v*",
        "--sort=-version:refname",
        "--format=%(refname:short)|%(creatordate:short)",
    ])
    .unwrap_or_default();

    output
        .lines()
        .filter_map(|line| {
            if let Some((tag, date)) = line.split_once('|') {
                Some((tag.to_string(), date.to_string()))
            } else {
                Some((line.to_string(), String::new()))
            }
        })
        .collect()
}

/// Get commits between two git refs
fn get_commits_between(from_ref: Option<&str>, to_ref: &str) -> Vec<Commit> {
    let ref_range = if let Some(from) = from_ref {
        format!("{}..{}", from, to_ref)
    } else {
        to_ref.to_string()
    };

    let output = run_git_command(&[
        "log",
        &ref_range,
        "--pretty=format:%h|%as|%s",
        "--no-merges",
    ])
    .unwrap_or_default();

    output
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(3, '|').collect();
            if parts.len() == 3 {
                Some(Commit {
                    hash: parts[0].to_string(),
                    date: parts[1].to_string(),
                    message: parts[2].to_string(),
                    category: String::new(),
                    clean_message: String::new(),
                })
            } else {
                None
            }
        })
        .collect()
}

// =============================================================================
// COMMIT PROCESSING
// =============================================================================

/// Check if a commit should be excluded
fn should_exclude_commit(message: &str) -> bool {
    for pattern in EXCLUDED_PREFIXES {
        if Regex::new(pattern).unwrap().is_match(message) {
            return true;
        }
    }

    for pattern in EXCLUDED_PATTERNS {
        if Regex::new(pattern).unwrap().is_match(message) {
            return true;
        }
    }

    false
}

/// Categorize a commit and clean up its message
fn categorize_commit(mut commit: Commit) -> Commit {
    let message = commit.message.trim();

    let patterns = [
        (r"^feat[\(:]", "Added", r"^feat(\([^)]*\))?:\s*"),
        (r"^fix[\(:]", "Fixed", r"^fix(\([^)]*\))?:\s*"),
        (r"^perf[\(:]", "Improved", r"^perf(\([^)]*\))?:\s*"),
        (r"^refactor[\(:]", "Changed", r"^refactor(\([^)]*\))?:\s*"),
        (r"^security[\(:]", "Security", r"^security(\([^)]*\))?:\s*"),
        (r"^chore[\(:]", "Changed", r"^chore(\([^)]*\))?:\s*"),
    ];

    for (match_pattern, category, remove_pattern) in &patterns {
        if Regex::new(match_pattern).unwrap().is_match(message) {
            let clean = Regex::new(remove_pattern)
                .unwrap()
                .replace(message, "")
                .to_string();

            let clean = if !clean.is_empty() {
                let mut chars = clean.chars();
                match chars.next() {
                    Some(first) => first.to_uppercase().chain(chars).collect(),
                    None => clean,
                }
            } else {
                clean
            };

            commit.category = category.to_string();
            commit.clean_message = clean;
            return commit;
        }
    }

    // Check for common action words
    let lower = message.to_lowercase();

    if lower.starts_with("add ") || lower.starts_with("added ") {
        commit.category = "Added".to_string();
    } else if lower.starts_with("fix ") || lower.starts_with("fixed ") {
        commit.category = "Fixed".to_string();
    } else if lower.starts_with("update ") || lower.starts_with("updated ") {
        commit.category = "Changed".to_string();
    } else if lower.starts_with("improve ") || lower.starts_with("improved ") {
        commit.category = "Improved".to_string();
    } else if lower.starts_with("remove ") || lower.starts_with("removed ") {
        commit.category = "Removed".to_string();
    } else {
        commit.category = "Changed".to_string();
    }

    // Capitalize first letter
    commit.clean_message = if !message.is_empty() {
        let mut chars = message.chars();
        match chars.next() {
            Some(first) => first.to_uppercase().chain(chars).collect(),
            None => message.to_string(),
        }
    } else {
        message.to_string()
    };

    commit
}

/// Filter and categorize commits
fn filter_and_categorize_commits(commits: Vec<Commit>) -> Vec<Commit> {
    commits
        .into_iter()
        .filter(|c| !should_exclude_commit(&c.message))
        .map(categorize_commit)
        .collect()
}

// =============================================================================
// CHANGELOG GENERATION
// =============================================================================

/// Generate the CHANGELOG.md content
fn generate_changelog_content(versions: &[Version]) -> String {
    let mut lines = vec![
        "# Changelog".to_string(),
        "".to_string(),
        format!("All notable changes to {} will be documented in this file.", APP_NAME),
        "".to_string(),
        "The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),".to_string(),
        "and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).".to_string(),
        "".to_string(),
    ];

    let category_order = ["Added", "Changed", "Improved", "Fixed", "Removed", "Security"];

    for version in versions {
        let version_str = if version.version.starts_with('[') {
            version.version.clone()
        } else {
            format!("[{}]", version.version)
        };

        let date_str = if version.date.is_empty() {
            "Unreleased".to_string()
        } else {
            version.date.clone()
        };

        lines.push(format!("## {} - {}", version_str, date_str));
        lines.push("".to_string());

        if version.commits.is_empty() {
            lines.push("- No notable changes".to_string());
            lines.push("".to_string());
            continue;
        }

        // Group commits by category
        let mut categories: HashMap<String, Vec<String>> = HashMap::new();

        for commit in &version.commits {
            categories
                .entry(commit.category.clone())
                .or_insert_with(Vec::new)
                .push(commit.clean_message.clone());
        }

        // Output categories in order
        for category in &category_order {
            if let Some(messages) = categories.get(*category) {
                lines.push(format!("### {}", category));
                for msg in messages {
                    let msg = msg.trim_end_matches('.');
                    lines.push(format!("- {}", msg));
                }
                lines.push("".to_string());
            }
        }

        // Any remaining categories
        for (category, messages) in categories.iter() {
            if !category_order.contains(&category.as_str()) {
                lines.push(format!("### {}", category));
                for msg in messages {
                    let msg = msg.trim_end_matches('.');
                    lines.push(format!("- {}", msg));
                }
                lines.push("".to_string());
            }
        }
    }

    lines.join("\n")
}

/// Generate changelog from git history
fn generate_changelog() -> Vec<Version> {
    let current_version = get_current_version();
    let tags = get_git_tags();
    let mut versions = Vec::new();

    println!("Current version: {}", current_version);
    println!("Found {} version tags", tags.len());

    // Get commits from HEAD to latest tag (unreleased changes)
    if !tags.is_empty() {
        let (latest_tag, _) = &tags[0];
        let head_commits = get_commits_between(Some(latest_tag), "HEAD");
        let head_commits = filter_and_categorize_commits(head_commits);

        if !head_commits.is_empty() {
            println!("  Unreleased: {} commits since {}", head_commits.len(), latest_tag);

            versions.push(Version {
                version: current_version.clone(),
                date: Local::now().format("%Y-%m-%d").to_string(),
                commits: head_commits,
                is_current: true,
            });
        }
    }

    // Process each tag
    for (i, (tag, date)) in tags.iter().enumerate() {
        if MAX_VERSIONS > 0 && versions.len() >= MAX_VERSIONS {
            break;
        }

        let version_str = tag.trim_start_matches('v');
        let prev_tag = tags.get(i + 1).map(|(t, _)| t.as_str());

        let commits = get_commits_between(prev_tag, tag);
        let commits = filter_and_categorize_commits(commits);

        println!("  {}: {} commits", tag, commits.len());

        versions.push(Version {
            version: version_str.to_string(),
            date: date.clone(),
            commits,
            is_current: false,
        });
    }

    // If no tags exist, get all commits
    if tags.is_empty() {
        println!("No version tags found, getting all commits...");
        let all_commits = get_commits_between(None, "HEAD");
        let all_commits = filter_and_categorize_commits(all_commits);

        versions.push(Version {
            version: current_version,
            date: Local::now().format("%Y-%m-%d").to_string(),
            commits: all_commits,
            is_current: true,
        });
    }

    versions
}

// =============================================================================
// MAIN
// =============================================================================

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dry_run = args.contains(&"--dry-run".to_string());
    let output = args
        .iter()
        .position(|a| a == "--output")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(CHANGELOG_OUTPUT));

    println!("{}", "=".repeat(60));
    println!("Generating CHANGELOG.md from git history");
    println!("{}", "=".repeat(60));
    println!();

    // Check we're in a git repo
    if run_git_command(&["rev-parse", "--git-dir"]).is_err() {
        eprintln!("Error: Not in a git repository");
        std::process::exit(1);
    }

    // Generate changelog
    let versions = generate_changelog();

    if versions.is_empty() {
        eprintln!("No versions found!");
        std::process::exit(1);
    }

    // Generate content
    let content = generate_changelog_content(&versions);

    println!();
    println!("Generated changelog with {} version(s)", versions.len());

    if dry_run {
        println!();
        println!("{}", "=".repeat(60));
        println!("DRY RUN - Preview of CHANGELOG.md:");
        println!("{}", "=".repeat(60));
        let preview = if content.len() > 3000 {
            format!("{}\n\n... ({} more characters)", &content[..3000], content.len() - 3000)
        } else {
            content
        };
        println!("{}", preview);
    } else {
        fs::write(&output, content).expect("Failed to write CHANGELOG.md");
        println!("Written to: {:?}", output);
    }
}
