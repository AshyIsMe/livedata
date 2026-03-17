# Four Issues Batch Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement all four open issues: expanded process metrics, guix.scm dev shell, column chooser UI, and historical journal backfill.

**Architecture:** Each issue is independent and touches different parts of the codebase. Process metrics expands `ProcessInfo` + schema migration + UI. Guix.scm is a standalone file. Column chooser adds UI to the log search page using the existing `/api/columns` endpoint. Backfill adds a new CLI option and background thread that reads journal entries backward.

**Tech Stack:** Rust (sysinfo, duckdb, axum, clap, systemd), HTML/JS (HTMX), Guix Scheme

---

## File Structure

### Task 1: Expand Process Metrics (livedata-oud)
- Modify: `src/process_monitor.rs` — add cmdline, rss, vsize, status, parent_pid fields
- Modify: `src/duckdb_buffer.rs` — migration_002 for expanded process_metrics schema + update appender
- Modify: `src/web_server.rs` — update ProcessMetricsRow, HTMX process table, and process API
- Test: `src/process_monitor.rs` (inline tests), `src/duckdb_buffer.rs` (inline tests)

### Task 2: Guix Shell (livedata-1ch)
- Create: `guix.scm` — Guix shell development environment

### Task 3: Column Chooser (livedata-zml)
- Modify: `src/web_server.rs` — add column chooser widget to `search_ui` / `build_search_html`

### Task 4: Historical Journal Backfill (livedata-iiy)
- Modify: `src/main.rs` — add `--max-db-size` CLI arg
- Modify: `src/config.rs` — add `max_db_size_bytes` setting + parse_size helper
- Modify: `src/app_controller.rs` — add backfill logic after forward stream is established
- Modify: `src/journal_reader.rs` — add `previous_entry()` method returning LogEntry
- Test: `src/config.rs` (inline tests for parse_size)

---

### Task 1: Expand Process Metrics (livedata-oud)

The existing `ProcessInfo` captures PID, name, CPU%, memory_bytes, user_id, runtime. The issue asks for: full cmdline, RSS, VSZ, status, parent PID. Network per-process is not reliably available without root/eBPF, so we'll skip it and document that limitation.

**Files:**
- Modify: `src/process_monitor.rs`
- Modify: `src/duckdb_buffer.rs`
- Modify: `src/web_server.rs`

- [ ] **Step 1: Expand ProcessInfo struct**

In `src/process_monitor.rs`, add fields to `ProcessInfo`:

```rust
pub struct ProcessInfo {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub name: String,
    pub cmd: Vec<String>,
    pub cpu_percent: f32,
    pub memory_bytes: u64,
    pub virtual_memory_bytes: u64,
    pub status: String,
    pub user_id: Option<String>,
    pub runtime_secs: u64,
}
```

Update the collection loop in `start_collection` to populate these fields using `sysinfo::Process` methods:
- `process.cmd()` for cmd (full command line args)
- `process.virtual_memory()` for virtual_memory_bytes
- `process.status().to_string()` for status
- `process.parent()` for parent_pid

- [ ] **Step 2: Run tests to verify ProcessInfo still works**

Run: `cargo test test_process_monitor_creation`
Expected: PASS

- [ ] **Step 3: Add migration_002 for expanded process_metrics schema**

In `src/duckdb_buffer.rs`:

1. Change `CURRENT_SCHEMA_VERSION` from 1 to 2
2. Add migration_002 in `run_migrations`:

```rust
if current_version < 2 {
    info!("Applying migration 2: Expand process_metrics columns");
    Self::migration_002(conn)?;
    Self::record_migration(conn, 2, "Expand process_metrics with cmdline, vsize, status, parent_pid")?;
}
```

3. Implement `migration_002`:

