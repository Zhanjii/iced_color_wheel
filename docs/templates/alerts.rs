//! alerts.rs
//!
//! Audio alert system for desktop applications.
//!
//! Provides sound notifications for different event types using system beeps.
//! Works on Windows, macOS, and Linux with platform-specific implementations.
//!
//! # Example
//!
//! ```rust
//! use alerts::{AlertManager, AlertType};
//!
//! // Get singleton instance
//! let mut alerts = AlertManager::get_instance();
//! alerts.initialize(true);
//!
//! // Play alerts
//! alerts.success("Build completed!");
//! alerts.error("Connection failed");
//! alerts.alert(AlertType::TaskComplete, "Task finished");
//!
//! // Disable sounds temporarily
//! alerts.set_sound_enabled(false);
//! ```

use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// =============================================================================
// ALERT TYPES
// =============================================================================

/// Types of alerts with associated sounds
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AlertType {
    Info,
    Success,
    Warning,
    Error,
    TaskComplete,
    Intervention, // Needs user attention
}

impl AlertType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::TaskComplete => "task_complete",
            Self::Intervention => "intervention",
        }
    }
}

// =============================================================================
// ALERT SOUND CONFIGURATION
// =============================================================================

/// Sound configuration for an alert type
#[derive(Debug, Clone)]
pub struct AlertSound {
    /// Windows system sounds (frequency in Hz, duration in ms)
    pub frequency: u32,
    pub duration: u64,
    /// Number of beeps
    pub beeps: u32,
    /// Delay between beeps (ms)
    pub delay: u64,
}

impl AlertSound {
    pub const fn new(frequency: u32, duration: u64, beeps: u32, delay: u64) -> Self {
        Self {
            frequency,
            duration,
            beeps,
            delay,
        }
    }
}

/// Default sound configurations for each alert type
fn default_sounds() -> HashMap<AlertType, AlertSound> {
    let mut sounds = HashMap::new();

    sounds.insert(
        AlertType::Info,
        AlertSound::new(600, 100, 1, 0),
    );
    sounds.insert(
        AlertType::Success,
        AlertSound::new(800, 150, 2, 100),
    );
    sounds.insert(
        AlertType::Warning,
        AlertSound::new(500, 300, 2, 150),
    );
    sounds.insert(
        AlertType::Error,
        AlertSound::new(400, 500, 3, 200),
    );
    sounds.insert(
        AlertType::TaskComplete,
        AlertSound::new(1000, 100, 3, 80),
    );
    sounds.insert(
        AlertType::Intervention,
        AlertSound::new(600, 200, 4, 150),
    );

    sounds
}

// =============================================================================
// ALERT MANAGER
// =============================================================================

/// Callback function type for alerts
pub type AlertCallback = Box<dyn Fn(&str) + Send + Sync>;

/// Manages application alerts with sound notifications
pub struct AlertManager {
    sound_enabled: bool,
    sounds: HashMap<AlertType, AlertSound>,
    callbacks: HashMap<AlertType, Vec<Arc<AlertCallback>>>,
    platform: String,
}

impl AlertManager {
    fn new() -> Self {
        Self {
            sound_enabled: true,
            sounds: default_sounds(),
            callbacks: HashMap::new(),
            platform: std::env::consts::OS.to_string(),
        }
    }

    /// Initialize or reinitialize the alert manager
    pub fn initialize(&mut self, sound_enabled: bool) {
        self.sound_enabled = sound_enabled;
    }

    /// Enable or disable sound alerts
    pub fn set_sound_enabled(&mut self, enabled: bool) {
        self.sound_enabled = enabled;
    }

    /// Check if sound alerts are enabled
    pub fn is_sound_enabled(&self) -> bool {
        self.sound_enabled
    }

    /// Register a callback to be called when an alert fires
    pub fn register_callback(&mut self, alert_type: AlertType, callback: AlertCallback) {
        self.callbacks
            .entry(alert_type)
            .or_insert_with(Vec::new)
            .push(Arc::new(callback));
    }

    /// Trigger an alert
    pub fn alert(&self, alert_type: AlertType, message: impl Into<String>) {
        self.alert_with_override(alert_type, message, None);
    }

