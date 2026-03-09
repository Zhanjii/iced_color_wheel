//! Windows title bar dark/light mode styling.
//!
//! Uses DWM API to match title bar appearance with app theme.
//! No-op on non-Windows platforms.
//!
//! Features:
//! - `set_title_bar_style(hwnd, dark_mode)` for direct HWND control
//! - Uses `DWMWA_USE_IMMERSIVE_DARK_MODE` (attribute 20), fallback to 19
//! - `set_title_bar_from_iced` convenience for iced windows
//! - Compile-time no-op on non-Windows via `#[cfg(target_os = "windows")]`
//!
//! # Example
//!
//! ```rust
//! use title_bar::set_title_bar_style;
//!
//! // Direct HWND usage:
//! set_title_bar_style(hwnd, true)?; // dark mode
//!
//! // Via iced window (in your App::update after window opens):
//! set_title_bar_from_iced(window_id, true);
//! ```

// =============================================================================
// WINDOWS IMPLEMENTATION
// =============================================================================

#[cfg(target_os = "windows")]
mod platform {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWINDOWATTRIBUTE};

    /// DWMWA_USE_IMMERSIVE_DARK_MODE (Windows 10 build 18985+).
    const DWMWA_USE_IMMERSIVE_DARK_MODE: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(20);

    /// Fallback attribute for older Windows 10 builds.
    const DWMWA_USE_IMMERSIVE_DARK_MODE_LEGACY: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(19);

    /// Set the Windows title bar to dark or light mode.
    ///
    /// Uses `DwmSetWindowAttribute` with `DWMWA_USE_IMMERSIVE_DARK_MODE`
    /// (attribute 20). Falls back to attribute 19 for older Windows 10 builds.
    ///
    /// # Arguments
    ///
    /// * `hwnd` - The window handle.
    /// * `dark_mode` - `true` for dark title bar, `false` for light.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or an error string describing the failure.
    pub fn set_title_bar_style(hwnd: isize, dark_mode: bool) -> Result<(), String> {
        let hwnd = HWND(hwnd as *mut _);
        let value: i32 = if dark_mode { 1 } else { 0 };

        // Try attribute 20 first (modern Windows 10/11)
        let result = unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &value as *const i32 as *const _,
                std::mem::size_of::<i32>() as u32,
            )
        };

        if result.is_ok() {
            return Ok(());
        }

        // Fallback to attribute 19 (older Windows 10)
        let result = unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE_LEGACY,
                &value as *const i32 as *const _,
                std::mem::size_of::<i32>() as u32,
            )
        };

        result.map_err(|e| format!("DwmSetWindowAttribute failed: {e}"))
    }

    /// Extract the raw HWND from an iced window via the winit backend.
    ///
    /// Iced 0.14 uses winit under the hood. After a window is opened, you
    /// can obtain the HWND through the `raw-window-handle` crate. This
    /// requires calling from a context where you have access to the winit
    /// window (e.g., a custom subscription or platform-specific hook).
    ///
    /// # Approach
    ///
    /// In practice, the simplest way to get the HWND in iced 0.14 is:
    ///
    /// 1. Use a winit-level event subscription to capture the window handle
    ///    when the window is created.
    /// 2. Store the HWND in your App state keyed by `window::Id`.
    /// 3. Call `set_title_bar_style(hwnd, dark_mode)` from your update().
    ///
    /// ```rust
    /// // In your App state:
    /// hwnd_map: HashMap<window::Id, isize>,
    ///
    /// // After capturing the HWND (platform-specific):
    /// use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};
    ///
    /// if let RawWindowHandle::Win32(handle) = winit_window.raw_window_handle() {
    ///     let hwnd = handle.hwnd as isize;
    ///     self.hwnd_map.insert(window_id, hwnd);
    /// }
    ///
    /// // Then apply the style:
    /// if let Some(&hwnd) = self.hwnd_map.get(&window_id) {
    ///     let _ = title_bar::set_title_bar_style(hwnd, true);
    /// }
    /// ```
    ///
    /// NOTE: Direct HWND extraction from iced's public API is not yet
    /// straightforward in 0.14. The above pattern works when you have
    /// access to the underlying winit Window object. Future iced versions
    /// may expose window handles more directly.
    pub fn hwnd_from_iced_hint() -> &'static str {
        "Use raw-window-handle on the underlying winit::Window. \
         See doc comment for the recommended pattern."
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    /// No-op on non-Windows platforms.
    pub fn set_title_bar_style(_hwnd: isize, _dark_mode: bool) -> Result<(), String> {
        Ok(())
    }

    /// Always returns a hint string on non-Windows platforms.
    pub fn hwnd_from_iced_hint() -> &'static str {
        "HWND is a Windows-only concept. This function is a no-op on this platform."
    }
}

