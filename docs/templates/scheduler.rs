//! scheduler.rs
//!
//! Background task scheduler for desktop applications.
//!
//! Schedule one-time or recurring tasks to run in the background.
//!
//! # Example
//!
//! ```rust
//! use scheduler::TaskScheduler;
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() {
//!     let scheduler = TaskScheduler::new();
//!     scheduler.start();
//!
//!     // Schedule recurring task (every hour)
//!     scheduler.schedule_recurring(
//!         "config_sync",
//!         || println!("Syncing config..."),
//!         Duration::from_secs(3600),
//!         false,
//!     );
//!
//!     // Schedule one-time task (in 5 minutes)
//!     scheduler.schedule_once(
//!         "reminder",
//!         || println!("Reminder!"),
//!         Duration::from_secs(300),
//!     );
//!
//!     // Later: cancel a task
//!     scheduler.cancel("config_sync");
//!
//!     // Stop scheduler on app exit
//!     scheduler.stop();
//! }
//! ```

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{debug, info, warn};

// =============================================================================
// TASK INFO
// =============================================================================

#[derive(Debug, Clone)]
struct TaskInfo {
    name: String,
    recurring: bool,
    interval: Option<Duration>,
    next_run: Instant,
}

// =============================================================================
// TASK SCHEDULER
// =============================================================================

/// Background task scheduler using Tokio.
#[derive(Debug, Clone)]
pub struct TaskScheduler {
    tasks: Arc<Mutex<HashMap<String, TaskInfo>>>,
    task_functions: Arc<Mutex<HashMap<String, Arc<dyn Fn() + Send + Sync>>>>,
    running: Arc<Mutex<bool>>,
}

impl TaskScheduler {
    /// Creates a new task scheduler.
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(HashMap::new())),
            task_functions: Arc::new(Mutex::new(HashMap::new())),
            running: Arc::new(Mutex::new(false)),
        }
    }

    /// Starts the scheduler thread.
    pub fn start(&self) {
        if *self.running.lock().unwrap() {
            debug!("Scheduler already running");
            return;
        }

        *self.running.lock().unwrap() = true;

        let tasks = Arc::clone(&self.tasks);
        let task_functions = Arc::clone(&self.task_functions);
        let running = Arc::clone(&self.running);

        tokio::spawn(async move {
            info!("Task scheduler started");

            while *running.lock().unwrap() {
                let now = Instant::now();
                let mut tasks_to_run = Vec::new();

                // Find tasks that are ready to run
                {
                    let tasks_lock = tasks.lock().unwrap();
                    for (name, task_info) in tasks_lock.iter() {
                        if now >= task_info.next_run {
                            tasks_to_run.push(name.clone());
                        }
                    }
                }

                // Execute ready tasks
                for task_name in tasks_to_run {
                    let func = {
                        let funcs = task_functions.lock().unwrap();
                        funcs.get(&task_name).cloned()
                    };

                    if let Some(func) = func {
                        debug!("Executing task: {}", task_name);

                        // Run in blocking context
                        let task_name_clone = task_name.clone();
                        tokio::task::spawn_blocking(move || {
                            func();
                        });

                        // Update task info
                        let mut tasks_lock = tasks.lock().unwrap();
                        if let Some(task_info) = tasks_lock.get_mut(&task_name) {
                            if task_info.recurring {
                                if let Some(interval) = task_info.interval {
                                    task_info.next_run = Instant::now() + interval;
                                } else {
                                    tasks_lock.remove(&task_name);
                                    task_functions.lock().unwrap().remove(&task_name);
                                }
                            } else {
                                // One-time task, remove it
                                tasks_lock.remove(&task_name);
                                task_functions.lock().unwrap().remove(&task_name);
                            }
                        }
                    }
                }

                // Sleep briefly before next check
                sleep(Duration::from_millis(100)).await;
            }

            info!("Task scheduler stopped");
        });
    }

    /// Stops the scheduler and cancels all pending tasks.
    pub fn stop(&self) {
        *self.running.lock().unwrap() = false;

        let mut tasks = self.tasks.lock().unwrap();
        let mut funcs = self.task_functions.lock().unwrap();

        tasks.clear();
        funcs.clear();

        info!("Task scheduler stopped");
    }

    /// Schedules a one-time task.
    pub fn schedule_once<F>(&self, name: impl Into<String>, func: F, delay: Duration) -> bool
    where
        F: Fn() + Send + Sync + 'static,
    {
        if !*self.running.lock().unwrap() {
            warn!("Scheduler not running, task not scheduled");
            return false;
        }

        let name = name.into();
        let next_run = Instant::now() + delay;

        // Store task info
        {
            let mut tasks = self.tasks.lock().unwrap();
            tasks.insert(
                name.clone(),
                TaskInfo {
                    name: name.clone(),
                    recurring: false,
                    interval: None,
                    next_run,
                },
            );
        }

        // Store function
        {
            let mut funcs = self.task_functions.lock().unwrap();
            funcs.insert(name.clone(), Arc::new(func));
        }

        debug!("Scheduled one-time task: {} in {:?}", name, delay);
        true
    }

    /// Schedules a recurring task.
    pub fn schedule_recurring<F>(
        &self,
        name: impl Into<String>,
        func: F,
        interval: Duration,
        run_immediately: bool,
    ) -> bool
    where
        F: Fn() + Send + Sync + 'static,
    {
        if !*self.running.lock().unwrap() {
            warn!("Scheduler not running, task not scheduled");
            return false;
        }

        let name = name.into();
        let first_delay = if run_immediately {
            Duration::from_secs(0)
        } else {
            interval
        };
        let next_run = Instant::now() + first_delay;

        // Run immediately if requested
        if run_immediately {
            func();
        }

        // Store task info
        {
            let mut tasks = self.tasks.lock().unwrap();
            tasks.insert(
                name.clone(),
                TaskInfo {
                    name: name.clone(),
                    recurring: true,
                    interval: Some(interval),
                    next_run,
                },
            );
        }

        // Store function
        {
            let mut funcs = self.task_functions.lock().unwrap();
            funcs.insert(name.clone(), Arc::new(func));
        }

        debug!("Scheduled recurring task: {} every {:?}", name, interval);
        true
    }

    /// Cancels a scheduled task.
    pub fn cancel(&self, name: &str) -> bool {
        let mut tasks = self.tasks.lock().unwrap();
        let mut funcs = self.task_functions.lock().unwrap();

        let removed = tasks.remove(name).is_some();
        funcs.remove(name);

        if removed {
            debug!("Cancelled task: {}", name);
        }

        removed
    }

    /// Checks if a task is scheduled.
    pub fn is_scheduled(&self, name: &str) -> bool {
        self.tasks.lock().unwrap().contains_key(name)
    }

    /// Gets the next scheduled run time for a task.
    pub fn get_next_run(&self, name: &str) -> Option<Instant> {
        self.tasks
            .lock()
            .unwrap()
            .get(name)
            .map(|info| info.next_run)
    }

    /// Lists all scheduled tasks.
    pub fn list_tasks(&self) -> Vec<TaskSummary> {
        self.tasks
            .lock()
            .unwrap()
            .values()
            .map(|info| TaskSummary {
                name: info.name.clone(),
                recurring: info.recurring,
                interval: info.interval,
                next_run: info.next_run,
            })
            .collect()
    }

    /// Checks if scheduler is running.
    pub fn is_running(&self) -> bool {
        *self.running.lock().unwrap()
    }
}