    /// Trigger an alert with sound override
    pub fn alert_with_override(
        &self,
        alert_type: AlertType,
        message: impl Into<String>,
        play_sound: Option<bool>,
    ) {
        let message = message.into();
        let should_play = play_sound.unwrap_or(self.sound_enabled);

        if !message.is_empty() {
            eprintln!("Alert [{}]: {}", alert_type.as_str(), message);
        }

        // Fire callbacks
        if let Some(callbacks) = self.callbacks.get(&alert_type) {
            for callback in callbacks {
                let callback = Arc::clone(callback);
                let msg = message.clone();
                thread::spawn(move || {
                    callback(&msg);
                });
            }
        }

        // Play sound in background thread
        if should_play {
            if let Some(sound) = self.sounds.get(&alert_type) {
                let sound = sound.clone();
                let platform = self.platform.clone();
                thread::spawn(move || {
                    play_sound(&sound, &platform);
                });
            }
        }
    }

    // Convenience methods
    pub fn info(&self, message: impl Into<String>) {
        self.alert(AlertType::Info, message);
    }

    pub fn success(&self, message: impl Into<String>) {
        self.alert(AlertType::Success, message);
    }

    pub fn warning(&self, message: impl Into<String>) {
        self.alert(AlertType::Warning, message);
    }

    pub fn error(&self, message: impl Into<String>) {
        self.alert(AlertType::Error, message);
    }

    pub fn task_complete(&self, message: impl Into<String>) {
        self.alert(AlertType::TaskComplete, message);
    }

    pub fn intervention(&self, message: impl Into<String>) {
        self.alert(AlertType::Intervention, message);
    }
}

// =============================================================================
// SOUND PLAYBACK
// =============================================================================

/// Play the alert sound (platform-specific)
fn play_sound(sound: &AlertSound, platform: &str) {
    for i in 0..sound.beeps {
        if i > 0 {
            thread::sleep(Duration::from_millis(sound.delay));
        }

        match platform {
            "windows" => beep_windows(sound.frequency, sound.duration),
            "macos" => beep_macos(),
            _ => beep_linux(),
        }
    }
}

/// Play a beep on Windows using winapi
#[cfg(target_os = "windows")]
fn beep_windows(frequency: u32, duration: u64) {
    use winapi::um::utilapiset::Beep;
    unsafe {
        Beep(frequency, duration as u32);
    }
}

#[cfg(not(target_os = "windows"))]
fn beep_windows(_frequency: u32, _duration: u64) {
    // Fallback for non-Windows platforms
    print!("\x07");
}

/// Play a beep on macOS using system sound
fn beep_macos() {
    use std::process::Command;

    let _ = Command::new("afplay")
        .arg("/System/Library/Sounds/Ping.aiff")
        .output();
}

/// Play a beep on Linux
fn beep_linux() {
    use std::process::Command;

    // Try paplay (PulseAudio)
    let _ = Command::new("paplay")
        .arg("/usr/share/sounds/freedesktop/stereo/complete.oga")
        .output();
}

// =============================================================================
// GLOBAL INSTANCE
// =============================================================================

static GLOBAL_ALERT_MANAGER: Lazy<Arc<Mutex<AlertManager>>> = Lazy::new(|| {
    Arc::new(Mutex::new(AlertManager::new()))
});

/// Get the global AlertManager instance
pub fn get_alert_manager() -> Arc<Mutex<AlertManager>> {
    GLOBAL_ALERT_MANAGER.clone()
}

// =============================================================================
// CONVENIENCE FUNCTIONS
// =============================================================================

/// Play a success sound
pub fn play_success_sound() {
    GLOBAL_ALERT_MANAGER.lock().unwrap().success("");
}

/// Play an error sound
pub fn play_error_sound() {
    GLOBAL_ALERT_MANAGER.lock().unwrap().error("");
}

/// Play a notification sound
pub fn play_notification_sound() {
    GLOBAL_ALERT_MANAGER.lock().unwrap().info("");
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alert_manager_initialization() {
        let mut manager = AlertManager::new();
        manager.initialize(false);
        assert!(!manager.is_sound_enabled());

        manager.set_sound_enabled(true);
        assert!(manager.is_sound_enabled());
    }

    #[test]
    fn test_default_sounds() {
        let sounds = default_sounds();
        assert_eq!(sounds.len(), 6);
        assert!(sounds.contains_key(&AlertType::Info));
        assert!(sounds.contains_key(&AlertType::Success));
    }

    #[test]
    fn test_alert_without_sound() {
        let mut manager = AlertManager::new();
        manager.set_sound_enabled(false);
        manager.info("Test info message");
        manager.success("Test success message");
    }
}
