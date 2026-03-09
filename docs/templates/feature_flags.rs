//! feature_flags.rs
//!
//! Runtime feature flag management system.
//!
//! Enable/disable features at runtime without code changes.
//! Useful for:
//! - Gradual feature rollouts
//! - A/B testing
//! - Development toggles
//! - Emergency kill switches
//!
//! # Example
//!
//! ```rust
//! use feature_flags::{FeatureFlags, is_enabled};
//!
//! // Initialize flags with defaults
//! let mut flags = FeatureFlags::new();
//! flags.set_defaults(vec![
//!     ("dark_mode", true),
//!     ("new_export", false),
//!     ("beta_features", false),
//! ]);
//!
//! // Check flag in code
//! if flags.is_enabled("new_export") {
//!     show_new_export_ui();
//! } else {
//!     show_legacy_export_ui();
//! }
//!
//! // Global convenience function
//! if is_enabled("dark_mode") {
//!     apply_dark_theme();
//! }
//! ```

use once_cell::sync::Lazy;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

// =============================================================================
// OBSERVER CALLBACK
// =============================================================================

/// Callback function type for flag changes
pub type FlagCallback = Box<dyn Fn(&str, bool) + Send + Sync>;

// =============================================================================
// FEATURE FLAGS
// =============================================================================

/// Runtime feature flag manager
pub struct FeatureFlags {
    flags: HashMap<String, bool>,
    defaults: HashMap<String, bool>,
    locked: HashSet<String>,
    observers: HashMap<String, Vec<Arc<FlagCallback>>>,
}

impl FeatureFlags {
    /// Create a new feature flags manager
    pub fn new() -> Self {
        Self {
            flags: HashMap::new(),
            defaults: HashMap::new(),
            locked: HashSet::new(),
            observers: HashMap::new(),
        }
    }

    /// Set default flag values
    pub fn set_defaults(&mut self, defaults: impl IntoIterator<Item = (impl Into<String>, bool)>) {
        for (flag, value) in defaults {
            let flag = flag.into();
            self.defaults.insert(flag.clone(), value);

            // Initialize flags that haven't been set
            if !self.flags.contains_key(&flag) {
                self.flags.insert(flag, value);
            }
        }
    }

    /// Check if a feature flag is enabled
    pub fn is_enabled(&self, flag: &str) -> bool {
        self.flags
            .get(flag)
            .or_else(|| self.defaults.get(flag))
            .copied()
            .unwrap_or(false)
    }

    /// Enable a feature flag
    pub fn enable(&mut self, flag: impl Into<String>) -> bool {
        let flag = flag.into();

        if self.locked.contains(&flag) {
            eprintln!("Cannot enable locked flag: {}", flag);
            return false;
        }

        let old_value = self.is_enabled(&flag);
        self.flags.insert(flag.clone(), true);

        if !old_value {
            self.notify_observers(&flag, true);
        }

        true
    }

    /// Disable a feature flag
    pub fn disable(&mut self, flag: impl Into<String>) -> bool {
        let flag = flag.into();

        if self.locked.contains(&flag) {
            eprintln!("Cannot disable locked flag: {}", flag);
            return false;
        }

        let old_value = self.is_enabled(&flag);
        self.flags.insert(flag.clone(), false);

        if old_value {
            self.notify_observers(&flag, false);
        }

        true
    }

    /// Toggle a feature flag
    pub fn toggle(&mut self, flag: impl Into<String>) -> bool {
        let flag = flag.into();
        let current = self.is_enabled(&flag);

        if current {
            self.disable(flag.clone());
        } else {
            self.enable(flag.clone());
        }

        !current
    }

    /// Set a feature flag to a specific value
    pub fn set(&mut self, flag: impl Into<String>, value: bool) -> bool {
        if value {
            self.enable(flag)
        } else {
            self.disable(flag)
        }
    }

    /// Lock a flag to prevent changes
    pub fn lock(&mut self, flag: impl Into<String>) {
        self.locked.insert(flag.into());
    }

