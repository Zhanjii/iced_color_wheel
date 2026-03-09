//! config.rs
//!
//! Configuration management with thread-safe singleton pattern.
//!
//! This module provides:
//! - ConfigManager: Main configuration access point
//! - Observer pattern for config change notifications
//! - Section-based organization

use crate::config_registry::ConfigRegistry;
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};
use once_cell::sync::Lazy;

// =============================================================================
// OBSERVER CALLBACK TYPE
// =============================================================================

type ObserverCallback = Box<dyn Fn(Option<&str>, &serde_json::Value, &str) + Send + Sync>;

// =============================================================================
// CONFIG MANAGER
// =============================================================================

/// Thread-safe singleton configuration manager
pub struct ConfigManager {
    registry: Option<ConfigRegistry>,
    observers: Vec<ObserverCallback>,
}

static INSTANCE: Lazy<Arc<RwLock<ConfigManager>>> = Lazy::new(|| {
    Arc::new(RwLock::new(ConfigManager {
        registry: None,
        observers: Vec::new(),
    }))
});

impl ConfigManager {
    /// Get the singleton instance
    pub fn get_instance() -> Arc<RwLock<ConfigManager>> {
        Arc::clone(&INSTANCE)
    }

    /// Initialize the configuration system
    pub fn initialize<P: AsRef<Path>>(config_dir: P) -> Result<(), std::io::Error> {
        let mut instance = INSTANCE.write().unwrap();
        instance.registry = Some(ConfigRegistry::new(config_dir)?);
        Ok(())
    }

    /// Reset singleton (for testing)
    pub fn reset() {
        let mut instance = INSTANCE.write().unwrap();
        instance.registry = None;
        instance.observers.clear();
    }

    // =========================================================================
    // BASIC ACCESS
    // =========================================================================

    /// Get a configuration value
    pub fn get(&self, key: &str, section: Option<&str>) -> Option<serde_json::Value> {
        let section = section.unwrap_or("settings");
        self.registry.as_ref()?.get(section, key)
    }

    /// Set a configuration value
    pub fn set(&mut self, key: &str, value: serde_json::Value, section: Option<&str>) {
        let section = section.unwrap_or("settings");
        if let Some(registry) = &mut self.registry {
            registry.set(section, key, value.clone());
            self.notify_observers(Some(key), &value, section);
        }
    }

    /// Delete a configuration key
    pub fn delete(&mut self, key: &str, section: Option<&str>) {
        let section = section.unwrap_or("settings");
        if let Some(registry) = &mut self.registry {
            registry.delete(section, key);
        }
    }

    // =========================================================================
    // SECTION ACCESS
    // =========================================================================

    /// Get all values in a section
    pub fn get_section(&self, section: &str) -> Option<serde_json::Value> {
        self.registry.as_ref()?.get_section(section)
    }

    /// Replace an entire section
    pub fn set_section(&mut self, section: &str, data: serde_json::Value) {
        if let Some(registry) = &mut self.registry {
            registry.set_section(section, data.clone());
            self.notify_observers(None, &data, section);
        }
    }

    // =========================================================================
    // OBSERVER PATTERN
    // =========================================================================

    /// Add an observer for configuration changes
    ///
    /// The callback receives: (key, value, section)
    /// - key is None when entire section is replaced
    pub fn add_observer<F>(&mut self, callback: F)
    where
        F: Fn(Option<&str>, &serde_json::Value, &str) + Send + Sync + 'static,
    {
        self.observers.push(Box::new(callback));
    }

    /// Notify all observers of a change
    fn notify_observers(&self, key: Option<&str>, value: &serde_json::Value, section: &str) {
        for callback in &self.observers {
            callback(key, value, section);
        }
    }

    // =========================================================================
    // CONVENIENCE METHODS
    // =========================================================================

    /// Get a boolean value
    pub fn get_bool(&self, key: &str, section: Option<&str>) -> Option<bool> {
        self.get(key, section)?.as_bool()
    }

    /// Get an integer value
    pub fn get_int(&self, key: &str, section: Option<&str>) -> Option<i64> {
        self.get(key, section)?.as_i64()
    }

    /// Get a float value
    pub fn get_float(&self, key: &str, section: Option<&str>) -> Option<f64> {
        self.get(key, section)?.as_f64()
    }

    /// Get a string value
    pub fn get_string(&self, key: &str, section: Option<&str>) -> Option<String> {
        self.get(key, section)?.as_str().map(String::from)
    }
}

// =============================================================================
// CONVENIENCE FUNCTIONS
// =============================================================================

/// Get a configuration value
pub fn get(key: &str, section: Option<&str>) -> Option<serde_json::Value> {
    let instance = INSTANCE.read().unwrap();
    instance.get(key, section)
}

/// Set a configuration value
pub fn set(key: &str, value: serde_json::Value, section: Option<&str>) {
    let mut instance = INSTANCE.write().unwrap();
    instance.set(key, value, section);
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_config_manager() {
        ConfigManager::reset();

        let temp_dir = tempdir().unwrap();
        ConfigManager::initialize(temp_dir.path()).unwrap();

        {
            let mut instance = INSTANCE.write().unwrap();
            instance.set("theme", serde_json::json!("dark"), None);
        }

        {
            let instance = INSTANCE.read().unwrap();
            let theme = instance.get("theme", None).unwrap();
            assert_eq!(theme, serde_json::json!("dark"));
        }
    }

    #[test]
    fn test_observer() {
        ConfigManager::reset();

        let temp_dir = tempdir().unwrap();
        ConfigManager::initialize(temp_dir.path()).unwrap();

        let called = Arc::new(Mutex::new(false));
        let called_clone = Arc::clone(&called);

        {
            let mut instance = INSTANCE.write().unwrap();
            instance.add_observer(move |key, _value, _section| {
                if key == Some("test") {
                    *called_clone.lock().unwrap() = true;
                }
            });

            instance.set("test", serde_json::json!("value"), None);
        }

        assert!(*called.lock().unwrap());
    }
}
