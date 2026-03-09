# Utility Systems

This guide covers the core utility systems for caching, parallel processing, validation, path security, and file operations.

## Overview

| System | Purpose | Template File |
|--------|---------|---------------|
| Caching | In-memory caching with TTL and LRU eviction | `cache.rs` |
| Async Operations | Parallel batch processing with progress tracking | `async_operations.rs` |
| Validation | Form and input validation framework | `validation.rs` |
| Path Security | Path traversal prevention and sanitization | `path_security.rs` |
| File Operations | Safe file operations with batch processing | `file_operations.rs` |

---

## Caching System

Thread-safe in-memory caching with time-based expiration and LRU eviction.

### Core Components

```
┌─────────────────────────────────────────────────────────────┐
│                      CacheManager                           │
│                    (Singleton)                              │
│                                                             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐        │
│  │ TimedCache  │  │ TimedCache  │  │ TimedCache  │        │
│  │ "api"       │  │ "config"    │  │ "images"    │        │
│  │ ttl=300s    │  │ ttl=600s    │  │ ttl=3600s   │        │
│  └─────────────┘  └─────────────┘  └─────────────┘        │
└─────────────────────────────────────────────────────────────┘
```

### TimedCache Struct

```rust
use your_app::utils::cache::{TimedCache, CacheStats};
use std::time::Duration;

// Create cache with TTL and max size
let cache: TimedCache<String, serde_json::Value> = TimedCache::new(
    Duration::from_secs(300),
    128,
    "api_responses",
);

// Store value
cache.set(
    "user_123".into(),
    serde_json::json!({"name": "John", "email": "john@example.com"}),
);

// Retrieve value (returns None if expired or not found)
let user = cache.get("user_123");

// Check if key exists
if cache.contains_key("user_123") {
    println!("User in cache");
}

// Invalidate specific key
cache.invalidate("user_123");

// Get cache statistics
let stats: CacheStats = cache.stats();
// CacheStats { name: "api_responses", size: 45, hits: 150, misses: 20, hit_rate_percent: 88.24 }

// Clear all entries
cache.clear();

// Cleanup expired entries (for maintenance)
let removed_count = cache.cleanup_expired();
```

### Memoization with Closures

```rust
use std::collections::HashMap;
use std::sync::Mutex;

/// Simple memoization wrapper using a HashMap behind a Mutex.
fn memoize<A, R, F>(f: F) -> impl FnMut(A) -> R
where
    A: Eq + std::hash::Hash + Clone,
    R: Clone,
    F: Fn(A) -> R,
{
    let cache = Mutex::new(HashMap::new());
    move |arg: A| {
        let mut map = cache.lock().unwrap();
        if let Some(result) = map.get(&arg) {
            return result.clone();
        }
        let result = f(arg.clone());
        map.insert(arg, result.clone());
        result
    }
}

// Usage
let mut cached_lookup = memoize(|user_id: u64| {
    // Expensive database/API call
    fetch_from_database(user_id)
});

let profile = cached_lookup(123); // First call: fetches from DB
let profile = cached_lookup(123); // Second call: returns cached value
```

### CacheManager (Multiple Caches)

```rust
use your_app::utils::cache::CacheManager;
use std::time::Duration;

// Get singleton instance (uses OnceLock internally)
let manager = CacheManager::instance();

// Get or create named caches
let api_cache = manager.get_cache::<String, String>("api", Duration::from_secs(300), 100);
let config_cache = manager.get_cache::<String, String>("config", Duration::from_secs(3600), 50);

// Clear all managed caches
manager.clear_all();

// Get stats for all caches
let all_stats = manager.all_stats();

// Cleanup expired entries in all caches
manager.cleanup_all_expired();
```

### When to Use Each Pattern

| Pattern | Use Case |
|---------|----------|
| `TimedCache` | Direct control, multiple keys, complex cache logic |
| Memoization closure | Pure function caching, permanent in-process caching |
| `CacheManager` | Application-wide cache organization |

---

## Parallel Operations

Parallel batch processing with progress tracking and cancellation via channels.

### BatchProcessor

```rust
use your_app::utils::async_operations::{BatchProcessor, BatchStatus, BatchProgress};
use std::sync::Arc;

// Create processor with worker count
let processor = BatchProcessor::new(4, "file_processor");

// Define progress callback
let on_progress = |progress: &BatchProgress| {
    println!("Progress: {:.1}%", progress.percent_complete());
    println!("Completed: {}/{}", progress.completed, progress.total);
    println!("Failed: {}", progress.failed);
    if let Some(eta) = progress.estimated_remaining_secs() {
        println!("ETA: {:.1}s", eta);
    }
};

// Process items
let results = processor.process_batch(
    &["file1.txt", "file2.txt", "file3.txt"],
    |file| process_file(file),
    Some(&on_progress),
)?;

// Check results
for result in &results {
    match &result.outcome {
        Ok(value) => println!("OK: {} -> {:?}", result.item, value),
        Err(e) => println!("FAILED: {} - {}", result.item, e),
    }
}

// Cancel a running batch (from another thread)
processor.cancel();
```