```rust
fn migration_002(conn: &Connection) -> Result<()> {
    // Add new columns to existing table (ALTER TABLE ADD COLUMN is safe - NULLs for old rows)
    conn.execute("ALTER TABLE process_metrics ADD COLUMN IF NOT EXISTS cmdline TEXT", [])?;
    conn.execute("ALTER TABLE process_metrics ADD COLUMN IF NOT EXISTS virtual_memory DOUBLE", [])?;
    conn.execute("ALTER TABLE process_metrics ADD COLUMN IF NOT EXISTS status TEXT", [])?;
    conn.execute("ALTER TABLE process_metrics ADD COLUMN IF NOT EXISTS parent_pid INTEGER", [])?;
    info!("Migration 002: Added cmdline, virtual_memory, status, parent_pid to process_metrics");
    Ok(())
}
```

- [ ] **Step 4: Update add_process_metrics appender**

In `src/duckdb_buffer.rs`, update `add_process_metrics` to include the new fields in the appender row:

```rust
let cmd_str = if process.cmd.is_empty() {
    None
} else {
    Some(process.cmd.join(" "))
};

appender.append_row(params![
    timestamp.to_rfc3339(),
    process.pid as i32,
    process.name,
    process.cpu_percent as f64,
    process.memory_bytes as f64,
    user,
    process.runtime_secs as i64,
    cmd_str,
    process.virtual_memory_bytes as f64,
    process.status,
    process.parent_pid.map(|p| p as i32),
])?;
```

- [ ] **Step 5: Update ProcessMetricRecord and retrieval queries**

Update `ProcessMetricRecord` to include the new fields, and update `get_process_metrics_for_timestamp` SELECT to include them.

- [ ] **Step 6: Update web server process API response**

In `src/web_server.rs`, add new fields to `ProcessMetricsRow` and `to_process_row`, and update the HTMX process table HTML in `render_process_chunk_fragment` to show cmdline and virtual_memory columns.

- [ ] **Step 7: Run all tests**

Run: `cargo test`
Expected: PASS

- [ ] **Step 8: Run clippy and fmt**

Run: `cargo fmt && cargo clippy --all-targets -- -D warnings`
Expected: No errors

- [ ] **Step 9: Commit**

```bash
git add src/process_monitor.rs src/duckdb_buffer.rs src/web_server.rs
git commit -m "feat: expand process metrics with cmdline, vsize, status, parent_pid

Adds migration_002 to expand process_metrics table. Collects full
command line, virtual memory, process status, and parent PID.
Updates web UI to display new fields.

Closes livedata-oud"
```

---

### Task 2: Create guix.scm (livedata-1ch)

**Files:**
- Create: `guix.scm`

- [ ] **Step 1: Check what system dependencies livedata needs**

From Cargo.toml: `systemd` crate needs `libsystemd-dev`, `duckdb` bundles its own C++ code. Need Rust toolchain + pkg-config + gcc/g++ + cmake (for DuckDB bundled build).

- [ ] **Step 2: Create guix.scm**

```scheme
(use-modules (guix packages)
             (gnu packages rust)
             (gnu packages pkg-config)
             (gnu packages cmake)
             (gnu packages gcc)
             (gnu packages freedesktop)
             (gnu packages tls)
             (gnu packages python))

(package
  (name "livedata-dev")
  (version "0.0.0")
  (source #f)
  (build-system gnu-build-system)
  (native-inputs
   (list rust
         rust:cargo
         rust:clippy
         rustfmt
         pkg-config
         cmake
         gcc-toolchain
         openssl
         elogind))  ;; provides libsystemd
  (synopsis "Development environment for livedata")
  (description "Guix shell environment for building livedata.")
  (home-page "")
  (license #f))
```

Note: Exact package names may need adjustment for the user's Guix channel. The key deps are: rust toolchain, pkg-config, cmake, gcc, libsystemd-dev equivalent.

- [ ] **Step 3: Verify the guix.scm loads**

Run: `guix shell -D -f guix.scm -- cargo --version` (if Guix is available)
Expected: prints cargo version

