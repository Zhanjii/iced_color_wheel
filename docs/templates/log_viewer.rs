//! log_viewer.rs
//!
//! Log file reading and popup viewer for iced daemon applications.
//!
//! Features:
//! - List log files with metadata
//! - Read and parse log file contents
//! - Filter logs by level and search query
//! - LogViewerState with view() for an iced popup window
//! - Delete old log files
//!
//! # Integration
//!
//! Register `WindowKind::LogViewer` in your daemon's window registry.
//! Open it with `window::open(log_viewer_settings())`. The state lives
//! in your App struct and renders via `LogViewerState::view()`.
//!
//! ```rust
//! // In App struct:
//! log_viewer: log_viewer::LogViewerState,
//!
//! // In App::view() dispatch:
//! Some(WindowKind::LogViewer) => self.log_viewer.view().map(Message::LogViewer),
//!
//! // In App::update():
//! Message::LogViewer(msg) => self.log_viewer.update(msg),
//! ```

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

// =============================================================================
// CONFIGURATION
// =============================================================================

/// Get log directory path.
pub fn get_log_dir() -> PathBuf {
    let app_dir = if cfg!(target_os = "windows") {
        dirs::data_local_dir()
    } else if cfg!(target_os = "macos") {
        dirs::data_dir()
    } else {
        dirs::data_dir()
    }
    .expect("Could not find data directory");

    app_dir.join(env!("CARGO_PKG_NAME")).join("logs")
}

/// Window settings for the log viewer popup.
pub fn log_viewer_settings() -> iced::window::Settings {
    iced::window::Settings {
        size: iced::Size::new(750.0, 500.0),
        min_size: Some(iced::Size::new(400.0, 300.0)),
        ..iced::window::Settings::default()
    }
}

// =============================================================================
// DATA STRUCTURES
// =============================================================================

/// Log file metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogFileInfo {
    /// File name
    pub filename: String,
    /// Full file path
    pub file_path: String,
    /// File size in bytes
    pub size: u64,
    /// Human-readable file size
    pub size_display: String,
    /// File creation date
    pub date: String,
    /// Day of week
    pub day: String,
    /// Last modified timestamp
    pub modified: i64,
}

/// Log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Log level (DEBUG, INFO, WARN, ERROR)
    pub level: String,
    /// Timestamp
    pub timestamp: String,
    /// Log message
    pub message: String,
    /// Module/source
    pub module: Option<String>,
}

/// Log filter options.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogFilter {
    /// Minimum log level to show
    pub min_level: Option<String>,
    /// Search query
    pub search: Option<String>,
    /// Module filter
    pub module: Option<String>,
}

// =============================================================================
// LOG FILE OPERATIONS (framework-agnostic)
// =============================================================================

/// Get list of log files with metadata.
pub fn get_log_files() -> Result<Vec<LogFileInfo>, String> {
    let log_dir = get_log_dir();

    if !log_dir.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();

    for entry in fs::read_dir(&log_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("log") {
            if let Ok(metadata) = fs::metadata(&path) {
                let filename = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                let size = metadata.len();
                let size_display = format_size(size);

                let modified = metadata
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);

                // Extract date from filename (assumes YYYY-MM-DD format)
                let date = filename
                    .trim_end_matches(".log")
                    .split('_')
                    .last()
                    .unwrap_or("Unknown")
                    .to_string();

                let day = if let Ok(date_time) = chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d") {
                    date_time.format("%A").to_string()
                } else {
                    "Unknown".to_string()
                };

                files.push(LogFileInfo {
                    filename,
                    file_path: path.to_string_lossy().to_string(),
                    size,
                    size_display,
                    date,
                    day,
                    modified,
                });
            }
        }
    }

    // Sort by modified time (newest first)
    files.sort_by(|a, b| b.modified.cmp(&a.modified));

    Ok(files)
}

/// Read log file contents as a string.
pub fn read_log_file(file_path: &str) -> Result<String, String> {
    fs::read_to_string(file_path).map_err(|e| format!("Failed to read log file: {}", e))
}

