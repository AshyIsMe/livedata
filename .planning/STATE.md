# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-02-01)

**Core value:** Instant system observability without infrastructure overhead — download, run, and understand what's happening
**Current focus:** Phase 2: Storage Enhancements

## Current Position

Phase: 2 of 3 (Storage Enhancements)
Plan: 4 of 4 in current phase
Status: Phase complete
Last activity: 2026-02-06 — Completed 02-04-PLAN.md (UI Navigation and Storage Health)

Progress: [████████░░] 80%

## Performance Metrics

**Velocity:**
- Total plans completed: 8
- Average duration: 5 min
- Total execution time: 0.72 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 1 | 4 | 18 min | 4.5 min |
| 2 | 4 | 24 min | 6 min |

**Recent Trend:**
- Last 5 plans: 5 completed
- Trend: Steady

*Updated after each plan completion*

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

| Phase | Decision | Rationale |
|-------|----------|-----------|
| 01-01 | sysinfo 0.38 API uses bool for refresh_processes | API differs from research, adapted to actual crate version |
| 01-01 | In-memory process data for Phase 1 | Follows plan; DuckDB storage deferred to later phase if needed |
| 01-01 | 5-second refresh interval | Balance between accuracy and system overhead |
| 01-02 | ProcessMonitor lifecycle managed by ApplicationController | Better encapsulation, main.rs simplified |
| 01-02 | Tests use #[tokio::test] for ProcessMonitor compatibility | ProcessMonitor spawns async tasks requiring tokio runtime |
| 01-03 | Tabulator 6.4.1 from CDN for process table | Proven library with sorting/formatting, no build step needed |
| 01-03 | 300ms debounce for search input | Industry standard, balances responsiveness with performance |
| 01-03 | Simple fuzzy matching (chars in order) | Provides fzf-like UX without heavy library dependency |
| 01-03 | 16GB memory assumption for client-side calc | Temporary until system memory API is available |
| 01-04 | Extensive console logging for frontend debugging | Easier diagnosis of rendering issues |
| 01-04 | Extract numeric UID from sysinfo's Uid() format | Cleaner display than raw "Uid(1234)" string |
| 01-04 | Visible error messages instead of console-only | Better user experience when errors occur |
| 02-01 | Multi-source configuration with priority: CLI > Env vars > Config file > Defaults | Maximum flexibility for different deployment scenarios |
| 02-01 | Auto-create default config at ~/.livedata/config.toml | Better UX - users get working config immediately |
| 02-01 | Schema version in _schema_version table with description and timestamp | Enables tracking which migrations applied when for debugging |
| 02-01 | Migration 001 ensures both journal_logs and process_metrics exist | Handles fresh installs and upgrades from pre-migration codebase |
| 02-01 | Default retention: logs 30d/1GB, processes 7d/0.5GB | Conservative defaults balance storage cost with debugging utility |
| 02-02 | Cleanup interval clamped to 5-15 minute range per user decision | Balances responsiveness with system overhead |
| 02-02 | Cleanup cycles are uninterruptible | No cancellation checks during enforce_retention() prevents partially-cleaned databases |
| 02-02 | Background cleanup spawned as tokio::spawn (high-priority async task) | Cleanup is async-friendly, runs at high priority |
| 02-02 | Shutdown signal checked only BETWEEN cleanup cycles | Cleanup must complete once started |
| 02-02 | Database backup created before migrations run (*.duckdb.bak) | Protects against migration failures destroying data |
| 02-02 | Cleanup runs immediately at startup, then periodically | Catches accumulated old data on restart |
| 02-03 | Use mpsc channel to decouple process collection from persistence | Non-blocking try_send with warning on full channel, clean separation of concerns |
| 02-03 | Spawn dedicated receiver task with separate DuckDB connection | Avoids contention on main buffer's connection, independent lifecycle |
| 02-03 | Add CHECKPOINT after COMMIT for data durability | DuckDB WAL may buffer writes until checkpoint, CHECKPOINT forces flush to disk |
| 02-03 | Extract numeric UID from Uid() format for storage | sysinfo returns "Uid(1234)", strip prefix/suffix for clean numeric user ID |
| 02-03 | Pass Settings to web server for retention policy exposure | Storage health API needs retention configuration, added Settings field to AppState |
| 02-04 | Navigation links use target='_blank' to preserve context across views | Users often want to reference both Logs and Processes simultaneously |
| 02-04 | Storage health refreshes every 30 seconds for near-real-time visibility | Storage changes slowly, 30s provides adequate visibility without excessive requests |
| 02-04 | Color coding thresholds: 75% yellow, 90% red for proactive warnings | Provides proactive warnings before storage limits are reached |

### Pending Todos

[From .planning/todos/pending/ — ideas captured during sessions]

None yet.

### Blockers/Concerns

[Issues that affect future work]

None yet.

## Session Continuity

Last session: 2026-02-06 03:04 UTC
Stopped at: Completed 02-04-PLAN.md (UI Navigation and Storage Health)
Resume file: None
