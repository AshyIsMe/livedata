# Codebase Concerns

**Analysis Date:** 2026-02-01

## Tech Debt

**Large Monolithic File - `src/web_server.rs` (1952 lines):**
- Issue: Single file contains API handlers, HTML generation, SQL building, and business logic
- Files: `src/web_server.rs`
- Impact: Difficult to maintain, test, and understand. Changes in one area risk breaking unrelated functionality
- Fix approach: Split into multiple modules: handlers (search, filters, columns), templates (HTML/CSS/JS), and query builder

**Missing Connection Pooling:**
- Issue: Uses `Mutex<Connection>` for all database access despite `r2d2` feature enabled
- Files: `src/web_server.rs:18-28`, `src/app_controller.rs:16-17`
- Impact: All database operations serialized, causing lock contention under load
- Fix approach: Implement actual connection pool using r2d2 for DuckDB connections

**Hardcoded Historical Processing Limit:**
- Issue: `process_historical_entries` only checks 10,000 entries regardless of time window
- Files: `src/journal_reader.rs:220-224`
- Impact: Systems with high log rates (>10k entries/hour) will miss historical data on startup
- Fix approach: Remove fixed limit or make it configurable based on log rate

**No Data Retention/Cleanup Strategy:**
- Issue: Database grows indefinitely without cleanup mechanism
- Files: `src/duckdb_buffer.rs` (no retention logic)
- Impact: Database already 2.5GB and will continue growing, consuming disk space
- Fix approach: Implement configurable retention policy with scheduled cleanup

**Excessive String Cloning:**
- Issue: `add_entry` in `duckdb_buffer.rs` clones dozens of String fields per entry
- Files: `src/duckdb_buffer.rs:161-264`
- Impact: Unnecessary memory allocation overhead during high-volume log ingestion
- Fix approach: Use references where possible or bulk insert patterns

**Missing Transaction Management:**
- Issue: `add_entry` executes inserts immediately without transaction batching
- Files: `src/duckdb_buffer.rs:155-496`
- Impact: Poor write performance due to per-row commit overhead (only historical data uses transactions)
- Fix approach: Implement transaction batching for real-time entries with configurable flush interval

## Known Bugs

**Follow Mode Cursor Positioning:**
- Symptoms: After `seek_to_tail()`, cursor is positioned past the last entry, requiring `previous_skip(1)` workaround
- Files: `src/app_controller.rs:75-77`, `src/journal_reader.rs:30-36`
- Trigger: Running with `--follow` flag
- Workaround: The `previous_skip(1)` call is already in place, but it's fragile and relies on specific journald behavior
- Workaround location: `src/app_controller.rs:77`

**Timestamp Parsing Inconsistency:**
- Symptoms: DuckDB stores timestamps as strings, requiring manual parsing on retrieval
- Files: `src/duckdb_buffer.rs:533-540`, `src/duckdb_buffer.rs:728-735`
- Trigger: Any query that retrieves timestamps
- Workaround: Manual string parsing in place, but fragile and error-prone

## Security Considerations

**No Authentication on Web Interface:**
- Risk: Anyone who can reach port 3000 can query all system logs
- Files: `src/web_server.rs:220-250`, `src/main.rs:54-72`
- Current mitigation: Server only binds to 127.0.0.1, but still accessible to local users
- Recommendations: Add authentication layer (JWT, API keys, or OAuth), implement RBAC for log access

**SQL Injection via String Interpolation:**
- Risk: While basic escaping exists, user input is concatenated into SQL queries
- Files: `src/web_server.rs:370-432`, `src/web_server.rs:583-652`
- Current mitigation: `escape_like()` for text search, basic quote escaping for filters
- Recommendations: Use parameterized queries throughout, or switch to a query builder library

**No Request Rate Limiting:**
- Risk: Single client can issue unlimited expensive queries, potentially DoS the service
- Files: `src/web_server.rs:323-493` (API search endpoint)
- Current mitigation: None
- Recommendations: Implement rate limiting middleware (e.g., governor crate)

**Insufficient Memory Synchronization:**
- Risk: `AtomicBool::load/store` uses `Ordering::Relaxed` without proper memory barriers
- Files: `src/main.rs:59`, `src/app_controller.rs:43,54`
- Current mitigation: None
- Recommendations: Use `Ordering::SeqCst` or `Ordering::Acquire/Release` for shutdown signal

**Unvalidated JSON in extra_fields:**
- Risk: JSON field `extra_fields` is blindly serialized from user-provided journal data
- Files: `src/duckdb_buffer.rs:363-374`
- Current mitigation: `serde_json::to_string` handles basic serialization
- Recommendations: Validate JSON structure and size limits before storing

## Performance Bottlenecks

**Mutex Contention on Database Access:**
- Problem: All web requests share a single database connection via `Mutex<Connection>`
- Files: `src/web_server.rs:306,337,499,546`
- Cause: No connection pooling
- Improvement path: Implement r2d2 connection pool with configurable max connections

**No Query Result Pagination:**
- Problem: While `limit` parameter exists, entire result set is loaded into memory before returning
- Files: `src/web_server.rs:323-493`
- Cause: DuckDB query executes completely, not using cursors or streaming
- Improvement path: Implement true server-side cursors or streaming result sets

**Unoptimized Histogram Queries:**
- Problem: Histogram queries run without time range indexes, scanning full table
- Files: `src/web_server.rs:686-748`
- Cause: No composite index on `(timestamp, minute_key)` for histogram aggregations
- Improvement path: Add composite index for histogram queries or pre-aggregate materialized views

**Per-Entry Database Writes:**
- Problem: Each log entry triggers immediate INSERT without batching
- Files: `src/duckdb_buffer.rs:376-494`
- Cause: `add_entry` executes INSERT immediately
- Improvement path: Batch entries into transactions, flush every N entries or M seconds

