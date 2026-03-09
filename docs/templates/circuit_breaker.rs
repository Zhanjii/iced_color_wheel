//! circuit_breaker.rs
//!
//! Circuit breaker pattern implementation for resilient service calls.
//!
//! Features:
//! - Per-service circuit breakers with configurable thresholds
//! - Three states: Closed (normal), Open (failing), Half-Open (testing)
//! - Automatic state transitions based on failure/success counts
//! - Registry for managing multiple circuit breakers
//! - Metrics and monitoring support
//!
//! # Example
//!
//! ```rust
//! use circuit_breaker::{CircuitBreaker, CircuitBreakerRegistry, CircuitOpenError};
//!
//! // Single breaker
//! let breaker = CircuitBreaker::new("github_api");
//! match breaker.call(|| api_request()) {
//!     Ok(result) => println!("Success: {:?}", result),
//!     Err(CircuitOpenError { .. }) => println!("Circuit is open, using fallback"),
//! }
//!
//! // Registry for multiple services
//! let mut registry = CircuitBreakerRegistry::new();
//! let github = registry.get_or_create("github");
//! let stripe = registry.get_or_create("stripe");
//! ```

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use thiserror::Error;
use tracing::{debug, warn};

// =============================================================================
// EXCEPTIONS
// =============================================================================

/// Error raised when circuit breaker is open and rejecting calls.
#[derive(Debug, Error)]
#[error("Circuit '{name}' is open. Retry in {time_until_retry:.1}s")]
pub struct CircuitOpenError {
    pub name: String,
    pub time_until_retry: f64,
}

// =============================================================================
// CIRCUIT STATE
// =============================================================================

/// Circuit breaker states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation, requests pass through
    Closed,
    /// Failing, requests are rejected
    Open,
    /// Testing if service recovered
    HalfOpen,
}

impl CircuitState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Closed => "closed",
            Self::Open => "open",
            Self::HalfOpen => "half_open",
        }
    }
}

// =============================================================================
// CIRCUIT METRICS
// =============================================================================

/// Metrics for monitoring circuit breaker behavior.
#[derive(Debug, Clone)]
pub struct CircuitMetrics {
    pub name: String,
    pub state: String,
    pub failure_count: u32,
    pub success_count: u32,
    pub total_calls: u64,
    pub total_rejections: u64,
    pub last_failure_time: Option<Instant>,
    pub consecutive_successes: u32,
}

// =============================================================================
// CIRCUIT BREAKER
// =============================================================================

/// Circuit breaker for preventing cascading failures.
#[derive(Debug)]
struct CircuitBreakerInner {
    name: String,
    failure_threshold: u32,
    cooldown: Duration,
    success_threshold: u32,
    state: CircuitState,
    failure_count: u32,
    success_count: u32,
    consecutive_successes: u32,
    last_failure_time: Option<Instant>,
    total_calls: u64,
    total_rejections: u64,
}

#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    inner: Arc<Mutex<CircuitBreakerInner>>,
}

impl CircuitBreaker {
    /// Creates a new circuit breaker.
    pub fn new(name: impl Into<String>) -> Self {
        Self::with_config(name, 3, Duration::from_secs(300), 1)
    }

    /// Creates a circuit breaker with custom configuration.
    pub fn with_config(
        name: impl Into<String>,
        failure_threshold: u32,
        cooldown: Duration,
        success_threshold: u32,
    ) -> Self {
        Self {
            inner: Arc::new(Mutex::new(CircuitBreakerInner {
                name: name.into(),
                failure_threshold,
                cooldown,
                success_threshold,
                state: CircuitState::Closed,
                failure_count: 0,
                success_count: 0,
                consecutive_successes: 0,
                last_failure_time: None,
                total_calls: 0,
                total_rejections: 0,
            })),
        }
    }

    /// Returns current circuit state.
    pub fn state(&self) -> CircuitState {
        let mut inner = self.inner.lock().unwrap();

        // Check if we should transition from OPEN to HALF_OPEN
        if inner.state == CircuitState::Open && Self::should_attempt_reset(&inner) {
            Self::transition_to(&mut inner, CircuitState::HalfOpen);
        }

        inner.state
    }

    /// Checks if circuit is closed (normal operation).
    pub fn is_closed(&self) -> bool {
        self.state() == CircuitState::Closed
    }

