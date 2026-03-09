# Testing Setup & Patterns

This document describes the testing architecture using `cargo test`, including project layout, test modules, fixtures, and patterns for testing applications that use global state.

## Project Structure

```
src/
├── lib.rs                      # Library root (re-exports modules)
├── main.rs                     # Binary entry point
│
├── utils/
│   ├── mod.rs
│   ├── config.rs               # ConfigManager + #[cfg(test)] mod tests
│   ├── logging.rs
│   └── error_handling.rs
│
├── core/
│   ├── mod.rs
│   └── features.rs             # Core logic + inline tests
│
└── security/
    ├── mod.rs
    ├── activation.rs
    └── credentials.rs

tests/                          # Integration tests (each file is a separate crate)
├── test_api.rs
├── test_config_integration.rs
└── common/
    └── mod.rs                  # Shared test helpers

benches/                        # Benchmarks (requires nightly or criterion)
└── benchmarks.rs
```

## Cargo.toml Test Configuration

```toml
[package]
name = "your-app"
version = "0.1.0"
edition = "2021"

[dependencies]
# ... your dependencies

[dev-dependencies]
tempfile = "3"              # Temporary directories for tests
mockall = "0.13"            # Mocking framework
assert_cmd = "2"            # Test CLI binaries
predicates = "3"            # Assertion helpers
criterion = "0.5"           # Benchmarking
tokio = { version = "1", features = ["test-util", "macros", "rt-multi-thread"] }
wiremock = "0.6"            # HTTP mocking

[[bench]]
name = "benchmarks"
harness = false             # Use criterion instead of built-in harness
```

## Inline Unit Tests (The Rust Way)

The idiomatic Rust approach is to put unit tests directly alongside the code they test, inside a `#[cfg(test)]` module:

```rust
// src/utils/config.rs

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

pub struct ConfigManager {
    data: HashMap<String, String>,
}

static INSTANCE: OnceLock<Mutex<ConfigManager>> = OnceLock::new();

impl ConfigManager {
    pub fn instance() -> &'static Mutex<ConfigManager> {
        INSTANCE.get_or_init(|| {
            Mutex::new(ConfigManager {
                data: HashMap::new(),
            })
        })
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        self.data.get(key)
    }

    pub fn get_or_default<'a>(&'a self, key: &str, default: &'a str) -> &'a str {
        self.data.get(key).map(|s| s.as_str()).unwrap_or(default)
    }

    pub fn set(&mut self, key: String, value: String) {
        self.data.insert(key, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a fresh ConfigManager for each test (avoids singleton state leaks).
    fn fresh_config() -> ConfigManager {
        ConfigManager {
            data: HashMap::new(),
        }
    }

    #[test]
    fn test_get_default_value() {
        let config = fresh_config();
        assert_eq!(config.get_or_default("nonexistent", "default_value"), "default_value");
    }

    #[test]
    fn test_set_and_get() {
        let mut config = fresh_config();
        config.set("test_key".into(), "test_value".into());
        assert_eq!(config.get("test_key"), Some(&"test_value".to_string()));
    }

    #[test]
    fn test_overwrite_value() {
        let mut config = fresh_config();
        config.set("key".into(), "first".into());
        config.set("key".into(), "second".into());
        assert_eq!(config.get("key"), Some(&"second".to_string()));
    }
}
```

## Testing Singletons

### The Problem

Global singletons (`OnceLock`, `lazy_static`, etc.) persist across tests within the same binary. Since `cargo test` runs tests in parallel threads within a single process, singleton state from one test can leak into another.

### The Solution

For unit tests, bypass the singleton and construct fresh instances directly:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Create a test-only instance, bypassing the singleton.
    fn test_instance() -> ConfigManager {
        ConfigManager {
            data: HashMap::new(),
        }
    }

    #[test]
    fn test_isolated() {
        let mut config = test_instance();
        config.set("key".into(), "value".into());
        assert_eq!(config.get("key"), Some(&"value".to_string()));
    }
}
```

For integration tests that must use the real singleton, use `serial_test` to prevent parallel execution:

```rust
// tests/test_config_integration.rs
use serial_test::serial;

#[test]
#[serial]
fn test_singleton_behavior_a() {
    // Only one #[serial] test runs at a time
    let config = ConfigManager::instance().lock().unwrap();
    // ...
}

#[test]
#[serial]
fn test_singleton_behavior_b() {
    let config = ConfigManager::instance().lock().unwrap();
    // ...
}
```

Add to `Cargo.toml`:
```toml
[dev-dependencies]
serial_test = "3"
```

## Test Patterns

### Unit Test Pattern (inline)

```rust
// src/core/features.rs