/// Read log file with filtering.
pub fn read_log_file_filtered(
    file_path: &str,
    filter: &LogFilter,
) -> Result<Vec<LogEntry>, String> {
    let content = fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read log file: {}", e))?;

    let mut entries = Vec::new();

    for line in content.lines() {
        if let Some(entry) = parse_log_line(line) {
            if let Some(ref min_level) = filter.min_level {
                if !matches_level(&entry.level, min_level) {
                    continue;
                }
            }

            if let Some(ref search) = filter.search {
                if !entry.message.to_lowercase().contains(&search.to_lowercase()) {
                    continue;
                }
            }

            if let Some(ref module_filter) = filter.module {
                if let Some(ref module) = entry.module {
                    if !module.contains(module_filter) {
                        continue;
                    }
                } else {
                    continue;
                }
            }

            entries.push(entry);
        }
    }

    Ok(entries)
}

/// Delete a log file.
pub fn delete_log_file(file_path: &str) -> Result<(), String> {
    fs::remove_file(file_path).map_err(|e| format!("Failed to delete log file: {}", e))
}

/// Delete old log files (older than specified days).
pub fn delete_old_logs(days: u64) -> Result<usize, String> {
    let log_dir = get_log_dir();
    if !log_dir.exists() {
        return Ok(0);
    }

    let cutoff = std::time::SystemTime::now() - std::time::Duration::from_secs(days * 24 * 60 * 60);
    let mut deleted_count = 0;

    for entry in fs::read_dir(&log_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("log") {
            if let Ok(metadata) = fs::metadata(&path) {
                if let Ok(modified) = metadata.modified() {
                    if modified < cutoff {
                        if fs::remove_file(&path).is_ok() {
                            deleted_count += 1;
                        }
                    }
                }
            }
        }
    }

    Ok(deleted_count)
}

/// Open log directory in file explorer.
pub fn open_log_directory() -> Result<(), String> {
    let log_dir = get_log_dir();

    if !log_dir.exists() {
        fs::create_dir_all(&log_dir)
            .map_err(|e| format!("Failed to create log directory: {}", e))?;
    }

    open::that(&log_dir).map_err(|e| format!("Failed to open directory: {}", e))
}

// =============================================================================
// ICED LOG VIEWER STATE
// =============================================================================

use iced::widget::{button, column, container, pick_list, row, scrollable, text, text_input};
use iced::{Element, Length, Task};

/// Messages for the log viewer popup.
#[derive(Debug, Clone)]
pub enum LogViewerMessage {
    /// Refresh the file list
    RefreshFiles,
    /// A log file was selected
    FileSelected(String),
    /// Log content was loaded
    ContentLoaded(Result<String, String>),
    /// Filter level changed
    LevelChanged(String),
    /// Search text changed
    SearchChanged(String),
    /// Delete old logs button pressed
    DeleteOldLogs,
    /// Open log directory in file explorer
    OpenDirectory,
}

/// State for the log viewer popup window.
pub struct LogViewerState {
    /// Available log files
    files: Vec<LogFileInfo>,
    /// Currently selected file path
    selected_file: Option<String>,
    /// Raw content of the selected log file
    content: String,
    /// Parsed and filtered log entries
    entries: Vec<LogEntry>,
    /// Current filter level
    filter_level: String,
    /// Current search query
    search_query: String,
    /// Error message, if any
    error: Option<String>,
}

impl LogViewerState {
    /// Create a new log viewer state and load the file list.
    pub fn new() -> Self {
        let files = get_log_files().unwrap_or_default();
        Self {
            files,
            selected_file: None,
            content: String::new(),
            entries: Vec::new(),
            filter_level: "DEBUG".to_string(),
            search_query: String::new(),
            error: None,
        }
    }

