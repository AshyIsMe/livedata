# Architecture

**Analysis Date:** 2026-02-01

## Pattern Overview

**Overall:** Streaming Pipeline with Layered Architecture

**Key Characteristics:**
- Single-threaded log collection loop with concurrent web server
- On-disk storage via DuckDB for persistence
- Event-driven architecture using systemd journal notifications
- Graceful shutdown via signal handling

## Layers

**Entry Layer:**
- Purpose: CLI argument parsing, application initialization, thread spawning
- Location: `src/main.rs`
- Contains: Argument parsing, logging setup, web server thread spawning
- Depends on: `app_controller`, `web_server`
- Used by: Operating system (as binary entry point)

**Orchestration Layer:**
- Purpose: Application lifecycle management, coordination of log collection
- Location: `src/app_controller.rs`
- Contains: Main collection loop, signal handling, startup historical data processing
- Depends on: `journal_reader`, `duckdb_buffer`, signal handling primitives
- Used by: `main.rs`

**Data Ingestion Layer:**
- Purpose: Reading log entries from systemd journald
- Location: `src/journal_reader.rs`
- Contains: Journal connection, entry iteration, historical data backfill
- Depends on: `systemd` crate, `log_entry`
- Used by: `app_controller`

**Data Model Layer:**
- Purpose: Representation of log entries with field accessors
- Location: `src/log_entry.rs`
- Contains: `LogEntry` struct, field getters, minute key generation
- Depends on: `chrono`, `serde`
- Used by: `journal_reader`, `duckdb_buffer`, `app_controller`

**Storage Layer:**
- Purpose: Persistent storage and retrieval of log entries
- Location: `src/duckdb_buffer.rs`
- Contains: DuckDB connection, schema initialization, CRUD operations
- Depends on: `duckdb`, `log_entry`, `serde_json`
- Used by: `app_controller`

**Query/API Layer:**
- Purpose: HTTP API and web UI for log search
- Location: `src/web_server.rs`
- Contains: Axum router, SQL query building, HTML UI rendering
- Depends on: `axum`, `duckdb`, `chrono`
- Used by: `main.rs` (spawned in separate thread)

## Data Flow

**Log Collection Flow:**

1. `main.rs` parses CLI args and creates `ApplicationController`
2. `ApplicationController::run()` starts signal handler thread
3. If not in follow mode, process historical entries from last hour:
   - Seek to tail of journal
   - Walk backwards to find entries within time window
   - Insert each entry in a single transaction
4. Position cursor at most recent entry (tail - 1)
5. Main loop:
   - Wait for journal entries with 100ms timeout
   - Read and process new entries
   - Insert into DuckDB
   - Log status every 30 seconds
   - Check shutdown signal

**Query Flow:**

1. HTTP request received by Axum router (`web_server.rs`)
2. Parse query parameters (time range, filters, columns)
3. Build dynamic SQL query against `journal_logs` table
4. Execute query via DuckDB connection
5. Serialize results as JSON or render HTML
6. Return response

**State Management:**
- Log collection: Single-threaded in main thread
- Web server: Separate Tokio runtime with shared `AppState`
- Shutdown: `Arc<AtomicBool>` shared signal between collector and web server

## Key Abstractions

**LogEntry:**
- Purpose: Represents a single journal log entry with metadata
- Examples: `src/log_entry.rs`
- Pattern: Domain model with convenience getters for all systemd journal fields

**DuckDBBuffer:**
- Purpose: Abstracts DuckDB storage operations
- Examples: `src/duckdb_buffer.rs`
- Pattern: Repository pattern with connection management, schema initialization

**JournalLogReader:**
- Purpose: Wraps systemd journal API for Rust iteration
- Examples: `src/journal_reader.rs`
- Pattern: Iterator adapter with seeking and waiting capabilities

**AppState:**
- Purpose: Shared state for web server handlers
- Examples: `src/web_server.rs` (struct AppState)
- Pattern: Thread-safe container with Mutex-protected DuckDB connection

## Entry Points

**Binary Entry:**
- Location: `src/main.rs::main()`
- Triggers: Command line execution
- Responsibilities: Parse args, initialize logging, spawn web server, run app controller

**Web Server Entry:**
- Location: `src/web_server.rs::run_web_server()`
- Triggers: Called from main thread when `--web` flag used
- Responsibilities: Start Axum server on port 3000, handle graceful shutdown

## Error Handling

**Strategy:** Result<T, E> propagation with anyhow for flexible error handling

**Patterns:**
- Use `anyhow::Result<T>` for fallible operations
- `?` operator for error propagation
- Log errors but continue processing where possible (e.g., individual log entry failures)
- Transaction rollback on batch failures (historical data processing)

## Cross-Cutting Concerns

**Logging:** `tracing` crate with subscriber, structured logging at info/error/warn levels

**Validation:** Type-safe field parsing with Option returns for missing fields

**Authentication:** Not implemented (localhost-only binding: `127.0.0.1:3000`)

---

*Architecture analysis: 2026-02-01*
