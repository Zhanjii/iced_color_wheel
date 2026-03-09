//! progress_bar.rs
//!
//! Progress tracking widget and background-task patterns for iced daemon applications.
//!
//! Features:
//! - `ProgressTracker` struct with current/total/status
//! - `view()` using iced's built-in `progress_bar` widget
//! - `Task::perform()` pattern for background work
//! - `iced::Subscription` for streaming progress updates
//! - Cancellable operations
//! - Multi-step progress tracking
//!
//! # Example
//!
//! ```rust
//! use progress_bar::{ProgressTracker, ProgressState, view_progress, ProgressMessage};
//!
//! // In your App state:
//! tracker: ProgressTracker,
//!
//! // In update():
//! ProgressMessage::Start { total } => {
//!     self.tracker.start(total);
//!     // Launch background work
//!     return Task::perform(
//!         do_heavy_work(total),
//!         ProgressMessage::BackgroundDone,
//!     );
//! }
//! ProgressMessage::Tick { current, message } => {
//!     self.tracker.update(current, message);
//! }
//!
//! // In view():
//! view_progress(&self.tracker)
//! ```

use iced::widget::{button, column, container, progress_bar, row, text};
use iced::{Alignment, Element, Length, Padding, Subscription, Task};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

// =============================================================================
// PROGRESS STRUCTURES
// =============================================================================

/// Progress update data (for serialization / logging)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressUpdate {
    /// Current progress value
    pub current: usize,
    /// Total progress value
    pub total: usize,
    /// Progress percentage (0-100)
    pub percentage: f64,
    /// Status message
    pub message: Option<String>,
    /// Operation ID
    pub operation_id: String,
}