    /// Handle a log viewer message. Returns an iced Task.
    pub fn update(&mut self, message: LogViewerMessage) -> Task<LogViewerMessage> {
        match message {
            LogViewerMessage::RefreshFiles => {
                self.files = get_log_files().unwrap_or_default();
                Task::none()
            }
            LogViewerMessage::FileSelected(path) => {
                self.selected_file = Some(path.clone());
                self.error = None;
                // Load file content
                let filter = LogFilter {
                    min_level: Some(self.filter_level.clone()),
                    search: if self.search_query.is_empty() {
                        None
                    } else {
                        Some(self.search_query.clone())
                    },
                    module: None,
                };
                match read_log_file_filtered(&path, &filter) {
                    Ok(entries) => {
                        self.entries = entries;
                        self.content = read_log_file(&path).unwrap_or_default();
                    }
                    Err(e) => {
                        self.error = Some(e);
                    }
                }
                Task::none()
            }
            LogViewerMessage::ContentLoaded(result) => {
                match result {
                    Ok(content) => self.content = content,
                    Err(e) => self.error = Some(e),
                }
                Task::none()
            }
            LogViewerMessage::LevelChanged(level) => {
                self.filter_level = level;
                // Re-filter if a file is selected
                if let Some(ref path) = self.selected_file {
                    let filter = LogFilter {
                        min_level: Some(self.filter_level.clone()),
                        search: if self.search_query.is_empty() {
                            None
                        } else {
                            Some(self.search_query.clone())
                        },
                        module: None,
                    };
                    self.entries = read_log_file_filtered(path, &filter).unwrap_or_default();
                }
                Task::none()
            }
            LogViewerMessage::SearchChanged(query) => {
                self.search_query = query;
                // Re-filter if a file is selected
                if let Some(ref path) = self.selected_file {
                    let filter = LogFilter {
                        min_level: Some(self.filter_level.clone()),
                        search: if self.search_query.is_empty() {
                            None
                        } else {
                            Some(self.search_query.clone())
                        },
                        module: None,
                    };
                    self.entries = read_log_file_filtered(path, &filter).unwrap_or_default();
                }
                Task::none()
            }
            LogViewerMessage::DeleteOldLogs => {
                match delete_old_logs(30) {
                    Ok(count) => log::info!("Deleted {} old log files", count),
                    Err(e) => log::error!("Failed to delete old logs: {}", e),
                }
                self.files = get_log_files().unwrap_or_default();
                Task::none()
            }
            LogViewerMessage::OpenDirectory => {
                if let Err(e) = open_log_directory() {
                    log::error!("Failed to open log directory: {}", e);
                }
                Task::none()
            }
        }
    }

