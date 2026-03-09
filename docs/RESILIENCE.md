# Resilience Patterns

This document describes resilience patterns for building robust Rust applications that degrade gracefully when external services fail.

## Overview

Things break. APIs go down, services timeout, OAuth tokens expire. Rather than hard failures, applications should degrade gracefully using these patterns:

| Pattern | Purpose | Use When |
|---------|---------|----------|
| **Retries** | Handle transient failures | Network blips, temporary 503s |
| **Circuit Breaker** | Prevent cascading failures | Service is persistently down |
| **Health Monitoring** | Proactive failure detection | Critical service dependencies |
| **Fallbacks** | Provide alternatives | Primary service unavailable |

---

## Architecture

```
┌────────────────────────────────────────────────────────────────────┐
│                      Resilience Layer                               │
├────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌──────────────────┐    ┌──────────────────┐                      │
│  │  Circuit Breaker │    │  Health Monitor  │                      │
│  │  (Per-service)   │    │  (Background)    │                      │
│  └────────┬─────────┘    └────────┬─────────┘                      │
│           │                       │                                 │
│  ┌────────┴─────────┐    ┌────────┴─────────┐                      │
│  │ States:          │    │ Checks:          │                      │
│  │ - Closed (OK)    │    │ - API endpoints  │                      │
│  │ - Open (Failed)  │    │ - Status pages   │                      │
│  │ - Half-Open      │    │ - Dependencies   │                      │
│  └──────────────────┘    └──────────────────┘                      │
│                                                                     │
│  ┌──────────────────────────────────────────┐                      │
│  │            Fallback Registry              │                      │
│  │  Primary -> Secondary -> Tertiary -> Err │                      │
│  └──────────────────────────────────────────┘                      │
│                                                                     │
└────────────────────────────────────────────────────────────────────┘
```

---

## Circuit Breaker Pattern

The circuit breaker prevents your application from repeatedly calling a failing service. Instead of hammering a broken endpoint, it "trips" and stops trying, giving the service time to recover.

### How It Works

```
                    ┌─────────────┐
           Success  │             │  Failure
        ┌──────────►│   CLOSED    │◄───────────┐
        │           │  (Normal)   │            │
        │           └──────┬──────┘            │
        │                  │                   │
        │         Threshold exceeded           │
        │                  │                   │
        │                  ▼                   │
        │           ┌─────────────┐            │
        │           │             │            │
        │           │    OPEN     │────────────┘
        │           │  (Failing)  │   Reject all calls
        │           └──────┬──────┘
        │                  │
        │          Cooldown expires
        │                  │
        │                  ▼
        │           ┌─────────────┐
        │           │             │
        └───────────│  HALF-OPEN  │
           Success  │   (Test)    │
                    └──────┬──────┘
                           │
                    Failure│
                           │
                           ▼
                     Back to OPEN
```

### States

| State | Behavior |
|-------|----------|
| **CLOSED** | Normal operation. Requests pass through. Failures are counted. |
| **OPEN** | Circuit tripped. All requests immediately fail without calling the service. |
| **HALF-OPEN** | After cooldown, allows one test request. Success -> CLOSED, Failure -> OPEN. |

### Configuration

Recommended defaults (based on real-world usage):

```rust
/// Per-service circuit breaker configuration.
struct CircuitBreakerConfig {
    /// Number of failures before the circuit opens.
    failure_threshold: u32,    // default: 3
    /// Seconds to wait before transitioning to half-open.
    cooldown_seconds: u64,     // default: 300 (5 minutes)
    /// Test calls allowed in half-open state.
    half_open_max_calls: u32,  // default: 1
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 3,
            cooldown_seconds: 300,
            half_open_max_calls: 1,
        }
    }
}
```

### When to Use

**Use Circuit Breaker for:**
- External API calls (REST, GraphQL)
- Database connections
- Third-party service integrations
- Anything with network I/O

**Don't use for:**
- In-memory operations
- CPU-bound computations
- Single-shot operations that must succeed

### Integration with an HTTP Client

