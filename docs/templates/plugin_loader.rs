//! plugin_loader.rs
//!
//! Dynamic plugin/module discovery and loading system.
//!
//! This module provides a way to dynamically discover and load
//! plugin modules, enabling extensible applications.
//!
//! # Example
//!
//! ```rust
//! use plugin_loader::PluginLoader;
//! use std::path::Path;
//!
//! // Initialize with plugins directory
//! let loader = PluginLoader::new(Path::new("src/plugins"));
//!
//! // Discover all plugins
//! let plugins = loader.discover();
//!
//! // Use plugins
//! for plugin in plugins {
//!     plugin.initialize();
//! }
//! ```
//!
//! Plugin Requirements:
//!     Each plugin must implement the Plugin trait
//!     Define name, description, and priority
//!
//! # Note on Dynamic Loading in Rust
//!
//! Unlike Python's dynamic imports, Rust requires plugins to be compiled
//! as separate dynamic libraries (.so/.dll/.dylib) or statically linked.
//!
//! For true dynamic plugin loading, consider:
//! - Using libloading crate for dynamic library loading
//! - Compiling plugins as cdylib crates
//! - Using a macro-based plugin registry for compile-time registration
//!
//! This template demonstrates a simpler compile-time plugin registry approach.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// =============================================================================
// PLUGIN TRAIT
// =============================================================================

/// Trait that all plugins must implement
pub trait Plugin: Send + Sync {
    /// Plugin name
    fn name(&self) -> &str;

    /// Plugin description
    fn description(&self) -> &str;

    /// Plugin priority (lower = loads first)
    fn priority(&self) -> i32 {
        100
    }

    /// Initialize the plugin
    fn initialize(&mut self) -> Result<(), PluginError>;

    /// Shutdown the plugin
    fn shutdown(&mut self) -> Result<(), PluginError>;

    /// Check if plugin is enabled
    fn is_enabled(&self) -> bool {
        true
    }
}

/// Errors that can occur during plugin operations
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("Plugin not found: {0}")]
    NotFound(String),

    #[error("Plugin initialization failed: {0}")]
    InitFailed(String),

    #[error("Plugin already registered: {0}")]
    AlreadyRegistered(String),

    #[error("Plugin error: {0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, PluginError>;

// =============================================================================
// PLUGIN REGISTRY (Compile-time)
// =============================================================================

/// Global plugin registry
pub struct PluginRegistry {
    plugins: HashMap<String, Arc<Mutex<Box<dyn Plugin>>>>,
}

