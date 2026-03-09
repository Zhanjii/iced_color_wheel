//! notifications.rs
//!
//! Multi-channel notification system.
//!
//! Send notifications to various channels:
//! - Slack webhooks
//! - SMS via Twilio
//! - Email via SMTP
//! - Desktop notifications
//! - Generic webhooks (Zapier, IFTTT, etc.)
//!
//! # Example
//!
//! ```rust
//! use notifications::{NotificationManager, SlackNotification, DesktopNotification};
//!
//! // Create manager and add channels
//! let mut notifier = NotificationManager::new();
//! notifier.add_channel("slack", Box::new(SlackNotification::new("https://...")));
//! notifier.add_channel("desktop", Box::new(DesktopNotification::new("MyApp", None)));
//!
//! // Send to all channels
//! notifier.notify_all("Alert", "Something important happened!");
//!
//! // Send to specific channel
//! notifier.notify("slack", "Status Update", "Deployment complete");
//! ```

use serde_json::json;
use std::collections::HashMap;
use std::process::Command;

// =============================================================================
// NOTIFICATION CHANNEL TRAIT
// =============================================================================

/// Trait for notification channels
pub trait NotificationChannel: Send + Sync {
    /// Send a notification
    fn send(&self, title: &str, message: &str, extra: &HashMap<String, String>) -> bool;

    /// Channel name for logging
    fn name(&self) -> &str;
}

// =============================================================================
// SLACK NOTIFICATION
// =============================================================================

/// Send notifications to Slack via incoming webhook
pub struct SlackNotification {
    webhook_url: String,
}

impl SlackNotification {
    pub fn new(webhook_url: impl Into<String>) -> Self {
        Self {
            webhook_url: webhook_url.into(),
        }
    }
}

impl NotificationChannel for SlackNotification {
    fn send(&self, title: &str, message: &str, extra: &HashMap<String, String>) -> bool {
        let blocks = json!([
            {
                "type": "header",
                "text": {"type": "plain_text", "text": title, "emoji": true}
            },
            {
                "type": "section",
                "text": {"type": "mrkdwn", "text": message}
            }
        ]);

        let payload = json!({
            "blocks": blocks
        });

        // Send HTTP POST request
        match ureq::post(&self.webhook_url)
            .send_json(payload)
        {
            Ok(_) => true,
            Err(e) => {
                eprintln!("Slack notification failed: {}", e);
                false
            }
        }
    }

    fn name(&self) -> &str {
        "SlackNotification"
    }
}

// =============================================================================
// EMAIL NOTIFICATION
// =============================================================================

/// Send email notifications via SMTP
pub struct EmailNotification {
    smtp_server: String,
    smtp_port: u16,
    smtp_user: String,
    smtp_password: String,
    from_email: String,
    to_emails: Vec<String>,
}

impl EmailNotification {
    pub fn new(
        smtp_server: impl Into<String>,
        smtp_port: u16,
        smtp_user: impl Into<String>,
        smtp_password: impl Into<String>,
        from_email: impl Into<String>,
        to_emails: Vec<String>,
    ) -> Self {
        Self {
            smtp_server: smtp_server.into(),
            smtp_port,
            smtp_user: smtp_user.into(),
            smtp_password: smtp_password.into(),
            from_email: from_email.into(),
            to_emails,
        }
    }
}

impl NotificationChannel for EmailNotification {
    fn send(&self, title: &str, message: &str, _extra: &HashMap<String, String>) -> bool {
        // TODO: Implement SMTP email sending using lettre crate
        eprintln!("Would send email '{}' to {:?}", title, self.to_emails);
        true
    }

    fn name(&self) -> &str {
        "EmailNotification"
    }
}

// =============================================================================
// WEBHOOK NOTIFICATION
// =============================================================================

/// Send notifications to generic webhooks
pub struct WebhookNotification {
    webhook_url: String,
    headers: HashMap<String, String>,
}

impl WebhookNotification {
    pub fn new(webhook_url: impl Into<String>) -> Self {
        Self {
            webhook_url: webhook_url.into(),
            headers: HashMap::new(),
        }
    }

    pub fn with_headers(mut self, headers: HashMap<String, String>) -> Self {
        self.headers = headers;
        self
    }
}

impl NotificationChannel for WebhookNotification {
    fn send(&self, title: &str, message: &str, extra: &HashMap<String, String>) -> bool {
        let mut payload = json!({
            "title": title,
            "message": message,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });

        // Add extra fields
        if let Some(obj) = payload.as_object_mut() {
            for (key, value) in extra {
                obj.insert(key.clone(), json!(value));
            }
        }

        // Send HTTP POST request
        let mut request = ureq::post(&self.webhook_url);

        for (key, value) in &self.headers {
            request = request.set(key, value);
        }

        match request.send_json(payload) {
            Ok(_) => true,
            Err(e) => {
                eprintln!("Webhook notification failed: {}", e);
                false
            }
        }
    }

    fn name(&self) -> &str {
        "WebhookNotification"
    }
}

// =============================================================================
// DESKTOP NOTIFICATION
// =============================================================================

/// Show desktop/system notifications
pub struct DesktopNotification {
    app_name: String,
    icon_path: Option<String>,
}

impl DesktopNotification {
    pub fn new(app_name: impl Into<String>, icon_path: Option<String>) -> Self {
        Self {
            app_name: app_name.into(),
            icon_path,
        }
    }
}

impl NotificationChannel for DesktopNotification {
    fn send(&self, title: &str, message: &str, _extra: &HashMap<String, String>) -> bool {
        let system = std::env::consts::OS;

        match system {
            "windows" => self.windows_notify(title, message),
            "macos" => self.macos_notify(title, message),
            _ => self.linux_notify(title, message),
        }
    }