impl ProgressUpdate {
    /// Create a new progress update
    pub fn new(
        operation_id: impl Into<String>,
        current: usize,
        total: usize,
        message: Option<String>,
    ) -> Self {
        let percentage = if total > 0 {
            (current as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        Self {
            current,
            total,
            percentage,
            message,
            operation_id: operation_id.into(),
        }
    }
}

/// Progress state
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProgressState {
    Idle,
    Running,
    Paused,
    Cancelled,
    Completed,
    Error,
}

// =============================================================================
// PROGRESS TRACKER
// =============================================================================

/// In-memory progress tracker.
///
/// Lives in your `App` struct. Mutate it from `update()`, read it from `view()`.
#[derive(Debug, Clone)]
pub struct ProgressTracker {
    /// Unique operation identifier
    pub operation_id: String,
    /// Current progress value
    pub current: usize,
    /// Total items to process
    pub total: usize,
    /// Current state
    pub state: ProgressState,
    /// Status message
    pub message: Option<String>,
    /// Error message (if in Error state)
    pub error: Option<String>,
    /// Shared cancellation flag for background tasks
    pub cancelled: Arc<Mutex<bool>>,
}

impl ProgressTracker {
    /// Create a new idle progress tracker
    pub fn new(operation_id: impl Into<String>) -> Self {
        Self {
            operation_id: operation_id.into(),
            current: 0,
            total: 0,
            state: ProgressState::Idle,
            message: None,
            error: None,
            cancelled: Arc::new(Mutex::new(false)),
        }
    }

    /// Start progress tracking
    pub fn start(&mut self, total: usize) {
        self.current = 0;
        self.total = total;
        self.state = ProgressState::Running;
        self.message = None;
        self.error = None;
        *self.cancelled.lock().unwrap() = false;
    }

    /// Update progress
    pub fn update(&mut self, current: usize, message: Option<String>) {
        self.current = current;
        self.message = message;
    }

    /// Increment progress by one
    pub fn increment(&mut self, message: Option<String>) {
        self.current += 1;
        self.message = message;
    }

    /// Mark as completed
    pub fn complete(&mut self) {
        self.current = self.total;
        self.state = ProgressState::Completed;
        self.message = Some("Complete".to_string());
    }

    /// Mark as cancelled
    pub fn cancel(&mut self) {
        self.state = ProgressState::Cancelled;
        *self.cancelled.lock().unwrap() = true;
    }

    /// Mark as error
    pub fn set_error(&mut self, message: impl Into<String>) {
        self.state = ProgressState::Error;
        self.error = Some(message.into());
    }

    /// Check whether cancellation was requested (safe to call from any thread)
    pub fn is_cancelled(&self) -> bool {
        *self.cancelled.lock().unwrap()
    }

    /// Get the cancellation flag clone (pass to background tasks)
    pub fn cancellation_token(&self) -> Arc<Mutex<bool>> {
        self.cancelled.clone()
    }

    /// Progress as a fraction in 0.0..=1.0
    pub fn fraction(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            (self.current as f32 / self.total as f32).clamp(0.0, 1.0)
        }
    }

    /// Percentage 0..=100
    pub fn percentage(&self) -> f64 {
        if self.total > 0 {
            (self.current as f64 / self.total as f64) * 100.0
        } else {
            0.0
        }
    }
}

// =============================================================================
// MESSAGES
// =============================================================================

/// Messages emitted / consumed by the progress widget
#[derive(Debug, Clone)]
pub enum ProgressMessage {
    /// Begin a new operation with `total` items
    Start { total: usize },
    /// Progress tick from a background task
    Tick { current: usize, message: Option<String> },
    /// Background task finished successfully
    BackgroundDone(()),
    /// Background task failed
    BackgroundError(String),
    /// User clicked Cancel
    Cancel,
}

// =============================================================================
// VIEW
// =============================================================================

/// Render the progress tracker as an iced widget tree.
///
/// Shows a progress bar, percentage, status message, and a Cancel button
/// (when running).
///
/// # Returns
///
/// `Element<ProgressMessage>` -- map this into your top-level `Message`.
pub fn view_progress<'a>(tracker: &ProgressTracker) -> Element<'a, ProgressMessage> {
    let pct = tracker.percentage();
    let pct_text = format!("{:.1}%", pct);
    let status_text = match &tracker.state {
        ProgressState::Idle => "Idle".to_string(),
        ProgressState::Running => {
            tracker
                .message
                .clone()
                .unwrap_or_else(|| format!("{} / {}", tracker.current, tracker.total))
        }
        ProgressState::Paused => "Paused".to_string(),
        ProgressState::Cancelled => "Cancelled".to_string(),
        ProgressState::Completed => "Complete".to_string(),
        ProgressState::Error => {
            tracker
                .error
                .clone()
                .unwrap_or_else(|| "Error".to_string())
        }
    };

    let bar = progress_bar(0.0..=100.0, pct as f32)
        .width(Length::Fill)
        .height(Length::Fixed(20.0));

    let info_row = row![
        text(&status_text).size(13),
        text(&pct_text).size(13),
    ]
    .spacing(12)
    .align_y(Alignment::Center);

    let mut col = column![bar, info_row].spacing(6).padding(8).width(Length::Fill);

    // Show Cancel button only while running
    if tracker.state == ProgressState::Running {
        col = col.push(
            button(text("Cancel").size(13))
                .on_press(ProgressMessage::Cancel)
                .padding(Padding::from([4, 12])),
        );
    }

    container(col).width(Length::Fill).into()
}

// =============================================================================
// TASK::PERFORM PATTERN
// =============================================================================

/// Example: launch a background computation and receive a single result.
///
/// ```rust
/// // In update():
/// ProgressMessage::Start { total } => {
///     self.tracker.start(total);
///     let token = self.tracker.cancellation_token();
///     return Task::perform(
///         async move { run_heavy_work(total, token).await },
///         |result| match result {
///             Ok(()) => ProgressMessage::BackgroundDone(()),
///             Err(e) => ProgressMessage::BackgroundError(e),
///         },
///     );
/// }
/// ```
///
/// For *streaming* progress (tick-by-tick), use the subscription pattern below.
pub async fn example_background_work(
    total: usize,
    cancel: Arc<Mutex<bool>>,
) -> Result<(), String> {
    for i in 0..total {
        if *cancel.lock().unwrap() {
            return Err("Cancelled".to_string());
        }
        // Simulate work
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let _ = i; // real work here
    }
    Ok(())
}

// =============================================================================
// SUBSCRIPTION PATTERN FOR STREAMING PROGRESS
// =============================================================================

/// Create a subscription that streams progress ticks from a background channel.
///
/// The caller should create a `tokio::sync::mpsc` channel, pass the sender to
/// the background task, and pass the receiver here.
///
/// # Example
///
/// ```rust
/// fn subscription(&self) -> Subscription<Message> {
///     if self.tracker.state == ProgressState::Running {
///         progress_subscription(self.progress_rx.clone())
///             .map(Message::Progress)
///     } else {
///         Subscription::none()
///     }
/// }
/// ```
pub fn progress_subscription(
    receiver: Arc<Mutex<Option<tokio::sync::mpsc::Receiver<(usize, Option<String>)>>>>,
) -> Subscription<ProgressMessage> {
    iced::Subscription::run_with_id(
        "progress-stream",
        iced::stream::channel(64, move |mut sender| async move {
            let mut rx = receiver
                .lock()
                .unwrap()
                .take()
                .expect("progress receiver already consumed");

            while let Some((current, message)) = rx.recv().await {
                let _ = sender
                    .send(ProgressMessage::Tick { current, message })
                    .await;
            }

            let _ = sender.send(ProgressMessage::BackgroundDone(())).await;

            // Keep the future alive so the subscription stays open until dropped
            std::future::pending::<()>().await;
            unreachable!()
        }),
    )
}

// =============================================================================
// MULTI-STEP PROGRESS TRACKER
// =============================================================================

/// Multi-step progress tracker
///
/// Useful for operations with multiple distinct phases
pub struct MultiStepProgress {
    tracker: ProgressTracker,
    steps: Vec<ProgressStep>,
    current_step: usize,
}

#[derive(Debug, Clone)]
struct ProgressStep {
    name: String,
    weight: f64,
}

impl MultiStepProgress {
    /// Create a new multi-step progress tracker
    pub fn new(operation_id: impl Into<String>, steps: Vec<(String, f64)>) -> Self {
        let progress_steps = steps
            .into_iter()
            .map(|(name, weight)| ProgressStep { name, weight })
            .collect();

        Self {
            tracker: ProgressTracker::new(operation_id),
            steps: progress_steps,
            current_step: 0,
        }
    }

