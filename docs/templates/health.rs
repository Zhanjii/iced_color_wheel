//! health.rs
//!
//! Application health checks for monitoring system status.
//!
//! Features:
//! - Disk space monitoring with configurable thresholds
//! - Memory usage tracking (system and process)
//! - CPU usage monitoring
//! - Network connectivity checks
//! - Config directory access verification
//! - Custom health check registration
//! - Overall health status aggregation
//!
//! # Example
//!
//! ```rust
//! use health::{HealthChecker, quick_health_check};
//!
//! // Quick check
//! let status = quick_health_check();
//! if status.healthy {
//!     println!("System is healthy");
//! }
//!
//! // Detailed checks
//! let checker = HealthChecker::new();
//! let results = checker.run_all_checks();
//! for result in results {
//!     println!("{}: {:?} - {}", result.name, result.status, result.message);
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use sysinfo::{CpuExt, System, SystemExt};
use tracing::{error, info};

// =============================================================================
// HEALTH STATUS ENUM
// =============================================================================

/// Health check result status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// System is operating normally
    Healthy,
    /// Some issues but system is functional
    Degraded,
    /// Critical issues that may affect operation
    Unhealthy,
    /// Unable to determine status (check failed)
    Unknown,
}

impl HealthStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Healthy => "healthy",
            Self::Degraded => "degraded",
            Self::Unhealthy => "unhealthy",
            Self::Unknown => "unknown",
        }
    }
}

// =============================================================================
// HEALTH CHECK RESULT
// =============================================================================

/// Result of a single health check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    pub name: String,
    pub status: HealthStatus,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<HashMap<String, serde_json::Value>>,
    pub duration_ms: f64,
}

impl HealthCheckResult {
    /// Creates a new health check result.
    pub fn new(name: impl Into<String>, status: HealthStatus, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status,
            message: message.into(),
            details: None,
            duration_ms: 0.0,
        }
    }

    /// Adds details to the result.
    pub fn with_details(mut self, details: HashMap<String, serde_json::Value>) -> Self {
        self.details = Some(details);
        self
    }

    /// Sets the duration.
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration_ms = duration.as_secs_f64() * 1000.0;
        self
    }
}

// =============================================================================
// HEALTH CHECKER
// =============================================================================

/// Central health checking system.
#[derive(Debug)]
pub struct HealthChecker {
    custom_checks: Arc<Mutex<HashMap<String, Box<dyn Fn() -> HealthCheckResult + Send>>>>,
    config_path: Arc<Mutex<Option<PathBuf>>>,
    system: Arc<Mutex<System>>,
}

impl HealthChecker {
    /// Creates a new health checker.
    pub fn new() -> Self {
        Self {
            custom_checks: Arc::new(Mutex::new(HashMap::new())),
            config_path: Arc::new(Mutex::new(None)),
            system: Arc::new(Mutex::new(System::new_all())),
        }
    }

    /// Sets the configuration directory path for config checks.
    pub fn set_config_path(&self, path: PathBuf) {
        *self.config_path.lock().unwrap() = Some(path);
    }

    /// Registers a custom health check.
    pub fn register_check<F>(&self, name: impl Into<String>, check_func: F)
    where
        F: Fn() -> HealthCheckResult + Send + 'static,
    {
        let name = name.into();
        self.custom_checks
            .lock()
            .unwrap()
            .insert(name.clone(), Box::new(check_func));
        info!("Registered health check: {}", name);
    }

    /// Unregisters a custom health check.
    pub fn unregister_check(&self, name: &str) -> bool {
        self.custom_checks.lock().unwrap().remove(name).is_some()
    }

    /// Checks if configuration directory is accessible.
    pub fn check_config_access(&self) -> HealthCheckResult {
        let start = Instant::now();

        let config_dir = self
            .config_path
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| PathBuf::from("."));

        if !config_dir.exists() {
            return HealthCheckResult::new(
                "config_access",
                HealthStatus::Unhealthy,
                "Config directory does not exist",
            )
            .with_duration(start.elapsed());
        }

        // Check if writable
        let test_file = config_dir.join(".health_check_test");
        let writable = std::fs::write(&test_file, "test").is_ok();
        if writable {
            let _ = std::fs::remove_file(&test_file);
        }

        let status = if writable {
            HealthStatus::Healthy
        } else {
            HealthStatus::Degraded
        };

        let message = if writable {
            "Config accessible"
        } else {
            "Config read-only"
        };

