---
phase: 01-process-monitoring-core
plan: 01
subsystem: api
tags: [sysinfo, process-monitoring, axum, tokio, rust]

requires:
  - phase: existing-log-collection
    provides: "DuckDB storage, web server infrastructure, ApplicationController pattern"

provides:
  - ProcessMonitor struct with background collection
  - ProcessInfo struct with PID, name, CPU%, memory, user, runtime
  - GET /api/processes endpoint returning JSON snapshot
  - sysinfo integration for system process data

affects:
  - 01-02-CLI-integration
  - 01-03-frontend-process-table

tech-stack:
  added:
    - sysinfo@0.38
    - fuzzy-matcher@0.3.7
  patterns:
    - "Arc<Mutex<T>> for shared state between threads"
    - "tokio::time::interval for periodic background tasks"
    - "Background collection + cached snapshot pattern"

key-files:
  created:
    - src/process_monitor.rs
  modified:
    - src/lib.rs
    - src/web_server.rs
    - src/main.rs

key-decisions:
  - "Use sysinfo 0.38 refresh_processes with bool parameter (not ProcessRefreshKind) - API changed from research"
  - "5-second collection interval balances accuracy vs overhead"
  - "In-memory only for Phase 1 (no DuckDB storage yet)"
  - "ProcessMonitor::Default trait required by clippy for idiomatic Rust"

patterns-established:
  - "Background task pattern: spawn tokio task with interval, update Arc<Mutex<snapshot>>"
  - "API handler pattern: extract State, call monitor.get_snapshot(), return Json<Response>"
  - "Error handling: Result<Json<T>, (StatusCode, String)> for consistent API errors"

duration: 8 min
completed: 2026-02-02
---

# Phase 1 Plan 1: Backend Process Collection Summary

**Process monitoring backend with sysinfo integration, background collection task, and GET /api/processes endpoint serving real-time process snapshots.**

## Performance

- **Duration:** 8 min
- **Started:** 2026-02-02T13:13:45Z
- **Completed:** 2026-02-02T13:21:36Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments

- ProcessMonitor module collecting system process data in background using sysinfo crate
- ProcessInfo struct with all required fields (PID, name, CPU%, memory_bytes, user_id, runtime_secs)
- Background collection task refreshing every 5 seconds with proper System state management
- GET /api/processes API endpoint returning JSON with process list, timestamp, and total count
- Integration with existing web server infrastructure via AppState

## Task Commits

Each task was committed atomically:

1. **Task 1: Create ProcessMonitor module** - `11c0578` (feat)
2. **Task 2: Add /api/processes endpoint** - `0dfbae6` (feat)
3. **Formatting** - `0a6cb19` (style)

**Plan metadata:** docs commit to follow

## Files Created/Modified

- `src/process_monitor.rs` - ProcessMonitor and ProcessInfo implementations with background collection
- `src/lib.rs` - Added `pub mod process_monitor` export
- `src/web_server.rs` - Added /api/processes endpoint, ProcessResponse struct, AppState updated
- `src/main.rs` - Created ProcessMonitor, started collection, passed to web server

## Decisions Made

- **sysinfo API adjustment:** Used `refresh_processes(ProcessesToUpdate::All, true)` instead of `ProcessRefreshKind::everything()` - the API changed between research and implementation. The bool parameter controls CPU usage refresh.
- **Default trait:** Added `impl Default for ProcessMonitor` as required by clippy for idiomatic Rust code (new_without_default lint).
- **No DuckDB storage (Phase 1):** Following the plan, process data is in-memory only. Historical storage can be added later if needed.

## Deviations from Plan

None - plan executed exactly as written. Minor API adjustment for sysinfo 0.38 (bool parameter instead of ProcessRefreshKind), which is a normal dependency version difference.

## Issues Encountered

None significant. The sysinfo crate API differed slightly from the research documentation (bool vs ProcessRefreshKind), but this is expected with actively maintained dependencies. Fix was straightforward.

## User Setup Required

None - no external service configuration required. Process monitoring uses local system calls via sysinfo.

## Next Phase Readiness

- Backend process collection is complete and functional
- API endpoint is ready for frontend integration
- Next: 01-02-CLI-integration for ApplicationController wiring
- Next: 01-03-frontend-process-table for web UI

---
*Phase: 01-process-monitoring-core*
*Completed: 2026-02-02*