    /// Unlock a flag to allow changes
    pub fn unlock(&mut self, flag: impl Into<String>) {
        self.locked.remove(&flag.into());
    }

    /// Check if a flag is locked
    pub fn is_locked(&self, flag: &str) -> bool {
        self.locked.contains(flag)
    }

    /// Load flag values from configuration
    pub fn load_from_config(&mut self, config: HashMap<String, bool>) -> usize {
        let mut updated = 0;

        for (flag, value) in config {
            if self.locked.contains(&flag) {
                continue;
            }

            let old_value = self.flags.get(&flag).copied();
            self.flags.insert(flag.clone(), value);

            if old_value != Some(value) {
                self.notify_observers(&flag, value);
                updated += 1;
            }
        }

        updated
    }

    /// Export all flag values
    pub fn to_dict(&self) -> HashMap<String, bool> {
        let mut result = self.defaults.clone();
        result.extend(self.flags.clone());
        result
    }

    /// Reset flags to defaults
    pub fn reset(&mut self, flag: Option<&str>) {
        if let Some(flag) = flag {
            if self.locked.contains(flag) {
                eprintln!("Cannot reset locked flag: {}", flag);
                return;
            }

            if let Some(&default) = self.defaults.get(flag) {
                self.flags.insert(flag.to_string(), default);
            } else {
                self.flags.remove(flag);
            }
        } else {
            // Reset all non-locked flags
            let locked_flags: HashMap<_, _> = self.flags
                .iter()
                .filter(|(k, _)| self.locked.contains(k.as_str()))
                .map(|(k, v)| (k.clone(), *v))
                .collect();

            self.flags = locked_flags;

            for (flag, value) in &self.defaults {
                if !self.locked.contains(flag) {
                    self.flags.insert(flag.clone(), *value);
                }
            }
        }
    }

    /// Add observer for flag changes
    pub fn add_observer(&mut self, flag: impl Into<String>, callback: FlagCallback) {
        let flag = flag.into();
        self.observers
            .entry(flag)
            .or_insert_with(Vec::new)
            .push(Arc::new(callback));
    }

    /// Notify observers of flag change
    fn notify_observers(&self, flag: &str, value: bool) {
        // Notify flag-specific observers
        if let Some(observers) = self.observers.get(flag) {
            for observer in observers {
                let observer = Arc::clone(observer);
                let flag = flag.to_string();
                std::thread::spawn(move || {
                    observer(&flag, value);
                });
            }
        }

        // Notify wildcard observers
        if let Some(observers) = self.observers.get("*") {
            for observer in observers {
                let observer = Arc::clone(observer);
                let flag = flag.to_string();
                std::thread::spawn(move || {
                    observer(&flag, value);
                });
            }
        }
    }

    /// List all flags with their status
    pub fn list_flags(&self) -> Vec<FlagInfo> {
        let mut all_flags: HashSet<_> = self.defaults.keys()
            .chain(self.flags.keys())
            .cloned()
            .collect();

        let mut info_list: Vec<_> = all_flags
            .drain()
            .map(|flag| FlagInfo {
                name: flag.clone(),
                enabled: self.is_enabled(&flag),
                default: self.defaults.get(&flag).copied(),
                locked: self.locked.contains(&flag),
            })
            .collect();

        info_list.sort_by(|a, b| a.name.cmp(&b.name));
        info_list
    }
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self::new()
    }
}

/// Information about a feature flag
#[derive(Debug, Clone)]
pub struct FlagInfo {
    pub name: String,
    pub enabled: bool,
    pub default: Option<bool>,
    pub locked: bool,
}

// =============================================================================
// GLOBAL INSTANCE
// =============================================================================

static GLOBAL_FLAGS: Lazy<Arc<Mutex<FeatureFlags>>> = Lazy::new(|| {
    Arc::new(Mutex::new(FeatureFlags::new()))
});