**Large HTML Responses:**
- Problem: HTML response includes inline JavaScript (~400 lines) and CSS (~250 lines)
- Files: `src/web_server.rs:939-1727` (entire HTML template)
- Cause: Server-side rendering with embedded assets
- Improvement path: Serve static CSS/JS files separately, or use client-side rendering

## Fragile Areas

**Historical Data Startup Logic:**
- Files: `src/app_controller.rs:121-159`, `src/journal_reader.rs:200-257`
- Why fragile: Depends on accurate time estimation and journald cursor behavior
- Safe modification: Always process with time-based query instead of entry count limit
- Test coverage: Only integration tests with real journal, limited test scenarios

**SQL Query Building:**
- Files: `src/web_server.rs:370-432`, `src/web_server.rs:583-652`
- Why fragile: String concatenation for SQL, any typo breaks queries
- Safe modification: Use a query builder library (e.g., sqlx or diesel)
- Test coverage: Basic tests exist, but edge cases not covered

**Field Extraction in DuckDB Buffer:**
- Files: `src/web_server.rs:436-480`, `src/web_server.rs:662-683` (row mapping)
- Why fragile: Manual index-based row extraction, any schema change breaks mapping
- Safe modification: Use ORM or query builder with compile-time checks
- Test coverage: Tests check happy path, not error handling for type mismatches

**Time Parsing Logic:**
- Files: `src/web_server.rs:157-196`, `src/journal_reader.rs:158-173`
- Why fragile: Multiple time formats (ISO 8601, relative, journald timestamps), parsing can fail
- Safe modification: Consolidate to single time representation, validate at boundaries
- Test coverage: Some tests, but not all format combinations covered

## Scaling Limits

**Database File Size:**
- Current capacity: 2.5GB with unknown retention period
- Limit: DuckDB performance degrades as file grows; no built-in rotation
- Scaling path: Implement partitioning (by day/month) or migrate to distributed DB (ClickHouse)

**Single-Process Architecture:**
- Current capacity: One process collects and serves logs
- Limit: Single point of failure, cannot scale horizontally
- Scaling path: Separate ingestion (collector) from serving (query API), enable clustering

**Memory Usage Per Request:**
- Current capacity: Up to 100,000 results loaded into memory
- Limit: Large queries can exhaust RAM under concurrent load
- Scaling path: Stream results instead of materializing, implement memory limits

**Journal Reading Rate:**
- Current capacity: Limited by single-threaded journald reading
- Limit: Cannot keep up with high-volume logging (>10k entries/sec)
- Scaling path: Multi-threaded journal reading or separate collector processes

## Dependencies at Risk

**DuckDB (1.4.3) with Bundled Feature:**
- Risk: Bundled feature uses static linking, may have compatibility issues on some platforms
- Impact: Build failures or runtime crashes on certain Linux distributions
- Migration plan: Test on target platforms, consider system-provided DuckDB if available

**Axum (0.8.8):**
- Risk: Rapidly evolving web framework, breaking changes possible
- Impact: Future upgrades may require significant refactoring
- Migration plan: Pin to minor version until stable API, monitor release notes

**Systemd crate (0.10):**
- Risk: Direct journald binding, sensitive to systemd version changes
- Impact: May not work with older or newer systemd versions
- Migration plan: Add systemd version detection, fallback to log file reading

## Missing Critical Features

**No Log Rotation or Retention:**
- Problem: Database grows without bound
- Blocks: Long-term deployment without manual intervention
- Risk: Disk exhaustion, system crash

**No Query Performance Monitoring:**
- Problem: Cannot identify slow queries or database bottlenecks
- Blocks: Performance optimization
- Risk: Degraded user experience without visibility

**No Configuration Management:**
- Problem: Settings hardcoded (e.g., `entries_to_check = 10000`, default limits)
- Blocks: Tuning for different environments
- Risk: Suboptimal performance in production

**No Health/Readiness Probes:**
- Problem: Only basic `/health` endpoint returns 200, no actual checks
- Blocks: Kubernetes deployment and orchestration
- Risk: Dead process considered healthy

**No Backup/Export Functionality:**
- Problem: No way to export or backup log data
- Blocks: Compliance and data retention requirements
- Risk: Data loss without recovery mechanism

## Test Coverage Gaps

**Concurrency Testing:**
- What's not tested: Multiple concurrent web requests, mutex contention scenarios
- Files: `src/web_server.rs` (no concurrent tests)
- Risk: Race conditions and deadlocks under load
- Priority: High

**Error Path Testing:**
- What's not tested: Database connection failures, journald read errors, malformed timestamps
- Files: All source files use `unwrap()` in critical paths
- Risk: Unhandled errors cause crashes in production
- Priority: High

**SQL Injection Testing:**
- What's not tested: Malicious query parameters, SQL special characters
- Files: `src/web_server.rs` query building
- Risk: Undiscovered SQL injection vulnerabilities
- Priority: High

**Large Dataset Performance:**
- What's not tested: Queries against 100k+ row datasets, histogram performance
- Files: Integration tests use small datasets
- Risk: Performance issues discovered only in production
- Priority: Medium

**Historical Data Edge Cases:**
- What's not tested: Gaps in journal data, clock skew, timezone changes
- Files: `src/journal_reader.rs:200-257`
- Risk: Missing or duplicated historical entries
- Priority: Medium

**Web Server Shutdown:**
- What's not tested: Graceful shutdown with in-flight requests
- Files: `src/web_server.rs:236-249`
- Risk: Data loss or client errors during restart
- Priority: Low

---

*Concerns audit: 2026-02-01*
