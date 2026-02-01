# Testing Patterns

**Analysis Date:** 2026-02-01

## Test Framework

**Runner:**
- Built-in Rust test framework (`cargo test`)
- No separate config file - uses `#[cfg(test)]` attribute

**Assertion Library:**
- Built-in: `assert!()`, `assert_eq!()`, `assert_ne!()`
- No external assertion libraries used

**Run Commands:**
```bash
cargo test                        # Run all tests
cargo test <test_name>            # Run specific test
cargo test -- --nocapture         # Show test output
cargo test --release              # Run in release mode (faster)
cargo test -- --ignored           # Run ignored tests
cargo test -- --test-threads=1    # Run tests sequentially
```

## Test File Organization

**Location:**
- Mixed approach: both co-located and separate tests
- Unit tests: Co-located in source files using `#[cfg(test)] mod tests` at end of file
- Integration tests: Separate files in `tests/` directory

**Naming:**
- Co-located test modules: `tests` (module name)
- Integration test files: `<name>_test.rs` pattern
- Examples: `simple_journal_test.rs`, `direct_journal_test.rs`, `journal_integration_test.rs`, `debug_journal_test.rs`

**Structure:**
```
src/
├── main.rs           (no tests)
├── lib.rs            (no tests)
├── app_controller.rs (#[cfg(test)] mod tests at end)
├── duckdb_buffer.rs  (#[cfg(test)] mod tests at end)
├── journal_reader.rs (#[cfg(test)] mod tests at end)
├── log_entry.rs      (#[cfg(test)] mod tests at end)
└── web_server.rs     (#[cfg(test)] mod tests at end)
tests/
├── simple_journal_test.rs
├── direct_journal_test.rs
├── journal_integration_test.rs
└── debug_journal_test.rs
```

## Test Structure

**Suite Organization:**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_functionality() {
        // Test code here
        assert!(condition);
    }
}
```

**Patterns:**
- Test modules marked with `#[cfg(test)]` at end of source files
- Individual tests marked with `#[test]`
- Async tests marked with `#[tokio::test]` (used in `web_server.rs`)
- Test helpers defined as regular functions in test module (not `#[test]`)

**Setup pattern:**
```rust
#[test]
fn test_function() {
    let temp_dir = TempDir::new().unwrap();
    let mut instance = Struct::new(temp_dir.path()).unwrap();
    // Test code
}
```

**Teardown pattern:**
- No explicit teardown - relies on Drop trait for TempDir
- Temporary directories cleaned up automatically

## Mocking

**Framework:**
- No dedicated mocking framework (e.g., mockall)
- Minimal mocking required - mostly integration-style tests

**Patterns:**
- `tempfile` crate for temporary directories in tests
- `Command::new("logger")` for system integration tests
- Direct instantiation of structs with test data
- `unwrap()` used extensively in tests (acceptable for test code)

**Example:**
```rust
fn test_duckdb_buffer_creation() {
    let temp_dir = TempDir::new().unwrap();
    let result = DuckDBBuffer::new(temp_dir.path());
    assert!(result.is_ok());
}
```

**What to Mock:**
- File system operations (via `tempfile`)
- System journal (via `Command::new("logger")`)
- Database connections (via temporary DuckDB instances)

**What NOT to Mock:**
- Business logic - tested directly with real implementations

## Fixtures and Factories

**Test Data:**
```rust
#[test]
fn test_log_entry_creation() {
    let mut fields = HashMap::new();
    fields.insert("MESSAGE".to_string(), "Test message".to_string());
    fields.insert("PRIORITY".to_string(), "6".to_string());

    let timestamp = Utc.with_ymd_and_hms(2026, 1, 17, 14, 30, 45).unwrap();
    let entry = LogEntry::new(timestamp, fields.clone());

    assert_eq!(entry.timestamp, timestamp);
}
```

**Location:**
- Test data created inline in each test
- No centralized fixture files or factories
- `TempDir::new()` for directory fixtures
- Hard-coded test timestamps using `Utc.with_ymd_and_hms()`

## Coverage

**Requirements:**
- No explicit coverage requirements enforced
- No coverage reporting configured

**View Coverage:**
- Not configured - would need `tarpaulin` or `llvm-cov`

**Coverage Areas:**
- Core data structures: `LogEntry` - good coverage
- Database layer: `DuckDBBuffer` - good coverage
- Journal reader: `JournalLogReader` - minimal coverage
- Application controller: `ApplicationController` - minimal coverage
- Web server: `web_server.rs` - good coverage

## Test Types

**Unit Tests:**
- Scope: Test individual functions and methods
- Approach: Direct instantiation with test data
- Location: Co-located in source files
- Examples: `test_log_entry_creation()`, `test_minute_key()`

**Integration Tests:**
- Scope: Test system components working together
- Approach: Use real journald and filesystem
- Location: `tests/` directory
- Examples: `test_real_time_journal_detection()`, `test_journal_reader_works()`

**E2E Tests:**
- Not used
- No framework for end-to-end testing

## Common Patterns

**Async Testing:**
```rust
#[tokio::test]
async fn test_api_search_empty_results() {
    let temp_dir = tempfile::tempdir().unwrap();
    let app = create_test_app(temp_dir.path().to_str().unwrap());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/search?start=-1h&end=now")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), AxumStatusCode::OK);
}
```

**Error Testing:**
```rust
#[test]
fn test_application_controller_creation() {
    let temp_dir = TempDir::new().unwrap();
    let result = ApplicationController::new(temp_dir.path());
    assert!(result.is_ok());
}
```

**Helper Functions:**
```rust
// Defined at module level, not in #[test]
fn is_logger_available() -> bool {
    Command::new("which")
        .arg("logger")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

// Used in tests
if !is_logger_available() {
    println!("Skipping test: logger command not available");
    return;
}
```

**Test Debugging:**
- `println!()` used for test output (use `--nocapture` to see)
- Early returns for skipping tests when prerequisites not met

## Test Dependencies

**Dev Dependencies:**
- `tempfile = "3.24"` - Temporary directories for tests
- `http-body-util = "0.1.3"` - HTTP testing utilities
- `tower = "0.5.3"` - Service trait for HTTP testing

## Test Performance

**Patterns:**
- Tests use `#[ignore]` attribute for slow tests (not observed in codebase)
- Integration tests with real journald may be slower
- Consider `--release` flag for faster test runs

---

*Testing analysis: 2026-02-01*