### BatchProgress Fields

```rust
pub struct BatchProgress {
    pub total: usize,
    pub completed: usize,
    pub failed: usize,
    pub current_item: String,
    pub status: BatchStatus,
    started_at: Instant,
}

impl BatchProgress {
    pub fn percent_complete(&self) -> f64 { ... }
    pub fn elapsed_secs(&self) -> f64 { ... }
    pub fn estimated_remaining_secs(&self) -> Option<f64> { ... }
}
```

### Simple Parallel Helpers

```rust
use rayon::prelude::*;
use std::thread;

// parallel_map using rayon
let results: Vec<i64> = vec![1, 2, 3, 4, 5]
    .par_iter()
    .map(|x| x * x)
    .collect();
// Returns: [1, 4, 9, 16, 25]

// Spawn a background task and get a JoinHandle
let handle = thread::spawn(|| {
    std::thread::sleep(std::time::Duration::from_secs(5));
    "done"
});
// ... do other work ...
let result = handle.join().unwrap(); // Blocks until complete
```

### Async Batch Processing with Tokio

For use in async contexts:

```rust
use tokio::task::JoinSet;

async fn process_all(items: Vec<i32>) -> Vec<i32> {
    let mut set = JoinSet::new();

    for item in items {
        set.spawn(async move { item * 2 });
    }

    let mut results = Vec::new();
    while let Some(res) = set.join_next().await {
        results.push(res.unwrap());
    }
    results
}

#[tokio::main]
async fn main() {
    let results = process_all(vec![1, 2, 3, 4, 5]).await;
    for r in &results {
        println!("{}", r);
    }
}
```

### Shared State with Arc/Mutex and Channels

```rust
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

// Shared counter across threads
let counter = Arc::new(Mutex::new(0u64));
let mut handles = vec![];

for _ in 0..4 {
    let counter = Arc::clone(&counter);
    handles.push(thread::spawn(move || {
        let mut num = counter.lock().unwrap();
        *num += 1;
    }));
}

for handle in handles {
    handle.join().unwrap();
}
println!("Counter: {}", *counter.lock().unwrap());

// Channel-based message passing
let (tx, rx) = mpsc::channel();

thread::spawn(move || {
    tx.send("work complete").unwrap();
});

let message = rx.recv().unwrap();
println!("Received: {message}");
```

---

## Validation Framework

Composable validators returning `Result<(), String>`.

### Individual Validators

```rust
use your_app::utils::validation::{
    validate_required,
    validate_length,
    validate_email,
    validate_url,
    validate_path_exists,
    validate_hex_color,
    validate_number_range,
    validate_filename,
};

// All validators return: Result<(), String>
// Ok(()) on success, Err(message) on failure

// Required field
validate_required(value, "Username")?;
// Err("Username is required") or Ok(())

// Length validation
validate_length(name, 3, 50, "Name")?;

// Email validation
validate_email("user@example.com", "Email")?;

// URL validation
validate_url("https://example.com", "Website", true /* require_https */)?;

// Path existence
validate_path_exists("/path/to/file", "Input File")?;
validate_directory_exists("/path/to/dir", "Output Directory")?;

// Hex color
validate_hex_color("#FF0000", "Background Color")?;

// Number range
validate_number_range(value, 1.0, 100.0, "Age")?;

// Filename (no path separators)
validate_filename("my_file.txt", "Filename")?;
```

### FormValidator Struct

```rust
use your_app::utils::validation::{FormValidator, validate_required, validate_email};
use std::collections::HashMap;

// Create validator and add fields
let mut validator = FormValidator::new();
validator.add_field("username", username.clone());
validator.add_field("email", email.clone());
validator.add_field("age", age.to_string());

// Define validation rules as closures
let validations: Vec<(&str, Vec<Box<dyn Fn(&str, &str) -> Result<(), String>>>)> = vec![
    ("username", vec![
        Box::new(validate_required),
        Box::new(|v, n| validate_length(v, 3, 50, n)),
    ]),
    ("email", vec![
        Box::new(validate_required),
        Box::new(validate_email),
    ]),
    ("age", vec![
        Box::new(|v, n| validate_number_range(v.parse::<f64>().unwrap_or(0.0), 18.0, 120.0, n)),
    ]),
];

// Run all validations
if validator.validate_all(&validations) {
    // All valid - proceed
    save_user(&validator.fields);
} else {
    // Show errors
    eprintln!("{}", validator.error_summary());
    // "Validation errors:\n- email: Email is not a valid email address"

    // Or get specific error
    if let Some(msg) = validator.error_for("email") {
        eprintln!("Email error: {msg}");
    }
}
```