    /// Render the log viewer popup content.
    pub fn view(&self) -> Element<LogViewerMessage> {
        let levels = vec![
            "DEBUG".to_string(),
            "INFO".to_string(),
            "WARN".to_string(),
            "ERROR".to_string(),
        ];

        // Toolbar
        let toolbar = row![
            pick_list(
                levels,
                Some(self.filter_level.clone()),
                LogViewerMessage::LevelChanged,
            ),
            text_input("Search logs...", &self.search_query)
                .on_input(LogViewerMessage::SearchChanged)
                .width(Length::Fill),
            button("Refresh").on_press(LogViewerMessage::RefreshFiles),
            button("Open Dir").on_press(LogViewerMessage::OpenDirectory),
            button("Clean Old").on_press(LogViewerMessage::DeleteOldLogs),
        ]
        .spacing(8)
        .padding(8);

        // File list (left sidebar)
        let file_list: Element<LogViewerMessage> = if self.files.is_empty() {
            text("No log files found.").into()
        } else {
            let items: Vec<Element<LogViewerMessage>> = self
                .files
                .iter()
                .map(|f| {
                    let label = format!("{} ({})", f.filename, f.size_display);
                    button(text(label).size(12))
                        .on_press(LogViewerMessage::FileSelected(f.file_path.clone()))
                        .width(Length::Fill)
                        .into()
                })
                .collect();
            scrollable(column(items).spacing(2))
                .height(Length::Fill)
                .into()
        };

        // Log content (right panel)
        let log_content: Element<LogViewerMessage> = if let Some(ref err) = self.error {
            text(format!("Error: {}", err)).into()
        } else if self.entries.is_empty() {
            text("Select a log file to view its contents.").into()
        } else {
            let lines: Vec<Element<LogViewerMessage>> = self
                .entries
                .iter()
                .map(|entry| {
                    let line = format!(
                        "[{}] {} {}",
                        entry.level,
                        entry.timestamp,
                        entry.message
                    );
                    text(line).size(11).into()
                })
                .collect();
            scrollable(column(lines).spacing(1))
                .height(Length::Fill)
                .into()
        };

        let body = row![
            container(file_list).width(220),
            container(log_content).width(Length::Fill),
        ]
        .spacing(8)
        .height(Length::Fill);

        container(
            column![toolbar, body]
                .spacing(4)
                .padding(8),
        )
        .into()
    }
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Parse a log line into a LogEntry.
fn parse_log_line(line: &str) -> Option<LogEntry> {
    // Example log format: "[2025-02-02 10:30:45] INFO [module::path] Message"
    let parts: Vec<&str> = line.splitn(4, &['[', ']'][..]).collect();

    if parts.len() >= 4 {
        let timestamp = parts[1].trim().to_string();
        let level = parts[2].trim().to_string();
        let rest = parts[3].trim();

        let (module, message) = if rest.starts_with('[') {
            let parts: Vec<&str> = rest.splitn(2, ']').collect();
            if parts.len() == 2 {
                (
                    Some(parts[0].trim_start_matches('[').to_string()),
                    parts[1].trim().to_string(),
                )
            } else {
                (None, rest.to_string())
            }
        } else {
            (None, rest.to_string())
        };

        Some(LogEntry {
            level,
            timestamp,
            message,
            module,
        })
    } else {
        Some(LogEntry {
            level: "INFO".to_string(),
            timestamp: String::new(),
            message: line.to_string(),
            module: None,
        })
    }
}

/// Check if log level matches minimum level.
fn matches_level(level: &str, min_level: &str) -> bool {
    let levels = ["DEBUG", "INFO", "WARN", "ERROR"];

    let level_idx = levels.iter().position(|&l| l == level).unwrap_or(0);
    let min_idx = levels.iter().position(|&l| l == min_level).unwrap_or(0);

    level_idx >= min_idx
}

/// Format file size in human-readable format.
fn format_size(size: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if size >= GB {
        format!("{:.2} GB", size as f64 / GB as f64)
    } else if size >= MB {
        format!("{:.2} MB", size as f64 / MB as f64)
    } else if size >= KB {
        format!("{:.2} KB", size as f64 / KB as f64)
    } else {
        format!("{} bytes", size)
    }
}

// =============================================================================
// DEPENDENCIES
// =============================================================================

// Add to Cargo.toml:
// [dependencies]
// iced = { version = "0.14", features = ["multi-window", "tokio"] }
// serde = { version = "1.0", features = ["derive"] }
// chrono = "0.4"
// dirs = "5.0"
// open = "5.0"
// log = "0.4"

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_log_line() {
        let line = "[2025-02-02 10:30:45] INFO [app::core] Application started";
        let entry = parse_log_line(line).unwrap();

        assert_eq!(entry.level, "INFO");
        assert_eq!(entry.timestamp, "2025-02-02 10:30:45");
        assert_eq!(entry.module, Some("app::core".to_string()));
        assert_eq!(entry.message, "Application started");
    }

    #[test]
    fn test_matches_level() {
        assert!(matches_level("ERROR", "DEBUG"));
        assert!(matches_level("INFO", "INFO"));
        assert!(!matches_level("DEBUG", "ERROR"));
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 bytes");
        assert_eq!(format_size(1024), "1.00 KB");
        assert_eq!(format_size(1024 * 1024), "1.00 MB");
    }

    #[test]
    fn test_log_filter_default() {
        let filter = LogFilter {
            min_level: Some("INFO".to_string()),
            search: None,
            module: None,
        };
        assert_eq!(filter.min_level, Some("INFO".to_string()));
        assert!(filter.search.is_none());
    }
}
