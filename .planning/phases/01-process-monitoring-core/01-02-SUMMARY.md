---
phase: 01-process-monitoring-core
plan: 02
subsystem: core

tags: [cli, clap, process-monitor, application-controller, rust]

requires:
  - phase: 01-process-monitoring-core
    plan: 01
    provides: "ProcessMonitor module with background collection and /api/processes endpoint"

provides:
  - CLI flag --process-interval for configurable collection interval
  - ApplicationController manages ProcessMonitor lifecycle
  - Arc<ProcessMonitor> shared state pattern for web server access

affects:
  - 01-03-frontend-process-table

tech-stack:
  added: []
  patterns:
    - "ApplicationController as lifecycle manager for subsystems"
    - "Arc<T> for shared ownership between main thread and web server"

key-files:
  created: []
  modified:
    - src/main.rs
    - src/app_controller.rs

key-decisions:
  - "ProcessMonitor lifecycle managed by ApplicationController (not main.rs)"
  - "process_interval passed through CLI with 5-second default"
  - "Tests use #[tokio::test] for async runtime compatibility with ProcessMonitor"

patterns-established:
  - "ApplicationController::new(data_dir, process_interval) - subsystem initialization with config"
  - "get_process_monitor() getter for web server to access shared state"

duration: 4 min
completed: 2026-02-02
---

# Phase 1 Plan 2: CLI Integration and ApplicationController Wiring Summary

**CLI flag --process-interval for configurable process collection, with ApplicationController managing ProcessMonitor lifecycle and providing shared Arc<ProcessMonitor> access to web server.**

## Performance

- **Duration:** 4 min
- **Started:** 2026-02-02T13:26:11Z
- **Completed:** 2026-02-02T13:30:56Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Added --process-interval CLI flag with short flag -p and default value of 5 seconds
- ApplicationController now manages ProcessMonitor lifecycle (creation and collection start)
- ApplicationController::new() signature updated to accept process_interval parameter
- get_process_monitor() getter provides Arc<ProcessMonitor> for web server access
- main.rs simplified - ProcessMonitor no longer created separately, retrieved from app controller
- All 34 tests pass with #[tokio::test] for async compatibility

## Task Commits

Both tasks committed together due to interdependencies:

1. **Tasks 1 & 2: CLI flag and ProcessMonitor integration** - `902fe91` (feat)
   - Task 1: Add CLI flag for process collection interval
   - Task 2: Integrate ProcessMonitor into ApplicationController

## Files Created/Modified

- `src/main.rs` - Added --process-interval CLI flag, updated ApplicationController::new() calls, removed ProcessMonitor creation (now managed by controller)
- `src/app_controller.rs` - Added ProcessMonitor field and Arc import, updated new() signature and implementation with collection start, added get_process_monitor() getter, tests updated to #[tokio::test]

## Decisions Made

- **ProcessMonitor lifecycle centralization:** Moved ProcessMonitor creation and management from main.rs to ApplicationController for better encapsulation
- **CLI flag naming:** Used --process-interval with -p short flag for brevity, default 5 seconds matches Phase 1 requirements
- **Test runtime compatibility:** Changed tests to #[tokio::test] since ProcessMonitor::start_collection() spawns async tasks requiring tokio runtime

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed test compatibility with ProcessMonitor's async collection**

- **Found during:** Task 2 (ApplicationController test execution)
- **Issue:** Tests failed with "there is no reactor running, must be called from the context of a Tokio 1.x runtime" because ProcessMonitor::start_collection() spawns a tokio::spawn() task
- **Fix:** Changed #[test] to #[tokio::test] for both ApplicationController tests
- **Files modified:** src/app_controller.rs (test module)
- **Verification:** All 34 tests pass after fix
- **Committed in:** 902fe91 (Task commit)

---

**Total deviations:** 1 auto-fixed (1 bug fix for test compatibility)
**Impact on plan:** Minimal - test infrastructure adjustment only, no functional changes to production code

## Issues Encountered

None - minor test runtime adjustment handled automatically via deviation rules.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- CLI integration complete - process collection interval configurable at startup
- ApplicationController manages ProcessMonitor lifecycle correctly
- Web server receives Arc<ProcessMonitor> for API endpoint access
- Next: 01-03-PLAN.md - Frontend process table with search and auto-refresh

---
*Phase: 01-process-monitoring-core*
*Completed: 2026-02-02*
