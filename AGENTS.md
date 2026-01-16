# AGENTS.md

This file contains guidelines and commands for agentic coding agents working in the livedata repository.

## Build Commands

**Build the project:**
```bash
cargo build
```

**Build for release:**
```bash
cargo build --release
```

**Run the application:**
```bash
cargo run
```

**Check build without compiling:**
```bash
cargo check
```

## Test Commands

**Run all tests:**
```bash
cargo test
```

**Run a single test:**
```bash
cargo test <test_name>
```

**Run tests with specific output:**
```bash
cargo test -- --nocapture
```

**Run tests in release mode:**
```bash
cargo test --release
```

## Linting and Formatting

**Format code (use rustfmt):**
```bash
cargo fmt
```

**Check code formatting without changes:**
```bash
cargo fmt --check
```

**Run Clippy lints:**
```bash
cargo clippy
```

**Run Clippy with all targets and strict checks:**
```bash
cargo clippy --all-targets -- -D warnings
```

**Auto-fix Clippy suggestions:**
```bash
cargo clippy --fix
```

## Code Style Guidelines

### Imports and Dependencies
- Use `cargo add <crate>` to add new dependencies
- Group imports logically: std lib, external crates, local modules
- Prefer explicit imports over `use *` statements
- Keep imports sorted alphabetically within each group

### Formatting
- Use `rustfmt` for consistent code formatting
- Maximum line length: 100 characters (rustfmt default)
- Use 4 spaces for indentation (no tabs)
- trailing commas in multi-line arrays/structs

### Naming Conventions
- Functions and variables: `snake_case`
- Types and structs: `PascalCase`
- Constants: `SCREAMING_SNAKE_CASE`
- File names: `snake_case.rs`
- Modules: `snake_case`

### Error Handling
- Use `Result<T, E>` for fallible operations
- Prefer the `?` operator for error propagation
- Create custom error types using `thiserror` when needed
- Use `Option<T>` for values that may or may not exist

### Type System
- Use type annotations sparingly - let Rust infer when possible
- Add type hints when it improves clarity
- Use strong types over primitive types when domain matters
- Prefer `&str` over `String` for function parameters when ownership isn't needed

### Performance and Memory
- Prefer stack allocation over heap allocation
- Use references (`&`) and borrowing to avoid unnecessary clones
- Consider `Cow<str>` for conditional string ownership
- Use `Vec::with_capacity()` when size is known

### Module Structure
- Keep modules focused on single responsibilities
- Use `mod.rs` for module directories
- Export public APIs at crate root
- Keep implementation details private

### Testing
- Write unit tests in the same file using `#[cfg(test)]`
- Write integration tests in `tests/` directory
- Use descriptive test names that explain what they test
- Use `assert_eq!` for equality checks, `assert!` for boolean conditions

### Documentation
- Document public APIs with `///` doc comments
- Use markdown formatting in documentation
- Include examples in doc comments where helpful
- Keep documentation up-to-date with code changes

## Project Specific Notes

This is a Rust project for live data streaming and analysis with the following characteristics:
- Single binary application targeting system monitoring
- Uses parquet for data storage
- DuckDB for querying
- Web-based search interface inspired by Splunk

When working on this codebase:
- Focus on performance and memory efficiency
- Consider streaming data processing patterns
- Implement proper error handling for system resource access
- Design for concurrent operations where beneficial