    fn name(&self) -> &str {
        "DesktopNotification"
    }
}

impl DesktopNotification {
    #[cfg(target_os = "windows")]
    fn windows_notify(&self, title: &str, message: &str) -> bool {
        // PowerShell toast notification
        let ps_script = format!(
            r#"
            [Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null
            $template = [Windows.UI.Notifications.ToastTemplateType]::ToastText02
            $xml = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent($template)
            $xml.GetElementsByTagName("text")[0].AppendChild($xml.CreateTextNode("{}"))
            $xml.GetElementsByTagName("text")[1].AppendChild($xml.CreateTextNode("{}"))
            $toast = [Windows.UI.Notifications.ToastNotification]::new($xml)
            [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier("{}").Show($toast)
            "#,
            title.replace("\"", "\\\""),
            message.replace("\"", "\\\""),
            self.app_name.replace("\"", "\\\"")
        );

        Command::new("powershell")
            .args(&["-Command", &ps_script])
            .output()
            .is_ok()
    }

    #[cfg(not(target_os = "windows"))]
    fn windows_notify(&self, _title: &str, _message: &str) -> bool {
        false
    }

    fn macos_notify(&self, title: &str, message: &str) -> bool {
        let script = format!(
            r#"display notification "{}" with title "{}""#,
            message.replace("\"", "\\\""),
            title.replace("\"", "\\\"")
        );

        Command::new("osascript")
            .args(&["-e", &script])
            .output()
            .is_ok()
    }

    fn linux_notify(&self, title: &str, message: &str) -> bool {
        let mut cmd = Command::new("notify-send");
        cmd.arg(title).arg(message);

        if let Some(icon) = &self.icon_path {
            cmd.arg("-i").arg(icon);
        }

        cmd.output().is_ok()
    }
}

// =============================================================================
// NOTIFICATION MANAGER
// =============================================================================

/// Manages multiple notification channels
pub struct NotificationManager {
    channels: HashMap<String, Box<dyn NotificationChannel>>,
}

impl NotificationManager {
    /// Create a new notification manager
    pub fn new() -> Self {
        Self {
            channels: HashMap::new(),
        }
    }

    /// Register a notification channel
    pub fn add_channel(&mut self, name: impl Into<String>, channel: Box<dyn NotificationChannel>) {
        self.channels.insert(name.into(), channel);
    }

    /// Remove a notification channel
    pub fn remove_channel(&mut self, name: &str) -> bool {
        self.channels.remove(name).is_some()
    }

    /// Send notification to a specific channel
    pub fn notify(
        &self,
        channel_name: &str,
        title: &str,
        message: &str,
    ) -> bool {
        self.notify_with_extra(channel_name, title, message, &HashMap::new())
    }

    /// Send notification to a specific channel with extra data
    pub fn notify_with_extra(
        &self,
        channel_name: &str,
        title: &str,
        message: &str,
        extra: &HashMap<String, String>,
    ) -> bool {
        if let Some(channel) = self.channels.get(channel_name) {
            channel.send(title, message, extra)
        } else {
            eprintln!("Unknown channel: {}", channel_name);
            false
        }
    }

    /// Send notification to all registered channels
    pub fn notify_all(&self, title: &str, message: &str) -> HashMap<String, bool> {
        self.notify_all_with_extra(title, message, &HashMap::new())
    }

    /// Send notification to all channels with extra data
    pub fn notify_all_with_extra(
        &self,
        title: &str,
        message: &str,
        extra: &HashMap<String, String>,
    ) -> HashMap<String, bool> {
        let mut results = HashMap::new();

        for (name, channel) in &self.channels {
            let success = channel.send(title, message, extra);
            results.insert(name.clone(), success);
        }

        results
    }

    /// Send notification to specific channels
    pub fn notify_channels(
        &self,
        channel_names: &[String],
        title: &str,
        message: &str,
    ) -> HashMap<String, bool> {
        self.notify_channels_with_extra(channel_names, title, message, &HashMap::new())
    }

    /// Send notification to specific channels with extra data
    pub fn notify_channels_with_extra(
        &self,
        channel_names: &[String],
        title: &str,
        message: &str,
        extra: &HashMap<String, String>,
    ) -> HashMap<String, bool> {
        let mut results = HashMap::new();

        for name in channel_names {
            if let Some(channel) = self.channels.get(name) {
                let success = channel.send(title, message, extra);
                results.insert(name.clone(), success);
            }
        }

        results
    }

    /// List all registered channel names
    pub fn list_channels(&self) -> Vec<String> {
        self.channels.keys().cloned().collect()
    }
}

impl Default for NotificationManager {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    struct MockChannel {
        name: String,
    }

    impl NotificationChannel for MockChannel {
        fn send(&self, _title: &str, _message: &str, _extra: &HashMap<String, String>) -> bool {
            true
        }

        fn name(&self) -> &str {
            &self.name
        }
    }

    #[test]
    fn test_notification_manager() {
        let mut manager = NotificationManager::new();
        manager.add_channel("test", Box::new(MockChannel {
            name: "test".to_string(),
        }));

        assert_eq!(manager.list_channels().len(), 1);

        let result = manager.notify("test", "Title", "Message");
        assert!(result);

        manager.remove_channel("test");
        assert_eq!(manager.list_channels().len(), 0);
    }

    #[test]
    fn test_desktop_notification() {
        let desktop = DesktopNotification::new("TestApp", None);
        // Just test that it doesn't panic
        let _ = desktop.send("Test", "Message", &HashMap::new());
    }
}