### Color Utilities

```rust
use your_app::utils::validation::{is_valid_hex_color, normalize_hex_color};

// Check validity
is_valid_hex_color("#FF0000");  // true
is_valid_hex_color("#fff");     // false (must be 6 digits)
is_valid_hex_color("FF0000");   // false (must have #)

// Normalize (uppercase with #)
normalize_hex_color("#ff0000");  // "#FF0000"
normalize_hex_color("ff0000");   // "#FF0000"
```

---

## Path Security

Prevent path traversal attacks and ensure safe file operations.

### Path Sanitization

```rust
use your_app::utils::path_security::{
    sanitize_filename,
    safe_join,
    is_safe_path,
    validate_path_input,
    contains_path_traversal,
    PathSecurityError,
};
use std::path::{Path, PathBuf};

// Sanitize filename (removes dangerous characters)
let safe_name = sanitize_filename("../../../etc/passwd");
// Returns: "_______etc_passwd"

let safe_name = sanitize_filename("my<file>:name?.txt");
// Returns: "my_file__name_.txt"

// Safe path joining (validates result is within base)
let base_dir = PathBuf::from("/app/uploads");
let safe_path = safe_join(&base_dir, &[user_input, "file.txt"])
    .map_err(|_| "Path traversal detected!")?;

// Check if path is within allowed directory
if is_safe_path(user_path, &base_dir, false /* allow_symlinks */) {
    // Safe to use
}

// Validate user input path
match validate_path_input(user_input, "/app/data", true /* must_exist */, false /* allow_absolute */) {
    Some(safe_path) => { /* use safe_path */ }
    None => eprintln!("Invalid path"),
}

// Check for traversal patterns
if contains_path_traversal("../secret") {
    eprintln!("Traversal detected!");
}
```

### Secure File Permissions

```rust
use your_app::utils::path_security::{
    set_secure_file_permissions,
    set_secure_directory_permissions,
};

// Set file to owner-only (chmod 600 on Unix, icacls on Windows)
set_secure_file_permissions("/path/to/credentials.json")?;

// Set directory to owner-only (chmod 700 on Unix)
set_secure_directory_permissions("/path/to/keys/")?;
```

### Safe Temporary Files

```rust
use your_app::utils::path_security::get_safe_temp_path;

// Get safe temp file path with sanitized prefix/suffix
let temp_file = get_safe_temp_path("download_", ".tmp")?;
```

---

## File Operations

Safe file operations with batch processing and protected path detection.

### Basic Operations

```rust
use your_app::utils::file_operations::{
    copy_file,
    move_file,
    delete_file,
    create_directory,
    delete_directory,
};
use std::path::Path;

// Copy with path validation
copy_file(source_path, destination_path)?;

// Move with path validation
move_file(source_path, destination_path)?;

// Delete file (returns Ok even if already deleted)
delete_file(file_path)?;

// Create directory (recursive by default)
create_directory(dir_path)?;

// Delete directory with safety checks
delete_directory(
    dir_path,
    true,   // recursive: delete contents
    false,  // force: require confirmation for large deletes
)?;
```

### Batch Operations

```rust
use your_app::utils::file_operations::{
    batch_copy,
    batch_move,
    batch_delete,
    FileOperationResult,
};

// Batch copy with progress
let results = batch_copy(
    &files,
    output_dir,
    4,      // max_workers
    true,   // preserve_structure
    Some(source_dir),
    Some(|done, total| println!("{done}/{total}")),
)?;

// Check results
for result in &results {
    match &result.outcome {
        Ok(_) => println!("Copied: {} -> {}", result.source.display(), result.destination.display()),
        Err(e) => println!("Failed: {} - {}", result.source.display(), e),
    }
}

// Batch move
let results = batch_move(&files, destination_dir, 4)?;

// Batch delete
let results = batch_delete(&files, 4)?;
```

### File Listing

```rust
use your_app::utils::file_operations::{
    list_files,
    list_directories,
    get_file_size,
    get_directory_size,
    format_file_size,
};
use std::path::Path;

// List files with glob pattern (validates pattern for safety)
let files = list_files(directory, "*.txt", true /* recursive */, true /* files_only */)?;

// List subdirectories
let dirs = list_directories(directory, false /* recursive */)?;

// Get sizes
let size = get_file_size(file_path)?;         // bytes (u64)
let total_size = get_directory_size(dir_path)?; // bytes (u64)

// Format for display
let formatted = format_file_size(1_536_000);  // "1.5 MB"
```

### Filename Utilities