    /// Start the multi-step operation
    pub fn start(&mut self) {
        let total_weight: f64 = self.steps.iter().map(|s| s.weight).sum();
        self.tracker.start(total_weight as usize);
        self.current_step = 0;
    }

    /// Begin a step
    pub fn begin_step(&mut self, step_index: usize) {
        if step_index < self.steps.len() {
            self.current_step = step_index;
            let step_name = self.steps[step_index].name.clone();
            self.tracker.update(0, Some(format!("Starting: {}", step_name)));
        }
    }

    /// Complete current step
    pub fn complete_step(&mut self) {
        if self.current_step < self.steps.len() {
            let accumulated_weight: f64 = self.steps[..=self.current_step]
                .iter()
                .map(|s| s.weight)
                .sum();
            let step_name = self.steps[self.current_step].name.clone();
            self.tracker.update(
                accumulated_weight as usize,
                Some(format!("Completed: {}", step_name)),
            );
        }
    }

    /// Complete all steps
    pub fn complete(&mut self) {
        self.tracker.complete();
    }

    /// Check if cancelled
    pub fn is_cancelled(&self) -> bool {
        self.tracker.is_cancelled()
    }

    /// Get a reference to the inner tracker for rendering
    pub fn tracker(&self) -> &ProgressTracker {
        &self.tracker
    }
}

// =============================================================================
// DEPENDENCIES
// =============================================================================

// Add to Cargo.toml:
// [dependencies]
// iced = { version = "0.14", features = ["multi-window", "tokio"] }
// serde = { version = "1.0", features = ["derive"] }
// tokio = { version = "1", features = ["sync", "time"] }

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_update_creation() {
        let update = ProgressUpdate::new("test", 50, 100, Some("Half done".to_string()));

        assert_eq!(update.current, 50);
        assert_eq!(update.total, 100);
        assert_eq!(update.percentage, 50.0);
        assert_eq!(update.operation_id, "test");
    }

    #[test]
    fn test_progress_update_percentage() {
        let update = ProgressUpdate::new("test", 75, 100, None);
        assert_eq!(update.percentage, 75.0);

        let update = ProgressUpdate::new("test", 1, 3, None);
        assert!((update.percentage - 33.333).abs() < 0.01);
    }

    #[test]
    fn test_progress_state() {
        assert_eq!(ProgressState::Idle, ProgressState::Idle);
        assert_ne!(ProgressState::Running, ProgressState::Completed);
    }

    #[test]
    fn test_tracker_lifecycle() {
        let mut tracker = ProgressTracker::new("test-op");
        assert_eq!(tracker.state, ProgressState::Idle);
        assert_eq!(tracker.fraction(), 0.0);

        tracker.start(100);
        assert_eq!(tracker.state, ProgressState::Running);
        assert_eq!(tracker.total, 100);

        tracker.update(50, Some("halfway".to_string()));
        assert_eq!(tracker.current, 50);
        assert!((tracker.percentage() - 50.0).abs() < 0.001);

        tracker.increment(None);
        assert_eq!(tracker.current, 51);

        tracker.complete();
        assert_eq!(tracker.state, ProgressState::Completed);
        assert_eq!(tracker.current, 100);
    }

    #[test]
    fn test_tracker_cancellation() {
        let mut tracker = ProgressTracker::new("cancel-test");
        tracker.start(10);
        assert!(!tracker.is_cancelled());

        let token = tracker.cancellation_token();
        tracker.cancel();
        assert!(tracker.is_cancelled());

        // Token reflects cancellation
        assert!(*token.lock().unwrap());
    }

    #[test]
    fn test_tracker_error() {
        let mut tracker = ProgressTracker::new("err-test");
        tracker.start(10);
        tracker.set_error("something broke");
        assert_eq!(tracker.state, ProgressState::Error);
        assert_eq!(tracker.error.as_deref(), Some("something broke"));
    }
}