impl PluginRegistry {
    /// Create a new plugin registry
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
        }
    }

    /// Register a plugin
    pub fn register(&mut self, plugin: Box<dyn Plugin>) -> Result<()> {
        let name = plugin.name().to_string();

        if self.plugins.contains_key(&name) {
            return Err(PluginError::AlreadyRegistered(name));
        }

        self.plugins.insert(name, Arc::new(Mutex::new(plugin)));
        Ok(())
    }

    /// Get a plugin by name
    pub fn get(&self, name: &str) -> Option<Arc<Mutex<Box<dyn Plugin>>>> {
        self.plugins.get(name).cloned()
    }

    /// List all registered plugins
    pub fn list(&self) -> Vec<String> {
        self.plugins.keys().cloned().collect()
    }

    /// Initialize all plugins
    pub fn initialize_all(&self) -> Vec<Result<()>> {
        let mut plugins: Vec<_> = self.plugins.values().collect();

        // Sort by priority
        plugins.sort_by_key(|p| p.lock().unwrap().priority());

        plugins
            .into_iter()
            .map(|p| {
                let mut plugin = p.lock().unwrap();
                if plugin.is_enabled() {
                    plugin.initialize()
                } else {
                    Ok(())
                }
            })
            .collect()
    }

    /// Shutdown all plugins
    pub fn shutdown_all(&self) -> Vec<Result<()>> {
        self.plugins
            .values()
            .map(|p| p.lock().unwrap().shutdown())
            .collect()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// PLUGIN LOADER
// =============================================================================

/// Plugin loader and manager
pub struct PluginLoader {
    registry: Arc<Mutex<PluginRegistry>>,
}

impl PluginLoader {
    /// Create a new plugin loader
    pub fn new() -> Self {
        Self {
            registry: Arc::new(Mutex::new(PluginRegistry::new())),
        }
    }

    /// Register a plugin
    pub fn register(&self, plugin: Box<dyn Plugin>) -> Result<()> {
        self.registry.lock().unwrap().register(plugin)
    }

    /// Get a plugin by name
    pub fn get(&self, name: &str) -> Option<Arc<Mutex<Box<dyn Plugin>>>> {
        self.registry.lock().unwrap().get(name)
    }

    /// List all registered plugins
    pub fn list(&self) -> Vec<String> {
        self.registry.lock().unwrap().list()
    }

    /// Initialize all plugins
    pub fn initialize_all(&self) -> Vec<Result<()>> {
        self.registry.lock().unwrap().initialize_all()
    }

    /// Shutdown all plugins
    pub fn shutdown_all(&self) -> Vec<Result<()>> {
        self.registry.lock().unwrap().shutdown_all()
    }

    /// Get plugin information
    pub fn get_info(&self, name: &str) -> Option<PluginInfo> {
        self.get(name).map(|p| {
            let plugin = p.lock().unwrap();
            PluginInfo {
                name: plugin.name().to_string(),
                description: plugin.description().to_string(),
                priority: plugin.priority(),
                enabled: plugin.is_enabled(),
            }
        })
    }

    /// List all plugin information
    pub fn list_info(&self) -> Vec<PluginInfo> {
        self.list()
            .into_iter()
            .filter_map(|name| self.get_info(&name))
            .collect()
    }
}

impl Default for PluginLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Plugin information
#[derive(Debug, Clone)]
pub struct PluginInfo {
    pub name: String,
    pub description: String,
    pub priority: i32,
    pub enabled: bool,
}

// =============================================================================
// PLUGIN MACRO
// =============================================================================

/// Macro to easily create plugins
#[macro_export]
macro_rules! plugin {
    (
        name: $name:expr,
        description: $description:expr,
        priority: $priority:expr,
        init: $init:expr,
        shutdown: $shutdown:expr
    ) => {
        {
            struct CustomPlugin {
                name: String,
                description: String,
                priority: i32,
                enabled: bool,
            }

            impl Plugin for CustomPlugin {
                fn name(&self) -> &str {
                    &self.name
                }

                fn description(&self) -> &str {
                    &self.description
                }

                fn priority(&self) -> i32 {
                    self.priority
                }

                fn initialize(&mut self) -> Result<(), PluginError> {
                    let init_fn: fn() -> Result<(), PluginError> = $init;
                    init_fn()
                }

                fn shutdown(&mut self) -> Result<(), PluginError> {
                    let shutdown_fn: fn() -> Result<(), PluginError> = $shutdown;
                    shutdown_fn()
                }

                fn is_enabled(&self) -> bool {
                    self.enabled
                }
            }

            Box::new(CustomPlugin {
                name: $name.to_string(),
                description: $description.to_string(),
                priority: $priority,
                enabled: true,
            })
        }
    };
}

// =============================================================================
// EXAMPLE PLUGIN
// =============================================================================

/// Example plugin implementation
pub struct ExamplePlugin {
    name: String,
    initialized: bool,
}

impl ExamplePlugin {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            initialized: false,
        }
    }
}

impl Plugin for ExamplePlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        "An example plugin"
    }

    fn priority(&self) -> i32 {
        10
    }

    fn initialize(&mut self) -> Result<()> {
        println!("Initializing plugin: {}", self.name);
        self.initialized = true;
        Ok(())
    }

    fn shutdown(&mut self) -> Result<()> {
        println!("Shutting down plugin: {}", self.name);
        self.initialized = false;
        Ok(())
    }

    fn is_enabled(&self) -> bool {
        true
    }
}

// =============================================================================
// GLOBAL INSTANCE
// =============================================================================

use once_cell::sync::Lazy;

static GLOBAL_LOADER: Lazy<PluginLoader> = Lazy::new(PluginLoader::new);

/// Get the global plugin loader instance
pub fn get_plugin_loader() -> &'static PluginLoader {
    &GLOBAL_LOADER
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_registration() {
        let loader = PluginLoader::new();
        let plugin = Box::new(ExamplePlugin::new("test_plugin"));

        assert!(loader.register(plugin).is_ok());
        assert_eq!(loader.list().len(), 1);
        assert!(loader.get("test_plugin").is_some());
    }

    #[test]
    fn test_plugin_initialization() {
        let loader = PluginLoader::new();
        let plugin = Box::new(ExamplePlugin::new("test_plugin"));

        loader.register(plugin).unwrap();

        let results = loader.initialize_all();
        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());
    }

    #[test]
    fn test_plugin_info() {
        let loader = PluginLoader::new();
        let plugin = Box::new(ExamplePlugin::new("test_plugin"));

        loader.register(plugin).unwrap();

        let info = loader.get_info("test_plugin").unwrap();
        assert_eq!(info.name, "test_plugin");
        assert_eq!(info.description, "An example plugin");
        assert_eq!(info.priority, 10);
        assert!(info.enabled);
    }

    #[test]
    fn test_plugin_macro() {
        let _plugin = plugin! {
            name: "macro_plugin",
            description: "Plugin created with macro",
            priority: 50,
            init: || {
                println!("Init from macro");
                Ok(())
            },
            shutdown: || {
                println!("Shutdown from macro");
                Ok(())
            }
        };
    }
}