    /// Checks if circuit is open (rejecting calls).
    pub fn is_open(&self) -> bool {
        self.state() == CircuitState::Open
    }

    /// Executes a function with circuit breaker protection.
    pub fn call<F, T, E>(&self, f: F) -> Result<T, CircuitOpenError>
    where
        F: FnOnce() -> Result<T, E>,
    {
        let mut inner = self.inner.lock().unwrap();
        inner.total_calls += 1;
        let current_state = inner.state;

        if current_state == CircuitState::Open {
            inner.total_rejections += 1;
            let time_until_retry = Self::time_until_retry(&inner);
            drop(inner); // Release lock
            return Err(CircuitOpenError {
                name: self.get_name(),
                time_until_retry,
            });
        }

        drop(inner); // Release lock before calling function

        match f() {
            Ok(result) => {
                self.record_success();
                Ok(result)
            }
            Err(_) => {
                self.record_failure();
                // Re-check if circuit opened
                if self.is_open() {
                    let inner = self.inner.lock().unwrap();
                    let time_until_retry = Self::time_until_retry(&inner);
                    Err(CircuitOpenError {
                        name: self.get_name(),
                        time_until_retry,
                    })
                } else {
                    // Circuit still closed/half-open, propagate original error
                    // In Rust, we can't propagate the original error through CircuitOpenError
                    // so the caller needs to handle this case
                    panic!("Function failed but circuit remained closed");
                }
            }
        }
    }

    /// Records a successful call.
    fn record_success(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.success_count += 1;
        inner.consecutive_successes += 1;

        if inner.state == CircuitState::HalfOpen {
            if inner.consecutive_successes >= inner.success_threshold {
                inner.failure_count = 0;
                inner.consecutive_successes = 0;
                Self::transition_to(&mut inner, CircuitState::Closed);
            }
        }
    }

    /// Records a failed call.
    fn record_failure(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.failure_count += 1;
        inner.consecutive_successes = 0;
        inner.last_failure_time = Some(Instant::now());

        debug!(
            "Circuit '{}' failure {}/{}",
            inner.name, inner.failure_count, inner.failure_threshold
        );

        if inner.state == CircuitState::HalfOpen {
            Self::transition_to(&mut inner, CircuitState::Open);
        } else if inner.state == CircuitState::Closed {
            if inner.failure_count >= inner.failure_threshold {
                Self::transition_to(&mut inner, CircuitState::Open);
            }
        }
    }

    /// Forces circuit to open state.
    pub fn force_open(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.last_failure_time = Some(Instant::now());
        Self::transition_to(&mut inner, CircuitState::Open);
    }

    /// Forces circuit to closed state.
    pub fn force_close(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.failure_count = 0;
        inner.consecutive_successes = 0;
        Self::transition_to(&mut inner, CircuitState::Closed);
    }

    /// Resets all circuit breaker state.
    pub fn reset(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.state = CircuitState::Closed;
        inner.failure_count = 0;
        inner.success_count = 0;
        inner.consecutive_successes = 0;
        inner.last_failure_time = None;
        inner.total_calls = 0;
        inner.total_rejections = 0;
    }

    /// Gets current metrics for monitoring.
    pub fn get_metrics(&self) -> CircuitMetrics {
        let inner = self.inner.lock().unwrap();
        CircuitMetrics {
            name: inner.name.clone(),
            state: inner.state.as_str().to_string(),
            failure_count: inner.failure_count,
            success_count: inner.success_count,
            total_calls: inner.total_calls,
            total_rejections: inner.total_rejections,
            last_failure_time: inner.last_failure_time,
            consecutive_successes: inner.consecutive_successes,
        }
    }

    /// Gets the circuit breaker name.
    pub fn get_name(&self) -> String {
        self.inner.lock().unwrap().name.clone()
    }

    // Helper methods

    fn should_attempt_reset(inner: &CircuitBreakerInner) -> bool {
        match inner.last_failure_time {
            Some(last_failure) => last_failure.elapsed() >= inner.cooldown,
            None => true,
        }
    }

    fn transition_to(inner: &mut CircuitBreakerInner, new_state: CircuitState) {
        if inner.state == new_state {
            return;
        }

        let old_state = inner.state;
        inner.state = new_state;

        warn!(
            "Circuit '{}' state change: {} -> {}",
            inner.name,
            old_state.as_str(),
            new_state.as_str()
        );
    }