```rust
use your_app::utils::file_operations::{
    ensure_unique_filename,
    safe_filename,
};
use std::path::PathBuf;

// Get unique filename (adds _1, _2, etc. if exists)
let unique_path = ensure_unique_filename(&PathBuf::from("/path/to/file.txt"));
// Returns: /path/to/file_1.txt if file.txt exists

// Sanitize filename for filesystem
let safe = safe_filename("My File: <Draft>.txt", 200);
// Returns: "My_File___Draft_.txt"
```

### Protected Paths

The system automatically protects system directories:

```rust
use your_app::utils::file_operations::is_protected_path;
use std::path::PathBuf;

// These operations will fail:
delete_directory(&dirs::home_dir().unwrap(), true, false);   // Protected
delete_directory(&PathBuf::from(r"C:\Windows"), true, false); // Protected
delete_directory(&PathBuf::from("C:\\"), true, false);         // Protected
delete_directory(&PathBuf::from("/usr"), true, false);         // Protected

// Check if path is protected
if is_protected_path(some_path) {
    eprintln!("Cannot modify system path");
}
```

### Safety Features

- **Path traversal prevention**: All operations validate paths
- **Protected paths**: System directories cannot be deleted
- **Minimum depth check**: Shallow paths (< 3 levels) cannot be deleted
- **Large delete warning**: Recursive deletes of >1000 files require `force=true`
- **Glob result limits**: Maximum 10,000 results from glob operations

---

## Integration Example

Combining all systems in a file processor:

```rust
use std::path::{Path, PathBuf};
use anyhow::Result;

use your_app::utils::cache::{CacheManager, TimedCache};
use your_app::utils::async_operations::BatchProcessor;
use your_app::utils::validation::{FormValidator, validate_required, validate_directory_exists};
use your_app::utils::path_security::{safe_join, sanitize_filename};
use your_app::utils::file_operations::{batch_copy, list_files};

pub struct FileProcessor {
    cache_manager: &'static CacheManager,
    file_cache: TimedCache<String, Vec<PathBuf>>,
    processor: BatchProcessor,
}

impl FileProcessor {
    pub fn new() -> Self {
        let cache_manager = CacheManager::instance();
        Self {
            cache_manager,
            file_cache: TimedCache::new(
                std::time::Duration::from_secs(300),
                128,
                "files",
            ),
            processor: BatchProcessor::new(4, "file_processor"),
        }
    }

    /// Validate input settings.
    pub fn validate_settings(&self, source_dir: &str, dest_dir: &str) -> Result<()> {
        let mut validator = FormValidator::new();
        validator.add_field("source", source_dir.to_string());
        validator.add_field("destination", dest_dir.to_string());

        let validations = vec![
            ("source", vec![
                Box::new(validate_required) as Box<dyn Fn(&str, &str) -> Result<(), String>>,
                Box::new(validate_directory_exists),
            ]),
            ("destination", vec![
                Box::new(validate_required),
            ]),
        ];

        if !validator.validate_all(&validations) {
            anyhow::bail!(validator.error_summary());
        }
        Ok(())
    }

    /// Get files with caching.
    pub fn get_file_list(&self, directory: &str, pattern: &str) -> Result<Vec<PathBuf>> {
        let cache_key = format!("{directory}:{pattern}");
        if let Some(cached) = self.file_cache.get(&cache_key) {
            return Ok(cached);
        }
        let files = list_files(Path::new(directory), pattern, true, true)?;
        self.file_cache.set(cache_key, files.clone());
        Ok(files)
    }

    /// Process files with validation and safety.
    pub fn process_files(
        &self,
        source_dir: &str,
        dest_dir: &str,
        pattern: &str,
        progress_callback: Option<&dyn Fn(usize, usize)>,
    ) -> Result<Vec<PathBuf>> {
        // Validate inputs
        self.validate_settings(source_dir, dest_dir)?;

        // Get file list (cached)
        let files = self.get_file_list(source_dir, pattern)?;

        // Process in parallel
        let dest = PathBuf::from(dest_dir);
        let results = self.processor.process_batch(
            &files,
            |file_path| {
                let safe_name = sanitize_filename(
                    file_path.file_name().unwrap().to_str().unwrap(),
                );
                let dest_path = safe_join(&dest, &[&safe_name])?;
                // Do processing...
                Ok(dest_path)
            },
            progress_callback.map(|cb| {
                move |progress: &_| cb(progress.completed, progress.total)
            }).as_ref(),
        )?;

        Ok(results.into_iter().filter_map(|r| r.outcome.ok()).collect())
    }
}
```

---

## See Also

- [ERROR-REPORTING.md](ERROR-REPORTING.md) - Logging and error handling
- [TESTING.md](TESTING.md) - Testing patterns for utilities
- [CROSS-PLATFORM.md](CROSS-PLATFORM.md) - Platform-specific considerations
- [UI-COMPONENTS.md](UI-COMPONENTS.md) - Reusable UI components