impl Default for TaskScheduler {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// TASK SUMMARY
// =============================================================================

/// Summary information about a scheduled task.
#[derive(Debug, Clone)]
pub struct TaskSummary {
    pub name: String,
    pub recurring: bool,
    pub interval: Option<Duration>,
    pub next_run: Instant,
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[tokio::test]
    async fn test_scheduler_one_time() {
        let scheduler = TaskScheduler::new();
        scheduler.start();

        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = Arc::clone(&counter);

        scheduler.schedule_once(
            "test_task",
            move || {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            },
            Duration::from_millis(100),
        );

        sleep(Duration::from_millis(200)).await;

        assert_eq!(counter.load(Ordering::SeqCst), 1);
        scheduler.stop();
    }

    #[tokio::test]
    async fn test_scheduler_recurring() {
        let scheduler = TaskScheduler::new();
        scheduler.start();

        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = Arc::clone(&counter);

        scheduler.schedule_recurring(
            "recurring_task",
            move || {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            },
            Duration::from_millis(100),
            false,
        );

        sleep(Duration::from_millis(350)).await;

        let count = counter.load(Ordering::SeqCst);
        assert!(count >= 3, "Expected at least 3 executions, got {}", count);

        scheduler.stop();
    }

    #[tokio::test]
    async fn test_cancel_task() {
        let scheduler = TaskScheduler::new();
        scheduler.start();

        scheduler.schedule_once("test", || {}, Duration::from_secs(10));
        assert!(scheduler.is_scheduled("test"));

        scheduler.cancel("test");
        assert!(!scheduler.is_scheduled("test"));

        scheduler.stop();
    }

    #[test]
    fn test_list_tasks() {
        let scheduler = TaskScheduler::new();
        scheduler.start();

        scheduler.schedule_once("task1", || {}, Duration::from_secs(1));
        scheduler.schedule_recurring("task2", || {}, Duration::from_secs(2), false);

        let tasks = scheduler.list_tasks();
        assert_eq!(tasks.len(), 2);

        scheduler.stop();
    }
}