pub fn calculate_discount(price: f64, tier: &str) -> f64 {
    match tier {
        "gold" => price * 0.8,
        "silver" => price * 0.9,
        _ => price,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gold_discount() {
        assert!((calculate_discount(100.0, "gold") - 80.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_silver_discount() {
        assert!((calculate_discount(100.0, "silver") - 90.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_no_discount_for_unknown_tier() {
        assert!((calculate_discount(100.0, "bronze") - 100.0).abs() < f64::EPSILON);
    }
}
```

### Integration Test Pattern

```rust
// tests/test_api.rs
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path};

#[tokio::test]
async fn test_fetch_remote_config() {
    // Start mock server
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/config.json"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"status": "ok"}))
        )
        .mount(&mock_server)
        .await;

    // Use the mock server URL in your client
    let url = format!("{}/config.json", mock_server.uri());
    let response: serde_json::Value = reqwest::get(&url)
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(response["status"], "ok");
}
```

### Shared Test Helpers

```rust
// tests/common/mod.rs
use std::path::{Path, PathBuf};
use tempfile::TempDir;

pub struct TestFixture {
    pub temp_dir: TempDir,
}

impl TestFixture {
    pub fn new() -> Self {
        Self {
            temp_dir: TempDir::new().expect("Failed to create temp dir"),
        }
    }

    pub fn path(&self) -> &Path {
        self.temp_dir.path()
    }

    /// Create a test file with content.
    pub fn create_file(&self, name: &str, content: &str) -> PathBuf {
        let path = self.temp_dir.path().join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, content).unwrap();
        path
    }

    /// Create a mock config directory.
    pub fn create_config_dir(&self) -> PathBuf {
        let config_dir = self.temp_dir.path().join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("settings.json"),
            r#"{"key": "value"}"#,
        ).unwrap();
        config_dir
    }
}

// Usage in integration tests:
// tests/test_config_integration.rs
mod common;

#[test]
fn test_config_loading() {
    let fixture = common::TestFixture::new();
    let config_dir = fixture.create_config_dir();
    // ... test with config_dir
}
```

### Performance / Benchmark Pattern

```rust
// benches/benchmarks.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use your_app::utils::config::ConfigManager;

fn bench_config_reads(c: &mut Criterion) {
    let mut config = ConfigManager::default();

    // Setup: insert 1000 keys
    for i in 0..1000 {
        config.set(format!("key_{i}"), format!("value_{i}"));
    }

    c.bench_function("config_read_1000", |b| {
        b.iter(|| {
            for i in 0..1000 {
                black_box(config.get(&format!("key_{i}")));
            }
        })
    });
}

criterion_group!(benches, bench_config_reads);
criterion_main!(benches);
```

### Testing with Platform-Specific Behavior

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "windows")]
    fn test_windows_path_handling() {
        let path = normalize_path(r"C:\Users\test\file.txt");
        assert!(path.is_absolute());
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_linux_path_handling() {
        let path = normalize_path("/home/test/file.txt");
        assert!(path.is_absolute());
    }

    #[test]
    fn test_cross_platform_path() {
        // This test runs on all platforms
        let path = normalize_path("relative/path/file.txt");
        assert!(path.is_relative());
    }
}
```

## Mocking External Dependencies

### Mocking with `mockall`

```rust
use mockall::automock;

#[automock]
pub trait CredentialStore {
    fn get(&self, service: &str, username: &str) -> Option<String>;
    fn set(&mut self, service: &str, username: &str, password: &str);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credential_retrieval() {
        let mut mock = MockCredentialStore::new();
        mock.expect_get()
            .with(
                mockall::predicate::eq("my_app"),
                mockall::predicate::eq("api_key"),
            )
            .returning(|_, _| Some("test_key_12345".to_string()));

        let result = mock.get("my_app", "api_key");
        assert_eq!(result, Some("test_key_12345".to_string()));
    }
}
```

### Mocking File System

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_with_mock_filesystem() {
        let temp_dir = TempDir::new().unwrap();

        // Create directory structure
        std::fs::create_dir_all(temp_dir.path().join("config")).unwrap();
        std::fs::create_dir_all(temp_dir.path().join("logs")).unwrap();

        // Create test files
        std::fs::write(
            temp_dir.path().join("config/settings.json"),
            r#"{"key": "value"}"#,
        ).unwrap();

        // Test with the temp directory
        let config_path = temp_dir.path().join("config/settings.json");
        assert!(config_path.exists());

        // TempDir is automatically cleaned up when dropped
    }
}
```

## Custom Test Attributes & Markers

Use `#[ignore]` for slow tests and conditional compilation for platform tests:

```rust
#[test]
#[ignore] // Run with: cargo test -- --ignored
fn test_slow_operation() {
    std::thread::sleep(std::time::Duration::from_secs(10));
    assert!(true);
}

#[test]
#[cfg(feature = "integration")]
fn test_requires_external_service() {
    // Only runs with: cargo test --features integration
}
```

## Running Tests

### Common Commands

```bash
# Run all tests
cargo test

# Run with verbose output
cargo test -- --nocapture

# Run specific test
cargo test test_get_default_value

# Run tests in a specific module
cargo test utils::config::tests

# Run tests matching pattern
cargo test config

# Run ignored (slow) tests
cargo test -- --ignored

# Run all tests including ignored
cargo test -- --include-ignored

# Run with specific number of threads
cargo test -- --test-threads=1

# Run benchmarks
cargo bench

# Run tests with coverage (requires cargo-tarpaulin)
cargo tarpaulin --out html

# Run only integration tests
cargo test --test '*'

# Run only unit tests (lib tests)
cargo test --lib

# Run doc tests
cargo test --doc
```

### CI Configuration

```yaml
# .github/workflows/test.yml
name: Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
        rust: [stable, "1.75"]  # MSRV + stable

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@master
        with:
          toolchain: ${{ matrix.rust }}
          components: clippy, rustfmt

      - name: Cache cargo dependencies
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}

      - name: Check formatting
        run: cargo fmt --check

      - name: Run clippy
        run: cargo clippy -- -D warnings

      - name: Run tests
        run: cargo test --verbose

      - name: Run doc tests
        run: cargo test --doc

  coverage:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Install tarpaulin
        run: cargo install cargo-tarpaulin

      - name: Generate coverage
        run: cargo tarpaulin --out xml

      - name: Upload coverage
        uses: codecov/codecov-action@v4
```

## Related Documentation

- [ARCHITECTURE.md](ARCHITECTURE.md) - Project structure
- [ERROR-REPORTING.md](ERROR-REPORTING.md) - Error handling tests
