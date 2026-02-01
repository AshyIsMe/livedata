# Codebase Structure

**Analysis Date:** 2026-02-01

## Directory Layout

```
livedata/
├── src/                    # Source code (flat module structure)
│   ├── main.rs            # Binary entry point
│   ├── lib.rs             # Library module declarations
│   ├── app_controller.rs  # Application orchestration
│   ├── journal_reader.rs  # Systemd journald interface
│   ├── duckdb_buffer.rs   # DuckDB storage layer
│   ├── log_entry.rs       # Log entry data model
│   └── web_server.rs      # HTTP API and web UI
├── tests/                 # Integration tests
│   ├── direct_journal_test.rs
│   ├── debug_journal_test.rs
│   ├── journal_integration_test.rs
│   └── simple_journal_test.rs
├── data/                  # Runtime data directory (created at runtime)
│   └── livedata.duckdb    # On-disk database (created by app)
├── .planning/             # Planning documents
│   └── codebase/          # Codebase analysis docs
├── Cargo.toml             # Project manifest and dependencies
├── Cargo.lock             # Dependency lockfile
├── AGENTS.md              # Development agent guidelines
├── README.md              # Project documentation
└── LICENSE                # License file
```

## Directory Purposes

**src/:**
- Purpose: All Rust source code for the application
- Contains: Application logic, data models, HTTP handlers
- Key files: `main.rs`, `app_controller.rs`, `journal_reader.rs`, `duckdb_buffer.rs`, `log_entry.rs`, `web_server.rs`

**tests/:**
- Purpose: Integration tests that interact with real journald
- Contains: Test utilities for journal access and debugging
- Key files: Integration test files for journal reading

**data/:**
- Purpose: Runtime data storage directory
- Contains: DuckDB database file created at runtime
- Key files: `livedata.duckdb` (created on first run)

**.planning/codebase/:**
- Purpose: Codebase analysis documents for GSD planning
- Contains: Architecture and structure documentation
- Key files: `ARCHITECTURE.md`, `STRUCTURE.md`

## Key File Locations

**Entry Points:**
- `src/main.rs`: Binary entry point, CLI parsing, app initialization

**Configuration:**
- `Cargo.toml`: Dependency declarations, build configuration, optimization profiles

**Core Logic:**
- `src/app_controller.rs`: Main orchestration, collection loop, lifecycle management
- `src/journal_reader.rs`: Journald reading, historical data backfill
- `src/duckdb_buffer.rs`: Database schema, CRUD operations, indexing
- `src/log_entry.rs`: Data model, field accessors, minute key generation
- `src/web_server.rs`: HTTP API, SQL query building, HTML UI rendering

**Testing:**
- `tests/`: Integration tests for journal reading
- Unit tests: Inline in each source file using `#[cfg(test)]`

## Naming Conventions

**Files:**
- Pattern: `snake_case.rs` for all source files
- Example: `app_controller.rs`, `journal_reader.rs`, `duckdb_buffer.rs`

**Directories:**
- Pattern: `snake_case` for all directories
- Example: `src/`, `tests/`, `data/`

**Modules:**
- Pattern: `snake_case` matching filename without extension
- Example: `mod app_controller;` in `lib.rs`

**Structs:**
- Pattern: `PascalCase`
- Example: `ApplicationController`, `JournalLogReader`, `DuckDBBuffer`, `LogEntry`

**Functions:**
- Pattern: `snake_case`
- Example: `new()`, `run()`, `add_entry()`, `get_status()`

**Constants:**
- Pattern: `SCREAMING_SNAKE_CASE`
- Example: `DEFAULT_COLUMNS`, `EXCLUDED_COLUMNS`

## Where to Add New Code

**New Feature:**
- Primary code: Add to appropriate existing module in `src/` or create new module
- Tests: Add inline tests with `#[cfg(test)]` in the same file

**New Log Source (e.g., Windows Event Log):**
- Create new reader: `src/event_log_reader.rs` (example pattern)
- Declare module: Add `pub mod event_log_reader;` to `src/lib.rs`
- Integrate with controller: Add reader field to `ApplicationController`

**New Web API Endpoint:**
- Add handler: `src/web_server.rs` in appropriate section (api handlers vs UI handlers)
- Register route: Add `.route("/api/new-endpoint", get(new_handler))` to Router
- Define structs: Add request/response structs near top of `web_server.rs`

**New Database Table/Schema:**
- Modify schema: Update table creation SQL in `DuckDBBuffer::new()` in `src/duckdb_buffer.rs`
- Add methods: Add CRUD methods to `DuckDBBuffer` impl block
- Update tests: Add test cases in `#[cfg(test)]` section

**Utilities:**
- Shared helpers: Add to appropriate existing module (e.g., time parsing in `web_server.rs`)
- If cross-cutting: Create `src/utils.rs` and declare in `lib.rs`

## Special Directories

**data/:**
- Purpose: Runtime data storage (DuckDB database)
- Generated: Yes (created by application)
- Committed: No (in `.gitignore`)

**target/:**
- Purpose: Cargo build artifacts
- Generated: Yes
- Committed: No

**.planning/:**
- Purpose: GSD planning documents
- Generated: Yes (by GSD agents)
- Committed: Yes

**.beads/ (linked):**
- Purpose: Issue tracking (bd/beads)
- Generated: Yes
- Committed: Yes

---

*Structure analysis: 2026-02-01*