```rust
use crate::utils::api_client::ApiClient;
use crate::utils::circuit_breaker::{CircuitBreaker, CircuitBreakerRegistry, CircuitOpenError};

// Create registry for managing multiple breakers
let registry = CircuitBreakerRegistry::new();

// Create client with circuit breaker
let github_breaker = registry.get_or_create("github");
let client = ApiClient::new("https://api.github.com")
    .bearer_token("ghp_xxx");

// Wrap calls with circuit breaker
match github_breaker.call(|| client.get("/user")) {
    Ok(user) => { /* use user */ }
    Err(CircuitOpenError) => {
        tracing::warn!("GitHub API is currently unavailable");
        let user = get_cached_user(); // Fallback
    }
    Err(e) => {
        tracing::error!("GitHub API request failed: {e}");
    }
}
```

---

## Health Monitoring

Proactive health checks detect service issues before user requests fail.

### Background Health Checks

```rust
use crate::utils::circuit_breaker::HealthMonitor;
use std::time::Duration;

// Create monitor with check interval
let mut monitor = HealthMonitor::new(Duration::from_secs(180)); // 3 minutes

// Register health checks
monitor.register_check(
    "github_api",
    || {
        let resp = reqwest::blocking::get("https://api.github.com/status")?;
        Ok(resp.status().is_success())
    },
    || circuit_registry.get("github").force_open(),
);

monitor.register_check(
    "database",
    || {
        let row: (i32,) = sqlx::query_as("SELECT 1").fetch_one(&pool)?;
        Ok(row.0 == 1)
    },
    || notify_admin("Database health check failed"),
);

// Start background monitoring (spawns a tokio task or std::thread)
monitor.start();
```

### Status Page Integration

Check external status pages before your requests fail:

```rust
use std::collections::HashMap;

/// Known provider status page URLs.
fn status_pages() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("github", "https://www.githubstatus.com/api/v2/status.json"),
        ("stripe", "https://status.stripe.com/api/v2/status.json"),
        ("aws", "https://health.aws.amazon.com/health/status"),
    ])
}

/// Check if a provider reports healthy status.
async fn check_provider_status(provider: &str) -> bool {
    let Some(url) = status_pages().get(provider) else {
        return true; // Unknown provider, assume healthy
    };

    let Ok(resp) = reqwest::Client::new()
        .get(*url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
    else {
        return true; // Can't check, assume healthy
    };

    let Ok(json) = resp.json::<serde_json::Value>().await else {
        return true;
    };

    matches!(
        json.get("status")
            .and_then(|s| s.get("indicator"))
            .and_then(|i| i.as_str()),
        Some("none" | "operational")
    )
}
```

---

## Fallback Strategies

When primary services fail, fallbacks provide degraded but functional behavior.

### Fallback Chain Pattern

```rust
use crate::utils::circuit_breaker::FallbackChain;

// Define fallback chain
let mut tts_chain = FallbackChain::<AudioOutput>::new("text_to_speech");
tts_chain.add("azure_tts", |text| azure_tts_client.synthesize(text));
tts_chain.add("google_tts", |text| google_tts_client.synthesize(text));
tts_chain.add("local_espeak", |text| local_espeak.synthesize(text));

// Execute with automatic fallback
let audio = tts_chain.execute("Hello, world!")?;
// Tries Azure first, then Google, then local espeak
```

### Fallback Types

| Type | Description | Example |
|------|-------------|---------|
| **Alternative Provider** | Different service, same result | Azure TTS -> Google TTS |
| **Cached Data** | Return stale but valid data | Return cached user profile |
| **Degraded Mode** | Reduced functionality | Skip analytics, continue operation |
| **Default Value** | Safe default | Return empty `Vec` instead of error |
| **Graceful Error** | Informative failure | "Service temporarily unavailable" |

### Example: Multi-Level Fallback

```rust
use crate::utils::circuit_breaker::CircuitOpenError;

fn get_weather(location: &str) -> WeatherData {
    // Level 1: Primary API with circuit breaker
    match weather_breaker.call(|| weather_api.get_current(location)) {
        Ok(data) => return data,
        Err(CircuitOpenError) => {
            tracing::info!("Weather API circuit open, trying cache");
        }
        Err(e) => {
            tracing::warn!("Weather API error: {e}");
        }
    }

    // Level 2: Cached data (may be stale)
    if let Some(cached) = weather_cache.get(location) {
        if cached.age_minutes() < 60 {
            tracing::info!("Using cached weather data");
            return cached.data;
        }
    }

    // Level 3: Alternative provider
    if let Ok(data) = backup_weather_api.get_current(location) {
        return data;
    }

    // Level 4: Graceful degradation
    WeatherData {
        location: location.to_string(),
        status: Status::Unavailable,
        message: "Weather data temporarily unavailable".into(),
    }
}
```