    fn time_until_retry(inner: &CircuitBreakerInner) -> f64 {
        match inner.last_failure_time {
            Some(last_failure) => {
                let elapsed = last_failure.elapsed();
                inner
                    .cooldown
                    .saturating_sub(elapsed)
                    .as_secs_f64()
                    .max(0.0)
            }
            None => 0.0,
        }
    }
}

// =============================================================================
// CIRCUIT BREAKER REGISTRY
// =============================================================================

/// Registry for managing multiple circuit breakers.
#[derive(Debug, Default)]
pub struct CircuitBreakerRegistry {
    breakers: Arc<Mutex<HashMap<String, CircuitBreaker>>>,
}

impl CircuitBreakerRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Gets or creates a circuit breaker by name.
    pub fn get_or_create(&self, name: impl Into<String>) -> CircuitBreaker {
        self.get_or_create_with_config(name, 3, Duration::from_secs(300), 1)
    }

    /// Gets or creates a circuit breaker with custom configuration.
    pub fn get_or_create_with_config(
        &self,
        name: impl Into<String>,
        failure_threshold: u32,
        cooldown: Duration,
        success_threshold: u32,
    ) -> CircuitBreaker {
        let name = name.into();
        let mut breakers = self.breakers.lock().unwrap();

        breakers
            .entry(name.clone())
            .or_insert_with(|| {
                debug!("Created circuit breaker: {}", name);
                CircuitBreaker::with_config(name, failure_threshold, cooldown, success_threshold)
            })
            .clone()
    }

    /// Gets a circuit breaker by name if it exists.
    pub fn get(&self, name: &str) -> Option<CircuitBreaker> {
        self.breakers.lock().unwrap().get(name).cloned()
    }

    /// Removes a circuit breaker from the registry.
    pub fn remove(&self, name: &str) {
        self.breakers.lock().unwrap().remove(name);
    }

    /// Gets all registered circuit breakers.
    pub fn all(&self) -> Vec<CircuitBreaker> {
        self.breakers.lock().unwrap().values().cloned().collect()
    }

    /// Gets metrics for all circuit breakers.
    pub fn get_all_metrics(&self) -> Vec<CircuitMetrics> {
        self.all().iter().map(|b| b.get_metrics()).collect()
    }

    /// Gets names of all open circuits.
    pub fn get_open_circuits(&self) -> Vec<String> {
        self.all()
            .iter()
            .filter(|b| b.is_open())
            .map(|b| b.get_name())
            .collect()
    }

    /// Resets all circuit breakers.
    pub fn reset_all(&self) {
        for breaker in self.all() {
            breaker.reset();
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
    fn test_circuit_breaker_opens_on_failures() {
        let breaker = CircuitBreaker::with_config("test", 3, Duration::from_secs(5), 1);

        assert!(breaker.is_closed());

        // Simulate 3 failures
        for _ in 0..3 {
            breaker.record_failure();
        }

        assert!(breaker.is_open());
    }

    #[test]
    fn test_circuit_breaker_closes_after_success() {
        let breaker = CircuitBreaker::with_config("test", 2, Duration::from_millis(100), 1);

        // Open circuit
        breaker.record_failure();
        breaker.record_failure();
        assert!(breaker.is_open());

        // Wait for cooldown
        std::thread::sleep(Duration::from_millis(150));

        // Should transition to half-open
        assert_eq!(breaker.state(), CircuitState::HalfOpen);

        // Success should close it
        breaker.record_success();
        assert!(breaker.is_closed());
    }

    #[test]
    fn test_registry() {
        let registry = CircuitBreakerRegistry::new();

        let breaker1 = registry.get_or_create("service1");
        let breaker2 = registry.get_or_create("service2");

        assert_eq!(breaker1.get_name(), "service1");
        assert_eq!(breaker2.get_name(), "service2");
        assert_eq!(registry.all().len(), 2);
    }

    #[test]
    fn test_metrics() {
        let breaker = CircuitBreaker::new("test");
        breaker.record_success();
        breaker.record_failure();

        let metrics = breaker.get_metrics();
        assert_eq!(metrics.success_count, 1);
        assert_eq!(metrics.failure_count, 1);
    }
}
