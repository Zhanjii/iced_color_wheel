//! async_operations.rs
//!
//! Async and parallel processing utilities.
//!
//! Provides utilities for batch processing with thread pools, progress tracking,
//! and graceful cancellation.
//!
//! # Example
//!
//! ```rust
//! use async_operations::{BatchProcessor, parallel_map};
//!
//! #[tokio::main]
//! async fn main() {
//!     // Batch processing with progress
//!     let processor = BatchProcessor::new();
//!     let items = vec![1, 2, 3, 4, 5];
//!
//!     let results = processor.process_batch(
//!         items,
//!         |x| x * 2,
//!         Some(|progress| {
//!             println!("Progress: {}%", progress.percent_complete());
//!         }),
//!     ).await;
//!
//!     // Simple parallel operations
//!     let results = parallel_map(|x| x * x, vec![1, 2, 3, 4, 5]);
//! }
//! ```

use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::task::JoinSet;
use tracing::{debug, error, info};

// =============================================================================
// CPU ALLOCATION
// =============================================================================

/// Strategy for determining worker count.
#[derive(Debug, Clone, Copy)]
pub enum CPUAllocationStrategy {
    Minimal,    // 1-2 workers
    Balanced,   // Half of CPU cores
    Aggressive, // All cores minus 1
    Maximum,    // All cores
}

/// Gets number of CPU cores based on allocation strategy.
pub fn get_cpu_core_count(strategy: CPUAllocationStrategy) -> usize {
    let cpu_count = num_cpus::get().max(1);

    match strategy {
        CPUAllocationStrategy::Minimal => cpu_count.min(2),
        CPUAllocationStrategy::Balanced => (cpu_count / 2).max(1),
        CPUAllocationStrategy::Aggressive => (cpu_count.saturating_sub(1)).max(1),
        CPUAllocationStrategy::Maximum => cpu_count,
    }
}

// =============================================================================
// BATCH PROCESSING
// =============================================================================

/// Status of a batch operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchStatus {
    Pending,
    Running,
    Completed,
    Cancelled,
    Failed,
}

/// Result of a single item in a batch operation.
#[derive(Debug, Clone)]
pub struct BatchResult<T> {
    pub item_index: usize,
    pub result: Option<T>,
    pub error: Option<String>,
    pub success: bool,
    pub duration_ms: f64,
}

/// Progress information for a batch operation.
#[derive(Debug, Clone)]
pub struct BatchProgress {
    pub total: usize,
    pub completed: usize,
    pub failed: usize,
    pub status: BatchStatus,
    pub start_time: Instant,
}

impl BatchProgress {
    /// Calculates completion percentage.
    pub fn percent_complete(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        (self.completed as f64 / self.total as f64) * 100.0
    }

    /// Calculates elapsed time in seconds.
    pub fn elapsed_seconds(&self) -> f64 {
        self.start_time.elapsed().as_secs_f64()
    }

    /// Estimates remaining time based on current progress.
    pub fn estimated_remaining_seconds(&self) -> Option<f64> {
        if self.completed == 0 {
            return None;
        }
        let elapsed = self.elapsed_seconds();
        if elapsed == 0.0 {
            return None;
        }
        let rate = elapsed / self.completed as f64;
        let remaining = self.total - self.completed;
        Some(rate * remaining as f64)
    }
}

// =============================================================================
// BATCH PROCESSOR
// =============================================================================

/// Processes batches of items with parallel execution and progress tracking.
#[derive(Debug)]
pub struct BatchProcessor {
    max_workers: usize,
    cancel_requested: Arc<Mutex<bool>>,
}

impl BatchProcessor {
    /// Creates a new batch processor with auto-detected worker count.
    pub fn new() -> Self {
        Self::with_strategy(CPUAllocationStrategy::Balanced)
    }

    /// Creates a batch processor with specific CPU allocation strategy.
    pub fn with_strategy(strategy: CPUAllocationStrategy) -> Self {
        let max_workers = get_cpu_core_count(strategy);
        Self {
            max_workers,
            cancel_requested: Arc::new(Mutex::new(false)),
        }
    }

    /// Creates a batch processor with explicit worker count.
    pub fn with_workers(max_workers: usize) -> Self {
        Self {
            max_workers,
            cancel_requested: Arc::new(Mutex::new(false)),
        }
    }

