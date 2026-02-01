# Coding Conventions

**Analysis Date:** 2026-02-01

## Naming Patterns

**Files:**
- `snake_case.rs` - All source files use lowercase with underscores

**Functions:**
- `snake_case` - All function names use lowercase with underscores
- Examples: `get_field()`, `process_historical_entries()`, `seek_to_tail()`

**Variables:**
- `snake_case` - All variable names use lowercase with underscores
- Examples: `timestamp`, `minute_key`, `fields`, `processed_count`

**Types:**
- `PascalCase` - All struct and type names use uppercase for each word
- Examples: `LogEntry`, `ApplicationController`, `DuckDBBuffer`, `JournalLogReader`

**Constants:**
- Not extensively used in codebase; follow Rust convention `SCREAMING_SNAKE_CASE`

## Code Style

**Formatting:**
- Tool: `rustfmt` (standard Rust formatter)
- Edition: 2024
- Key settings:
  - Max line length: 100 characters (rustfmt default)
  - Indentation: 4 spaces (no tabs)
  - Trailing commas in multi-line arrays/structs
- No custom `.rustfmt.toml` - using default configuration

**Linting:**
- Tool: `clippy`
- Version: 0.1.93
- No custom `clippy.toml` - using default configuration
- Run: `cargo clippy`

## Import Organization

**Order:**
1. Crate imports (`crate::module::Type`)
2. External crate imports (e.g., `anyhow::Result`, `chrono::Utc`)
3. Standard library imports (e.g., `std::collections::HashMap`)

**Within each group:**
- Imports are sorted alphabetically
- Grouped by crate: `use std::sync::{Arc, Mutex};`
- No wildcard imports (`use *`) observed

**Path Aliases:**
- None used - always fully qualified paths

## Error Handling

**Patterns:**
- Primary error type: `anyhow::Result<T>` for fallible operations
- Error propagation: `?` operator used consistently
- Error conversion: `.map_err(|e| anyhow!("Message: {}", e))?` pattern
- Custom errors: `anyhow!` macro for creating ad-hoc errors
- Optional values: `Option<T>` used for values that may or may not exist

**Error message format:**
- Descriptive context: `"Failed to open journal: {}"`
- Specific operation identified in message

## Logging

**Framework:**
- Primary: `log` crate with macros: `info!()`, `error!()`, `warn!()`, `debug!()`
- Application entry point: `tracing` crate with `tracing-subscriber`
- Initialization in `src/main.rs` using `tracing_subscriber::fmt::layer()`

**Patterns:**
- Info logging for lifecycle events: `info!("Starting journald log collection to DuckDB")`
- Error logging with context: `error!("Failed to process log entry: {}", e)`
- Debug logging for detailed operations: `debug!("Processing historical entries")`
- Structured logging not used - simple string messages only

## Comments

**When to Comment:**
- Complex operations (e.g., seeking journal, transaction management)
- Section divisions (e.g., "// User journal fields", "// Trusted journal fields")
- Implementation details that aren't obvious from code

**JSDoc/TSDoc:**
- Rust doc comments (`///`) used for:
  - Public structs and types: `/// Application state shared across handlers`
  - Struct fields in derive macros: `/// Text search (MESSAGE field, case-insensitive ILIKE)`
  - Module-level documentation in `main.rs`
- Not extensive - most public APIs lack doc comments

## Function Design

**Size:**
- No strict guidelines observed
- Functions range from small (10-20 lines) to larger (100+ lines)
- Larger functions tend to have clear sub-sections marked by comments

**Parameters:**
- Generic parameters used for flexibility: `P: AsRef<std::path::Path>`
- References preferred: `&self`, `&LogEntry` to avoid clones
- Builder pattern not used - direct constructors with multiple parameters

**Return Values:**
- Fallible operations: `Result<T>` using anyhow
- Optional values: `Option<T>`
- Simple values: Direct return types (e.g., `&Path`, `String`)

## Module Design

**Exports:**
- Public APIs exported via `pub mod` declarations in `src/lib.rs`
- Barrel file pattern: `src/lib.rs` exports all modules: `app_controller`, `duckdb_buffer`, `journal_reader`, `log_entry`, `web_server`

**Barrel Files:**
- `src/lib.rs` serves as main barrel file
- Re-exports module-level types (e.g., `pub mod app_controller;`)

**Visibility:**
- `pub` used for public APIs
- Private implementation details not marked (default private)
- Test modules marked with `#[cfg(test)]`

## Additional Patterns

**Serialization:**
- `serde::{Serialize, Deserialize}` derives for structs
- JSON serialization via `serde_json`
- Timestamp fields use `serde` feature from chrono

**Async:**
- `tokio` runtime used for async operations
- `#[tokio::test]` for async test functions
- Async handlers in web server: `async fn health(...) -> impl IntoResponse`

**Concurrency:**
- `std::sync::Arc` for shared ownership
- `std::sync::Mutex` for mutable shared state
- `std::sync::atomic::{AtomicBool, Ordering}` for atomic flags

---

*Convention analysis: 2026-02-01*