- [ ] **Step 4: Commit**

```bash
git add guix.scm
git commit -m "feat: add guix.scm for Guix shell development environment

Closes livedata-1ch"
```

---

### Task 3: Column Chooser UI (livedata-zml)

The `/api/columns` endpoint already exists and returns available columns with types and default status. We need to add a column chooser dropdown/panel to the log search UI.

**Files:**
- Modify: `src/web_server.rs` — the `search_ui` handler / `build_search_html` or equivalent inline HTML

- [ ] **Step 1: Find the search UI HTML generation**

The search UI is served from `search_ui` handler. Need to find whether it's inline HTML in web_server.rs or loads from static/index.html. Based on reading: `search_ui` serves the search page with inline HTML (the `build_search_html` function generates it with HTMX).

- [ ] **Step 2: Add column chooser widget to search HTML**

Add a collapsible "Columns" panel between the search bar and results table. On page load, fetch `/api/columns` and render checkboxes for each column. Default columns start checked. When columns change, update the HTMX request to include `columns=col1,col2,...` parameter.

The widget should:
- Fetch columns from `/api/columns` on page load
- Show checkboxes grouped by category (default columns checked)
- Update search results when selection changes
- Persist selection in localStorage

Implementation: Add JavaScript in the search page that:
1. Fetches `/api/columns`
2. Renders a checkbox list in a `<details>` element
3. On checkbox change, updates a hidden `columns` parameter and re-triggers the HTMX search

- [ ] **Step 3: Run the build**

Run: `cargo build`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/web_server.rs
git commit -m "feat: add column chooser to log search UI

Adds a collapsible column picker that fetches available columns
from /api/columns and lets users toggle column visibility.
Selection persists in localStorage.

