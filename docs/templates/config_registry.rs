//! config_registry.rs
//!
//! Registry-based configuration storage.
//!
//! Handles reading/writing configuration to JSON files on disk.
//! Each section is stored as a separate file.

use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

/// File-based configuration registry
///
/// Configuration is organized into sections, each stored as a separate
/// JSON file in the config directory.
///
/// # Example structure
/// ```text
/// config_dir/
/// ├── settings.json      # "settings" section
/// ├── ui.json            # "ui" section
/// └── credentials.json   # "credentials" section
/// ```
pub struct ConfigRegistry {
    config_dir: PathBuf,
    cache: RwLock<HashMap<String, Value>>,
}

impl ConfigRegistry {
    /// Initialize the registry
    ///
    /// # Arguments
    ///
    /// * `config_dir` - Directory to store configuration files
    pub fn new<P: AsRef<Path>>(config_dir: P) -> std::io::Result<Self> {
        let config_dir = config_dir.as_ref().to_path_buf();
        fs::create_dir_all(&config_dir)?;

        let mut registry = Self {
            config_dir,
            cache: RwLock::new(HashMap::new()),
        };

        registry.load_all()?;
        Ok(registry)
    }

    /// Get the file path for a section
    fn get_file_path(&self, section: &str) -> PathBuf {
        // Sanitize section name for filesystem
        let safe_name: String = section
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
            .collect();
        self.config_dir.join(format!("{}.json", safe_name))
    }

    /// Load all configuration files into cache
    fn load_all(&mut self) -> std::io::Result<()> {
        let mut cache = self.cache.write().unwrap();

        for entry in fs::read_dir(&self.config_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Some(section) = path.file_stem().and_then(|s| s.to_str()) {
                    match fs::read_to_string(&path) {
                        Ok(contents) => match serde_json::from_str(&contents) {
                            Ok(value) => {
                                cache.insert(section.to_string(), value);
                            }
                            Err(e) => {
                                eprintln!("Warning: Failed to parse {}: {}", path.display(), e);
                                cache.insert(section.to_string(), Value::Object(Default::default()));
                            }
                        },
                        Err(e) => {
                            eprintln!("Warning: Failed to load {}: {}", path.display(), e);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Save a section to disk
    fn save_section(&self, section: &str) -> std::io::Result<()> {
        let path = self.get_file_path(section);
        let cache = self.cache.read().unwrap();

        let value = cache.get(section).unwrap_or(&Value::Object(Default::default()));
        let contents = serde_json::to_string_pretty(value)?;

        fs::write(path, contents)?;
        Ok(())
    }

    // =========================================================================
    // PUBLIC API
    // =========================================================================

    /// Get a configuration value
    pub fn get(&self, section: &str, key: &str) -> Option<Value> {
        let cache = self.cache.read().unwrap();
        cache.get(section)?.get(key).cloned()
    }

    /// Set a configuration value
    pub fn set(&mut self, section: &str, key: &str, value: Value) {
        let mut cache = self.cache.write().unwrap();

        let section_data = cache
            .entry(section.to_string())
            .or_insert_with(|| Value::Object(Default::default()));

        if let Value::Object(map) = section_data {
            map.insert(key.to_string(), value);
        }

        drop(cache);
        let _ = self.save_section(section);
    }

    /// Delete a configuration key
    pub fn delete(&mut self, section: &str, key: &str) {
        let mut cache = self.cache.write().unwrap();

        if let Some(Value::Object(map)) = cache.get_mut(section) {
            map.remove(key);
        }

        drop(cache);
        let _ = self.save_section(section);
    }

    /// Get all values in a section
    pub fn get_section(&self, section: &str) -> Option<Value> {
        let cache = self.cache.read().unwrap();
        cache.get(section).cloned()
    }

    /// Replace an entire section
    pub fn set_section(&mut self, section: &str, data: Value) {
        let mut cache = self.cache.write().unwrap();
        cache.insert(section.to_string(), data);
        drop(cache);
        let _ = self.save_section(section);
    }

    /// Delete an entire section
    pub fn delete_section(&mut self, section: &str) {
        let mut cache = self.cache.write().unwrap();
        cache.remove(section);
        drop(cache);

        let path = self.get_file_path(section);
        let _ = fs::remove_file(path);
    }

    /// Check if a key exists in a section
    pub fn has_key(&self, section: &str, key: &str) -> bool {
        let cache = self.cache.read().unwrap();
        cache
            .get(section)
            .and_then(|v| v.get(key))
            .is_some()
    }

    /// Check if a section exists
    pub fn has_section(&self, section: &str) -> bool {
        let cache = self.cache.read().unwrap();
        cache.contains_key(section)
    }

    /// Get list of all section names
    pub fn list_sections(&self) -> Vec<String> {
        let cache = self.cache.read().unwrap();
        cache.keys().cloned().collect()
    }

    /// Clear all keys in a section without deleting the section
    pub fn clear_section(&mut self, section: &str) {
        let mut cache = self.cache.write().unwrap();
        cache.insert(section.to_string(), Value::Object(Default::default()));
        drop(cache);
        let _ = self.save_section(section);
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_config_registry() {
        let temp_dir = tempdir().unwrap();
        let mut registry = ConfigRegistry::new(temp_dir.path()).unwrap();

        registry.set("settings", "theme", Value::String("dark".to_string()));

        let theme = registry.get("settings", "theme").unwrap();
        assert_eq!(theme, Value::String("dark".to_string()));

        assert!(registry.has_key("settings", "theme"));
        assert!(registry.has_section("settings"));
    }

    #[test]
    fn test_section_operations() {
        let temp_dir = tempdir().unwrap();
        let mut registry = ConfigRegistry::new(temp_dir.path()).unwrap();

        let data = serde_json::json!({
            "key1": "value1",
            "key2": "value2"
        });

        registry.set_section("test", data.clone());

        let section = registry.get_section("test").unwrap();
        assert_eq!(section, data);

        registry.delete_section("test");
        assert!(!registry.has_section("test"));
    }

    #[test]
    fn test_persistence() {
        let temp_dir = tempdir().unwrap();

        {
            let mut registry = ConfigRegistry::new(temp_dir.path()).unwrap();
            registry.set("settings", "key", Value::String("value".to_string()));
        }

        {
            let registry = ConfigRegistry::new(temp_dir.path()).unwrap();
            let value = registry.get("settings", "key").unwrap();
            assert_eq!(value, Value::String("value".to_string()));
        }
    }
}