---

## Integration with Error Reporting

Circuit breakers integrate with the existing error reporting system.

### Logging Circuit Events

```rust
use tracing::{warn, info};

impl CircuitBreaker {
    fn on_state_change(&self, old_state: CircuitState, new_state: CircuitState) {
        warn!(
            name = %self.name,
            "Circuit state change: {old_state:?} -> {new_state:?}"
        );

        if new_state == CircuitState::Open {
            ErrorReporter::instance().log_action(&format!(
                "Circuit breaker '{}' opened after {} failures",
                self.name, self.failure_count
            ));
        }
    }
}
```

### Metrics and Monitoring

Track circuit breaker metrics for observability:

```rust
use chrono::{DateTime, Utc};

/// Metrics for a circuit breaker.
struct CircuitMetrics {
    name: String,
    state: CircuitState,
    failure_count: u32,
    success_count: u32,
    last_failure_time: Option<DateTime<Utc>>,
    last_state_change: DateTime<Utc>,
    total_calls: u64,
    total_rejections: u64, // Calls rejected while open
}

/// Export metrics for monitoring.
fn get_all_metrics(registry: &CircuitBreakerRegistry) -> Vec<CircuitMetrics> {
    registry.all().iter().map(|b| b.get_metrics()).collect()
}
```

---

## Best Practices

### 1. One Breaker Per Service Category

Group related endpoints under one breaker:

```rust
// Good: One breaker per service
registry.get_or_create("home_assistant");   // All HA endpoints
registry.get_or_create("apple_ecosystem");  // Calendar, Reminders, etc.
registry.get_or_create("gmail");

// Bad: One breaker per endpoint
registry.get_or_create("ha_lights");
registry.get_or_create("ha_thermostat");  // Too granular
```

### 2. Configure Thresholds Appropriately

Different services need different thresholds:

```rust
// Critical, fast-failing services
registry.get_or_create_with("payment_api", CircuitBreakerConfig {
    failure_threshold: 2,
    cooldown_seconds: 60,
    ..Default::default()
});

// Less critical, slower services
registry.get_or_create_with("analytics_api", CircuitBreakerConfig {
    failure_threshold: 5,
    cooldown_seconds: 600,
    ..Default::default()
});
```

### 3. Always Have a Fallback Strategy

Never let a circuit breaker opening crash your application:

```rust
// Bad: No fallback
fn get_data() -> Result<Data> {
    breaker.call(|| api.get_data()) // Returns CircuitOpenError
}

// Good: Graceful degradation
fn get_data() -> Data {
    match breaker.call(|| api.get_data()) {
        Ok(data) => data,
        Err(_) => get_cached_data().unwrap_or(Data::default()),
    }
}
```

### 4. Log State Changes

Always log when circuits open/close for debugging:

```rust
breaker.set_on_state_change(|old, new| {
    tracing::warn!("Circuit '{}': {old:?} -> {new:?}", breaker.name());
});
```

### 5. Test Circuit Behavior

Include tests for circuit breaker behavior:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circuit_opens_after_threshold() {
        let breaker = CircuitBreaker::new("test", CircuitBreakerConfig {
            failure_threshold: 3,
            ..Default::default()
        });

        // Simulate failures
        for _ in 0..3 {
            let _ = breaker.call::<(), _>(|| {
                Err(anyhow::anyhow!("Simulated failure"))
            });
        }

        // Circuit should be open
        assert_eq!(breaker.state(), CircuitState::Open);

        // Next call should be rejected immediately
        let result = breaker.call::<(), _>(|| Ok(()));
        assert!(matches!(result, Err(CircuitOpenError)));
    }
}
```

---

## Template Files

See `docs/templates/circuit_breaker.rs` for a complete implementation.

---

## Related Documentation

- [ERROR-REPORTING.md](ERROR-REPORTING.md) - Error handling and logging
- [API Client Template](templates/api_client.rs) - HTTP client with retries
- [Error Handling Template](templates/error_handling.rs) - Error type hierarchy