/// Get the global feature flags instance
pub fn get_flags() -> Arc<Mutex<FeatureFlags>> {
    GLOBAL_FLAGS.clone()
}

/// Check if a feature flag is enabled (global convenience function)
pub fn is_enabled(flag: &str) -> bool {
    GLOBAL_FLAGS.lock().unwrap().is_enabled(flag)
}

/// Set a feature flag value (global convenience function)
pub fn set_flag(flag: impl Into<String>, value: bool) -> bool {
    GLOBAL_FLAGS.lock().unwrap().set(flag, value)
}

// =============================================================================
// CONTEXT MANAGER
// =============================================================================

/// Context manager for temporarily changing flag values
pub struct FeatureFlagContext {
    flags: Arc<Mutex<FeatureFlags>>,
    overrides: HashMap<String, bool>,
    original_values: HashMap<String, Option<bool>>,
}

impl FeatureFlagContext {
    /// Create a new context with overrides
    pub fn new(flags: Arc<Mutex<FeatureFlags>>, overrides: HashMap<String, bool>) -> Self {
        let mut original_values = HashMap::new();

        // Save original values and apply overrides
        {
            let mut flags_lock = flags.lock().unwrap();
            for (flag, value) in &overrides {
                original_values.insert(flag.clone(), flags_lock.flags.get(flag).copied());
                flags_lock.flags.insert(flag.clone(), *value);
            }
        }

        Self {
            flags,
            overrides,
            original_values,
        }
    }
}

impl Drop for FeatureFlagContext {
    fn drop(&mut self) {
        // Restore original values
        let mut flags = self.flags.lock().unwrap();
        for (flag, original) in &self.original_values {
            if let Some(value) = original {
                flags.flags.insert(flag.clone(), *value);
            } else {
                flags.flags.remove(flag);
            }
        }
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flag_enable_disable() {
        let mut flags = FeatureFlags::new();
        flags.set_defaults(vec![("test_flag", false)]);

        assert!(!flags.is_enabled("test_flag"));

        flags.enable("test_flag");
        assert!(flags.is_enabled("test_flag"));

        flags.disable("test_flag");
        assert!(!flags.is_enabled("test_flag"));
    }

    #[test]
    fn test_flag_locking() {
        let mut flags = FeatureFlags::new();
        flags.set_defaults(vec![("locked_flag", true)]);

        flags.lock("locked_flag");
        assert!(flags.is_locked("locked_flag"));

        assert!(!flags.disable("locked_flag"));
        assert!(flags.is_enabled("locked_flag"));
    }

    #[test]
    fn test_flag_toggle() {
        let mut flags = FeatureFlags::new();
        flags.set_defaults(vec![("toggle_flag", false)]);

        assert!(!flags.is_enabled("toggle_flag"));

        let new_value = flags.toggle("toggle_flag");
        assert!(new_value);
        assert!(flags.is_enabled("toggle_flag"));

        let new_value = flags.toggle("toggle_flag");
        assert!(!new_value);
        assert!(!flags.is_enabled("toggle_flag"));
    }

    #[test]
    fn test_load_from_config() {
        let mut flags = FeatureFlags::new();
        flags.set_defaults(vec![("flag1", false), ("flag2", false)]);

        let mut config = HashMap::new();
        config.insert("flag1".to_string(), true);
        config.insert("flag2".to_string(), true);

        let updated = flags.load_from_config(config);
        assert_eq!(updated, 2);

        assert!(flags.is_enabled("flag1"));
        assert!(flags.is_enabled("flag2"));
    }

    #[test]
    fn test_list_flags() {
        let mut flags = FeatureFlags::new();
        flags.set_defaults(vec![("flag1", true), ("flag2", false)]);

        let list = flags.list_flags();
        assert_eq!(list.len(), 2);

        let flag1 = list.iter().find(|f| f.name == "flag1").unwrap();
        assert!(flag1.enabled);
        assert_eq!(flag1.default, Some(true));
    }
}