// =============================================================================
// PUBLIC API
// =============================================================================

/// Set the Windows title bar to dark or light mode.
///
/// On non-Windows platforms, this is a no-op that always returns `Ok(())`.
///
/// # Arguments
///
/// * `hwnd` - The raw window handle (HWND on Windows).
/// * `dark_mode` - `true` for a dark title bar, `false` for light.
///
/// # Example
///
/// ```rust
/// // Apply dark title bar to match your iced dark theme
/// set_title_bar_style(hwnd, true)?;
///
/// // Switch to light title bar
/// set_title_bar_style(hwnd, false)?;
/// ```
pub fn set_title_bar_style(hwnd: isize, dark_mode: bool) -> Result<(), String> {
    platform::set_title_bar_style(hwnd, dark_mode)
}

/// Convenience: apply title bar style to an iced window by HWND.
///
/// You must obtain the HWND first (see platform::hwnd_from_iced_hint docs).
/// This is a thin wrapper that logs errors instead of propagating them,
/// since title bar styling is cosmetic and should not crash the app.
///
/// # Example
///
/// ```rust
/// // In your App::update(), after opening a window:
/// if let Some(&hwnd) = self.hwnd_map.get(&window_id) {
///     set_title_bar_for_window(hwnd, matches!(self.theme, Theme::Dark));
/// }
/// ```
pub fn set_title_bar_for_window(hwnd: isize, dark_mode: bool) {
    if let Err(e) = set_title_bar_style(hwnd, dark_mode) {
        log::warn!("Failed to set title bar style: {}", e);
    }
}

/// Get a hint string about how to extract HWND from iced's winit backend.
pub fn hwnd_extraction_hint() -> &'static str {
    platform::hwnd_from_iced_hint()
}

// =============================================================================
// DEPENDENCIES
// =============================================================================

// Add to Cargo.toml:
//
// [target.'cfg(target_os = "windows")'.dependencies]
// windows = { version = "0.58", features = [
//     "Win32_Foundation",
//     "Win32_Graphics_Dwm",
// ]}
//
// [dependencies]
// iced = { version = "0.14", features = ["multi-window"] }
// raw-window-handle = "0.6"   # For HWND extraction from winit
// log = "0.4"

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(not(target_os = "windows"))]
    fn test_noop_on_non_windows() {
        let result = set_title_bar_style(0, true);
        assert!(result.is_ok());

        let result = set_title_bar_style(0, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_api_accepts_both_modes() {
        // Verify the function signature works for both dark and light
        // (actual DWM calls only work on Windows with a real HWND)
        let _ = set_title_bar_style(0, true);
        let _ = set_title_bar_style(0, false);
    }

    #[test]
    fn test_set_title_bar_for_window_does_not_panic() {
        // Cosmetic wrapper should never panic, even with invalid HWND
        set_title_bar_for_window(0, true);
        set_title_bar_for_window(0, false);
    }

    #[test]
    fn test_hwnd_hint_not_empty() {
        let hint = hwnd_extraction_hint();
        assert!(!hint.is_empty());
    }
}