    /// Processes a batch of items in parallel.
    pub async fn process_batch<T, R, F, P>(
        &self,
        items: Vec<T>,
        processor: F,
        mut progress_callback: Option<P>,
    ) -> Vec<BatchResult<R>>
    where
        T: Send + 'static,
        R: Send + 'static,
        F: Fn(T) -> R + Send + Sync + 'static,
        P: FnMut(BatchProgress),
    {
        *self.cancel_requested.lock().unwrap() = false;

        let progress = Arc::new(Mutex::new(BatchProgress {
            total: items.len(),
            completed: 0,
            failed: 0,
            status: BatchStatus::Running,
            start_time: Instant::now(),
        }));

        if let Some(ref mut callback) = progress_callback {
            callback(progress.lock().unwrap().clone());
        }

        let processor = Arc::new(processor);
        let mut join_set = JoinSet::new();
        let mut results = Vec::with_capacity(items.len());

        // Spawn tasks
        for (index, item) in items.into_iter().enumerate() {
            let processor = Arc::clone(&processor);
            let progress = Arc::clone(&progress);
            let cancel_requested = Arc::clone(&self.cancel_requested);

            join_set.spawn(async move {
                if *cancel_requested.lock().unwrap() {
                    return BatchResult {
                        item_index: index,
                        result: None,
                        error: Some("Cancelled".to_string()),
                        success: false,
                        duration_ms: 0.0,
                    };
                }

                let start = Instant::now();
                let result = tokio::task::spawn_blocking(move || processor(item)).await;
                let duration_ms = start.elapsed().as_secs_f64() * 1000.0;

                let batch_result = match result {
                    Ok(r) => BatchResult {
                        item_index: index,
                        result: Some(r),
                        error: None,
                        success: true,
                        duration_ms,
                    },
                    Err(e) => {
                        error!("Task failed: {}", e);
                        let mut prog = progress.lock().unwrap();
                        prog.failed += 1;

                        BatchResult {
                            item_index: index,
                            result: None,
                            error: Some(e.to_string()),
                            success: false,
                            duration_ms,
                        }
                    }
                };

                // Update progress
                {
                    let mut prog = progress.lock().unwrap();
                    prog.completed += 1;
                }

                batch_result
            });

            // Limit concurrent tasks
            if join_set.len() >= self.max_workers {
                if let Some(result) = join_set.join_next().await {
                    if let Ok(batch_result) = result {
                        results.push(batch_result);
                        if let Some(ref mut callback) = progress_callback {
                            callback(progress.lock().unwrap().clone());
                        }
                    }
                }
            }
        }

        // Collect remaining results
        while let Some(result) = join_set.join_next().await {
            if let Ok(batch_result) = result {
                results.push(batch_result);
                if let Some(ref mut callback) = progress_callback {
                    callback(progress.lock().unwrap().clone());
                }
            }
        }

        // Finalize progress
        {
            let mut prog = progress.lock().unwrap();
            prog.status = if *self.cancel_requested.lock().unwrap() {
                BatchStatus::Cancelled
            } else {
                BatchStatus::Completed
            };
        }

        if let Some(ref mut callback) = progress_callback {
            callback(progress.lock().unwrap().clone());
        }

        // Sort results by original index
        results.sort_by_key(|r| r.item_index);

        info!(
            "Batch complete: {} items, {} failures",
            results.len(),
            progress.lock().unwrap().failed
        );

        results
    }

    /// Requests cancellation of the current batch operation.
    pub fn cancel(&self) {
        *self.cancel_requested.lock().unwrap() = true;
        debug!("Cancel requested");
    }
}

impl Default for BatchProcessor {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// SIMPLE HELPERS
// =============================================================================

/// Applies function to items in parallel using rayon.
pub fn parallel_map<T, R, F>(func: F, items: Vec<T>) -> Vec<R>
where
    T: Send,
    R: Send,
    F: Fn(T) -> R + Send + Sync,
{
    use rayon::prelude::*;
    items.into_par_iter().map(func).collect()
}

/// Applies function to items with argument unpacking in parallel.
pub fn parallel_starmap<R, F>(func: F, items: Vec<Vec<String>>) -> Vec<R>
where
    R: Send,
    F: Fn(Vec<String>) -> R + Send + Sync,
{
    use rayon::prelude::*;
    items.into_par_iter().map(func).collect()
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_allocation() {
        let minimal = get_cpu_core_count(CPUAllocationStrategy::Minimal);
        let balanced = get_cpu_core_count(CPUAllocationStrategy::Balanced);
        let maximum = get_cpu_core_count(CPUAllocationStrategy::Maximum);

        assert!(minimal <= 2);
        assert!(balanced > 0);
        assert!(maximum > 0);
        assert!(minimal <= balanced);
        assert!(balanced <= maximum);
    }

    #[tokio::test]
    async fn test_batch_processor() {
        let processor = BatchProcessor::new();
        let items = vec![1, 2, 3, 4, 5];

        let results = processor
            .process_batch(items, |x| x * 2, None::<fn(BatchProgress)>)
            .await;

        assert_eq!(results.len(), 5);
        assert!(results.iter().all(|r| r.success));
    }

    #[test]
    fn test_parallel_map() {
        let items = vec![1, 2, 3, 4, 5];
        let results = parallel_map(|x| x * x, items);

        assert_eq!(results, vec![1, 4, 9, 16, 25]);
    }
}