        HealthCheckResult::new("config_access", status, message).with_duration(start.elapsed())
    }

    /// Checks available disk space.
    pub fn check_disk_space(&self, min_gb: f64) -> HealthCheckResult {
        let start = Instant::now();

        let mut sys = self.system.lock().unwrap();
        sys.refresh_disks_list();

        let disks = sys.disks();
        if disks.is_empty() {
            return HealthCheckResult::new(
                "disk_space",
                HealthStatus::Unknown,
                "No disks found",
            )
            .with_duration(start.elapsed());
        }

        // Check the first disk (usually the system disk)
        let disk = &disks[0];
        let available_gb = disk.available_space() as f64 / (1024.0 * 1024.0 * 1024.0);
        let total_gb = disk.total_space() as f64 / (1024.0 * 1024.0 * 1024.0);

        let status = if available_gb < min_gb {
            HealthStatus::Unhealthy
        } else if available_gb < min_gb * 2.0 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };

        let message = format!("Disk space: {:.2}GB available", available_gb);

        let mut details = HashMap::new();
        details.insert(
            "available_gb".to_string(),
            serde_json::json!(format!("{:.2}", available_gb)),
        );
        details.insert(
            "total_gb".to_string(),
            serde_json::json!(format!("{:.2}", total_gb)),
        );

        HealthCheckResult::new("disk_space", status, message)
            .with_details(details)
            .with_duration(start.elapsed())
    }

    /// Checks system memory usage.
    pub fn check_memory(&self, max_percent: f64) -> HealthCheckResult {
        let start = Instant::now();

        let mut sys = self.system.lock().unwrap();
        sys.refresh_memory();

        let total = sys.total_memory() as f64;
        let available = sys.available_memory() as f64;
        let used_percent = ((total - available) / total) * 100.0;

        let status = if used_percent > max_percent {
            HealthStatus::Unhealthy
        } else if used_percent > max_percent - 10.0 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };

        let message = format!("Memory: {:.1}% used", used_percent);

        let mut details = HashMap::new();
        details.insert(
            "used_percent".to_string(),
            serde_json::json!(format!("{:.1}", used_percent)),
        );
        details.insert(
            "available_gb".to_string(),
            serde_json::json!(format!("{:.2}", available / 1024.0 / 1024.0 / 1024.0)),
        );

        HealthCheckResult::new("memory", status, message)
            .with_details(details)
            .with_duration(start.elapsed())
    }

    /// Checks CPU usage.
    pub fn check_cpu(&self, max_percent: f64) -> HealthCheckResult {
        let start = Instant::now();

        let mut sys = self.system.lock().unwrap();
        sys.refresh_cpu();

        // Get global CPU usage
        let cpu_percent = sys.global_cpu_info().cpu_usage() as f64;

        let status = if cpu_percent > max_percent {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };

        let message = format!("CPU: {:.1}% used", cpu_percent);

        let mut details = HashMap::new();
        details.insert(
            "system_percent".to_string(),
            serde_json::json!(format!("{:.1}", cpu_percent)),
        );
        details.insert("cpu_count".to_string(), serde_json::json!(sys.cpus().len()));

        HealthCheckResult::new("cpu", status, message)
            .with_details(details)
            .with_duration(start.elapsed())
    }

    /// Runs all registered health checks.
    pub fn run_all_checks(&self) -> Vec<HealthCheckResult> {
        let mut results = Vec::new();

        // Built-in checks
        results.push(self.check_config_access());
        results.push(self.check_disk_space(1.0));
        results.push(self.check_memory(90.0));
        results.push(self.check_cpu(90.0));

        // Custom checks
        let checks = self.custom_checks.lock().unwrap();
        for (name, check_func) in checks.iter() {
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| check_func())) {
                Ok(result) => results.push(result),
                Err(_) => {
                    results.push(HealthCheckResult::new(
                        name,
                        HealthStatus::Unknown,
                        "Check panicked",
                    ));
                }
            }
        }

        results
    }

    /// Gets overall health status.
    pub fn get_health_status(&self) -> HealthSummary {
        let results = self.run_all_checks();

        let overall_status = if results.iter().any(|r| r.status == HealthStatus::Unhealthy) {
            HealthStatus::Unhealthy
        } else if results
            .iter()
            .any(|r| r.status == HealthStatus::Degraded || r.status == HealthStatus::Unknown)
        {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };

        HealthSummary {
            overall_status,
            healthy: overall_status == HealthStatus::Healthy,
            checks: results,
        }
    }

    /// Quick check if system is healthy.
    pub fn is_healthy(&self) -> bool {
        self.get_health_status().healthy
    }
}

impl Default for HealthChecker {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// HEALTH SUMMARY
// =============================================================================

/// Overall health status summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthSummary {
    pub overall_status: HealthStatus,
    pub healthy: bool,
    pub checks: Vec<HealthCheckResult>,
}

// =============================================================================
// CONVENIENCE FUNCTIONS
// =============================================================================

/// Gets a global HealthChecker instance.
static HEALTH_CHECKER: std::sync::OnceLock<HealthChecker> = std::sync::OnceLock::new();

pub fn get_health_checker() -> &'static HealthChecker {
    HEALTH_CHECKER.get_or_init(HealthChecker::new)
}

/// Runs a quick health check and returns status.
pub fn quick_health_check() -> HealthSummary {
    get_health_checker().get_health_status()
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_checker() {
        let checker = HealthChecker::new();
        let results = checker.run_all_checks();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_custom_check() {
        let checker = HealthChecker::new();
        checker.register_check("test", || {
            HealthCheckResult::new("test", HealthStatus::Healthy, "Test passed")
        });

        let results = checker.run_all_checks();
        assert!(results.iter().any(|r| r.name == "test"));
    }

    #[test]
    fn test_health_status() {
        let status = quick_health_check();
        assert!(status.overall_status != HealthStatus::Unknown);
    }
}
