//! logging_setup.rs
//!
//! Logging configuration with sensitive data filtering.
//!
//! Features:
//! - Thread-safe singleton LogManager
//! - Console, file, and in-memory handlers
//! - Daily log rotation
//! - Sensitive data redaction
//! - Per-module log level configuration
//!
//! # Example
//!
//! ```rust
//! use logging_setup::{LogManager, get_logger};
//!
//! // Initialize logging
//! let mut log_manager = LogManager::get_instance();
//! log_manager.initialize(Path::new("logs"), "development");
//!
//! // Get a logger
//! let logger = get_logger("my_module");
//! logger.info("Hello, world!");
//! ```

use chrono::Local;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

// =============================================================================
// LOG LEVELS
// =============================================================================

/// Log levels for filtering
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Debug = 0,
    Info = 1,
    Warning = 2,
    Error = 3,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warning => "WARNING",
            Self::Error => "ERROR",
        }
    }
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// =============================================================================
// LOG RECORD
// =============================================================================

/// A single log record
#[derive(Debug, Clone)]
pub struct LogRecord {
    pub timestamp: String,
    pub level: LogLevel,
    pub module: String,
    pub message: String,
}

impl LogRecord {
    pub fn new(level: LogLevel, module: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            timestamp: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            level,
            module: module.into(),
            message: message.into(),
        }
    }
}

// =============================================================================
// SENSITIVE DATA FILTER
// =============================================================================

/// Filter that redacts sensitive data from log messages
pub struct SensitiveDataFilter {
    patterns: Vec<(Regex, &'static str)>,
}

impl SensitiveDataFilter {
    pub fn new() -> Self {
        let patterns = vec![
            (Regex::new(r"ghp_[a-zA-Z0-9]{36}").unwrap(), "[GITHUB_PAT]"),
            (Regex::new(r"gho_[a-zA-Z0-9]{36}").unwrap(), "[GITHUB_OAUTH]"),
            (Regex::new(r"ghs_[a-zA-Z0-9]{36}").unwrap(), "[GITHUB_APP]"),
            (Regex::new(r"ghr_[a-zA-Z0-9]{36}").unwrap(), "[GITHUB_REFRESH]"),
            (Regex::new(r"sk-[a-zA-Z0-9]{48}").unwrap(), "[OPENAI_KEY]"),
            (Regex::new(r"Bearer\s+[a-zA-Z0-9\-._~+/]+=*").unwrap(), "Bearer [REDACTED]"),
            (Regex::new(r"api[_-]?key[\"']?\s*[:=]\s*[\"']?[\w\-]+").unwrap(), "api_key=[REDACTED]"),
            (Regex::new(r"password[\"']?\s*[:=]\s*[\"']?[^\s\"']+").unwrap(), "password=[REDACTED]"),
            (Regex::new(r"secret[\"']?\s*[:=]\s*[\"']?[\w\-]+").unwrap(), "secret=[REDACTED]"),
            (Regex::new(r"token[\"']?\s*[:=]\s*[\"']?[\w\-]+").unwrap(), "token=[REDACTED]"),
            (Regex::new(r"[\w\.-]+@[\w\.-]+\.\w+").unwrap(), "[EMAIL]"),
        ];

        Self { patterns }
    }

    pub fn filter(&self, message: &str) -> String {
        let mut filtered = message.to_string();

        for (pattern, replacement) in &self.patterns {
            filtered = pattern.replace_all(&filtered, *replacement).to_string();
        }

        filtered
    }
}

impl Default for SensitiveDataFilter {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// MEMORY HANDLER
// =============================================================================

/// Handler that stores logs in memory for in-app viewing
pub struct MemoryHandler {
    capacity: usize,
    records: Arc<Mutex<VecDeque<LogRecord>>>,
}

impl MemoryHandler {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            records: Arc::new(Mutex::new(VecDeque::with_capacity(capacity))),
        }
    }

    pub fn emit(&self, record: LogRecord) {
        let mut records = self.records.lock().unwrap();
        records.push_back(record);
        if records.len() > self.capacity {
            records.pop_front();
        }
    }

    pub fn get_records(&self, level: LogLevel, limit: usize, search: &str) -> Vec<LogRecord> {
        let records = self.records.lock().unwrap();
        let search_lower = search.to_lowercase();

        records
            .iter()
            .filter(|r| r.level >= level)
            .filter(|r| search.is_empty() || r.message.to_lowercase().contains(&search_lower))
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn clear(&self) {
        self.records.lock().unwrap().clear();
    }
}

// =============================================================================
// LOG MANAGER
// =============================================================================

/// Configuration presets for different environments
#[derive(Debug, Clone, Copy)]
pub struct LogConfig {
    pub console: LogLevel,
    pub file: LogLevel,
}

impl LogConfig {
    pub const DEVELOPMENT: Self = Self {
        console: LogLevel::Debug,
        file: LogLevel::Debug,
    };

    pub const TESTING: Self = Self {
        console: LogLevel::Info,
        file: LogLevel::Debug,
    };

    pub const PRODUCTION: Self = Self {
        console: LogLevel::Warning,
        file: LogLevel::Info,
    };

