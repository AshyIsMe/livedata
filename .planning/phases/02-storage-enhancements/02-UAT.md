---
status: testing
phase: 02-storage-enhancements
source: [02-01-SUMMARY.md, 02-02-SUMMARY.md, 02-03-SUMMARY.md, 02-04-SUMMARY.md]
started: 2026-02-06T03:10:00Z
updated: 2026-02-06T03:10:00Z
---

## Current Test

number: 1
name: Configuration via CLI flags
expected: |
  Run `cargo run -- --help` and verify retention flags are listed:
  --log-retention-days, --log-max-size-gb, --process-retention-days, --process-max-size-gb, --cleanup-interval
awaiting: user response

## Tests

### 1. Configuration via CLI flags
expected: Run `cargo run -- --help` and verify retention flags are listed (--log-retention-days, --log-max-size-gb, --process-retention-days, --process-max-size-gb, --cleanup-interval)
result: [pending]

### 2. Default config file creation
expected: Delete ~/.livedata/config.toml if it exists, run the application, then check that ~/.livedata/config.toml was auto-created with retention settings
result: [pending]

### 3. Application starts and collects data
expected: Run `cargo run`, application starts without errors, web server listening on 127.0.0.1:3000, process collection and log collection messages appear in output
result: [pending]

### 4. Navigation header on Log Search page
expected: Open http://127.0.0.1:3000 in browser. A navigation header is visible at the top with "Log Search" (active/highlighted) and "Processes" links. Clicking "Processes" opens the process monitor in a new tab.
result: [pending]

### 5. Navigation header on Processes page
expected: Open http://127.0.0.1:3000/processes.html. Same navigation header with "Log Search" and "Processes" (active/highlighted). Clicking "Log Search" opens the log search in a new tab.
result: [pending]

### 6. Storage health indicator
expected: On either page, below the navigation header, a storage health bar shows: database size (e.g., "0.01GB / 1GB"), log count, metric count, and retention policy (e.g., "30d logs / 7d proc"). Status is color-coded green (under 75% usage).
result: [pending]

### 7. Process data persists to database
expected: Run the application for ~15 seconds, then stop it. Run it again and check /api/storage/health — process_metric_count should be > 0, confirming data survived restart.
result: [pending]

### 8. Storage health API
expected: While app is running, open http://127.0.0.1:3000/api/storage/health in browser. Returns JSON with database_size_bytes, journal_log_count, process_metric_count, and retention_policy object.
result: [pending]

## Summary

total: 8
passed: 0
issues: 0
pending: 8
skipped: 0

## Gaps

[none yet]