Closes livedata-zml"
```

---

### Task 4: Historical Journal Backfill (livedata-iiy)

**Files:**
- Modify: `src/config.rs`
- Modify: `src/main.rs`
- Modify: `src/app_controller.rs`
- Modify: `src/journal_reader.rs`

- [ ] **Step 1: Add parse_size utility to config.rs**

Add a function that parses human-friendly size strings like "5G", "500M", "1T":

```rust
pub fn parse_size(s: &str) -> Result<u64> {
    let s = s.trim();
    let (num_str, multiplier) = if let Some(n) = s.strip_suffix('T') {
        (n, 1_099_511_627_776u64)
    } else if let Some(n) = s.strip_suffix('G') {
        (n, 1_073_741_824u64)
    } else if let Some(n) = s.strip_suffix('M') {
        (n, 1_048_576u64)
    } else if let Some(n) = s.strip_suffix('K') {
        (n, 1_024u64)
    } else {
        (s, 1u64)
    };
    let num: f64 = num_str.parse().context("Invalid size number")?;
    Ok((num * multiplier as f64) as u64)
}
```

- [ ] **Step 2: Add tests for parse_size**

```rust
#[test]
fn test_parse_size() {
    assert_eq!(parse_size("5G").unwrap(), 5 * 1024 * 1024 * 1024);
    assert_eq!(parse_size("500M").unwrap(), 500 * 1024 * 1024);
    assert_eq!(parse_size("1T").unwrap(), 1024 * 1024 * 1024 * 1024);
    assert_eq!(parse_size("1024K").unwrap(), 1024 * 1024);
    assert_eq!(parse_size("1024").unwrap(), 1024);
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test test_parse_size`
Expected: PASS

- [ ] **Step 4: Add max_db_size_bytes to Settings**

In `src/config.rs`, add `max_db_size_bytes: Option<u64>` to `Settings` (default None = no limit beyond existing retention).

- [ ] **Step 5: Add --max-db-size CLI arg**

In `src/main.rs`, add:

```rust
/// Maximum database size (e.g., 5G, 500M). Limits backfill and triggers eviction.
#[arg(long)]
max_db_size: Option<String>,
```

Parse it with `parse_size` and pass to Settings.

- [ ] **Step 6: Add previous_entry to JournalLogReader**

In `src/journal_reader.rs`, add a method that reads the previous journal entry:

```rust
pub fn previous_entry(&mut self) -> Result<Option<LogEntry>> {
    match self.journal.previous_entry() {
        Ok(Some(entry)) => {
            let log_entry = self.convert_journal_entry(&entry)?;
            Ok(Some(log_entry))
        }
        Ok(None) => Ok(None),
        Err(e) => {
            info!("Error reading previous journal entry: {}", e);
            Ok(None)
        }
    }
}
```

- [ ] **Step 7: Add backfill logic to ApplicationController**

In `src/app_controller.rs`, add a method `run_backfill` that:
1. Creates a **second** JournalLogReader (the main one is positioned for forward reading)
2. Seeks to the oldest timestamp already in the DB (or current time if empty)
3. Reads backward using `previous_entry()`
4. Every 1000 entries, checks the DB file size against max_db_size_bytes
5. Stops when: DB size limit reached, or beginning of journal reached

Run this in a background thread after the main loop starts, with lower priority (nice scheduling or just yielding often).

```rust
fn start_backfill(&self, max_db_size_bytes: u64) -> Option<thread::JoinHandle<()>> {
    let buffer = self.buffer.clone();
    let shutdown_signal = self.shutdown_signal.clone();
    let db_path = buffer.lock().unwrap().db_path().to_path_buf();

    Some(thread::spawn(move || {
        let mut reader = match JournalLogReader::new() {
            Ok(r) => r,
            Err(e) => {
                error!("Backfill: failed to open journal: {}", e);
                return;
            }
        };

        // Seek to tail, then walk backward
        if let Err(e) = reader.seek_to_tail() {
            error!("Backfill: failed to seek to tail: {}", e);
            return;
        }

        let mut backfilled = 0u64;
        loop {
            if shutdown_signal.load(Ordering::Relaxed) {
                break;
            }

            match reader.previous_entry() {
                Ok(Some(entry)) => {
                    if let Err(e) = buffer.lock().unwrap().add_entry(&entry) {
                        error!("Backfill: failed to add entry: {}", e);
                        break;
                    }
                    backfilled += 1;

                    // Check size every 1000 entries
                    if backfilled % 1000 == 0 {
                        if let Ok(meta) = std::fs::metadata(&db_path) {
                            if meta.len() >= max_db_size_bytes {
                                info!("Backfill complete: DB size limit reached after {} entries", backfilled);
                                break;
                            }
                        }
                        // Yield to let live ingestion proceed
                        thread::sleep(Duration::from_millis(10));
                    }
                }
                Ok(None) => {
                    info!("Backfill complete: reached beginning of journal after {} entries", backfilled);
                    break;
                }
                Err(e) => {
                    error!("Backfill error: {}", e);
                    break;
                }
            }
        }
    }))
}
```

- [ ] **Step 8: Wire backfill into the run loop**

In `ApplicationController::run()`, after `process_startup_historical_data` (or after `seek_to_tail` in follow mode), start the backfill thread if `max_db_size_bytes` is set. Join the handle during graceful shutdown.

- [ ] **Step 9: Run all tests**

Run: `cargo test`
Expected: PASS

- [ ] **Step 10: Run clippy and fmt**

Run: `cargo fmt && cargo clippy --all-targets -- -D warnings`
Expected: No errors

- [ ] **Step 11: Commit**

```bash
git add src/config.rs src/main.rs src/app_controller.rs src/journal_reader.rs
git commit -m "feat: add historical journal backfill with --max-db-size limit

On startup, spawns a background thread that reads journal entries
backward in time, filling the database up to the configured max size.
Backfill yields to live ingestion and respects shutdown signals.

Closes livedata-iiy"
```