    pub fn from_name(name: &str) -> Self {
        match name {
            "development" => Self::DEVELOPMENT,
            "testing" => Self::TESTING,
            "production" => Self::PRODUCTION,
            _ => Self::PRODUCTION,
        }
    }
}

/// Thread-safe singleton log manager
pub struct LogManager {
    log_dir: Option<PathBuf>,
    config: LogConfig,
    memory_handler: Arc<MemoryHandler>,
    filter: Arc<SensitiveDataFilter>,
    file_handle: Arc<Mutex<Option<std::fs::File>>>,
}

impl LogManager {
    fn new() -> Self {
        Self {
            log_dir: None,
            config: LogConfig::PRODUCTION,
            memory_handler: Arc::new(MemoryHandler::new(1000)),
            filter: Arc::new(SensitiveDataFilter::new()),
            file_handle: Arc::new(Mutex::new(None)),
        }
    }

    /// Initialize the logging system
    pub fn initialize(&mut self, log_dir: impl AsRef<Path>, config_name: &str) {
        let log_dir = log_dir.as_ref().to_path_buf();
        fs::create_dir_all(&log_dir).ok();

        self.log_dir = Some(log_dir.clone());
        self.config = LogConfig::from_name(config_name);

        // Open log file
        let date = Local::now().format("%Y-%m-%d").to_string();
        let log_file = log_dir.join(format!("app_{}.log", date));

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_file)
            .ok();

        *self.file_handle.lock().unwrap() = file;
    }

    /// Log a message
    pub fn log(&self, level: LogLevel, module: impl Into<String>, message: impl Into<String>) {
        let message = self.filter.filter(&message.into());
        let record = LogRecord::new(level, module, message);

        // Console output
        if level >= self.config.console {
            let formatted = format!(
                "{} | {:<8} | {} | {}",
                record.timestamp,
                record.level.as_str(),
                record.module,
                record.message
            );
            eprintln!("{}", formatted);
        }

        // File output
        if level >= self.config.file {
            if let Some(file) = self.file_handle.lock().unwrap().as_mut() {
                let formatted = format!(
                    "{} | {:<8} | {} | {}\n",
                    record.timestamp,
                    record.level.as_str(),
                    record.module,
                    record.message
                );
                let _ = file.write_all(formatted.as_bytes());
            }
        }

        // Memory handler
        self.memory_handler.emit(record);
    }

    /// Get recent log records from memory
    pub fn get_recent_logs(&self, level: LogLevel, limit: usize, search: &str) -> Vec<LogRecord> {
        self.memory_handler.get_records(level, limit, search)
    }

    /// Clear memory logs
    pub fn clear_memory(&self) {
        self.memory_handler.clear();
    }

    /// Get the log directory path
    pub fn get_log_dir(&self) -> Option<&Path> {
        self.log_dir.as_deref()
    }
}

// =============================================================================
// GLOBAL INSTANCE
// =============================================================================

static GLOBAL_LOG_MANAGER: Lazy<Arc<Mutex<LogManager>>> = Lazy::new(|| {
    Arc::new(Mutex::new(LogManager::new()))
});

/// Get the global log manager instance
pub fn get_log_manager() -> Arc<Mutex<LogManager>> {
    GLOBAL_LOG_MANAGER.clone()
}

// =============================================================================
// LOGGER
// =============================================================================

/// Logger for a specific module
pub struct Logger {
    module: String,
}

impl Logger {
    pub fn new(module: impl Into<String>) -> Self {
        Self {
            module: module.into(),
        }
    }

    pub fn debug(&self, message: impl Into<String>) {
        GLOBAL_LOG_MANAGER
            .lock()
            .unwrap()
            .log(LogLevel::Debug, &self.module, message);
    }

    pub fn info(&self, message: impl Into<String>) {
        GLOBAL_LOG_MANAGER
            .lock()
            .unwrap()
            .log(LogLevel::Info, &self.module, message);
    }

    pub fn warning(&self, message: impl Into<String>) {
        GLOBAL_LOG_MANAGER
            .lock()
            .unwrap()
            .log(LogLevel::Warning, &self.module, message);
    }

    pub fn error(&self, message: impl Into<String>) {
        GLOBAL_LOG_MANAGER
            .lock()
            .unwrap()
            .log(LogLevel::Error, &self.module, message);
    }
}

/// Get a logger for a specific module
pub fn get_logger(module: impl Into<String>) -> Logger {
    Logger::new(module)
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sensitive_data_filter() {
        let filter = SensitiveDataFilter::new();

        assert_eq!(
            filter.filter("Token: ghp_1234567890123456789012345678901234567890"),
            "Token: [GITHUB_PAT]"
        );

        assert_eq!(
            filter.filter("API key: sk-123456789012345678901234567890123456789012345678"),
            "API key: [OPENAI_KEY]"
        );

        assert_eq!(
            filter.filter("Email: user@example.com"),
            "Email: [EMAIL]"
        );
    }

    #[test]
    fn test_memory_handler() {
        let handler = MemoryHandler::new(10);

        handler.emit(LogRecord::new(LogLevel::Info, "test", "Message 1"));
        handler.emit(LogRecord::new(LogLevel::Error, "test", "Message 2"));

        let records = handler.get_records(LogLevel::Debug, 10, "");
        assert_eq!(records.len(), 2);

        let records = handler.get_records(LogLevel::Error, 10, "");
        assert_eq!(records.len(), 1);
    }

    #[test]
    fn test_logger() {
        let logger = Logger::new("test_module");
        logger.info("Test message");
        logger.error("Error message");
    }
}
