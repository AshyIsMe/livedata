---
phase: 02-storage-enhancements
plan: 01
subsystem: database
tags: [duckdb, configuration, schema-migration, toml, retention-settings]

# Dependency graph
requires:
  - phase: 01-process-monitoring-core
    provides: Basic DuckDB integration and data directory setup
provides:
  - Configuration system with multi-source settings (CLI, env vars, TOML)
  - Database schema versioning with migration runner
  - Process metrics table structure for storing process data
affects: [02-02, 02-03, 02-04, process-data-collection, retention-policies]

# Tech tracking
tech-stack:
  added: [toml 0.9.11]
  patterns: [schema-migration-pattern, layered-configuration]

key-files:
  created: [src/config.rs]
  modified: [src/duckdb_buffer.rs, src/main.rs, src/lib.rs, Cargo.toml]

key-decisions:
  - "Multi-source configuration with priority: CLI > Env vars > Config file > Defaults"
  - "Auto-create default config at ~/.livedata/config.toml on first run"
  - "_schema_version table tracks applied migrations with timestamp and description"
  - "Migration 001 creates process_metrics table and ensures journal_logs exists"

patterns-established:
  - "Schema migration pattern: version tracking, idempotent migrations, automatic application"
  - "Configuration layering: defaults → file → environment → CLI with explicit precedence"

# Metrics
duration: 6min
completed: 2026-02-05
---

# Phase 2 Plan 1: Configuration and Schema Summary

**Multi-source configuration system with TOML support, database schema versioning via migration runner, and process_metrics table for time-series process data**

## Performance

- **Duration:** 6 min
- **Started:** 2026-02-05T21:52:20Z
- **Completed:** 2026-02-05T21:57:58Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- Configuration system supporting CLI flags, environment variables, and TOML config files with proper precedence
- Database schema versioning with _schema_version table tracking applied migrations
- Process metrics table created with timestamp, pid, name, cpu_usage, mem_usage, user, runtime columns
- Automatic migration runner applies pending migrations on database initialization
- Default config file auto-created at ~/.livedata/config.toml

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement Configuration System** - `e74b3d3` (feat)
2. **Task 2: Implement Schema Evolution and Process Metrics Table** - `a1fa6ab` (feat)

## Files Created/Modified
- `src/config.rs` - Settings struct with multi-source configuration loading (CLI, env vars, TOML)
- `src/duckdb_buffer.rs` - Added schema versioning and migration system; migration 001 creates process_metrics table
- `src/main.rs` - Integrated configuration loading with CLI argument parsing; displays loaded settings
- `src/lib.rs` - Exported config module
- `Cargo.toml` - Added toml dependency for config file parsing

## Decisions Made

1. **Configuration precedence: CLI > Env vars > Config file > Defaults**
   - Rationale: Allows maximum flexibility - ops can override via env vars, users via CLI, sane defaults for new installations

2. **Auto-create default config file at ~/.livedata/config.toml**
   - Rationale: Better UX - users get a working config immediately without manual creation

3. **Schema version stored in _schema_version table with description and timestamp**
   - Rationale: Enables tracking which migrations applied when, aids debugging and auditing

4. **Migration 001 ensures both journal_logs and process_metrics exist**
   - Rationale: Handles fresh installs and upgrades from pre-migration codebase

5. **Default retention: logs 30 days/1GB, processes 7 days/0.5GB**
   - Rationale: Conservative defaults balance storage cost with debugging utility

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None - implementation proceeded smoothly with all tests passing.

## User Setup Required

None - no external service configuration required.

Configuration file is auto-created at `~/.livedata/config.toml` on first run. Users can optionally customize via:
- CLI flags: `--log-retention-days`, `--log-max-size-gb`, `--process-retention-days`, `--process-max-size-gb`
- Environment variables: `LIVEDATA_LOG_RETENTION_DAYS`, `LIVEDATA_LOG_MAX_SIZE_GB`, etc.
- Edit config file directly

## Next Phase Readiness

Ready for next plans:
- Configuration system available for all modules to use retention settings
- Database migration system ready to accept new migrations
- Process metrics table structure established for data collection

No blockers or concerns.

---
*Phase: 02-storage-enhancements*
*Completed: 2026-02-05*

## Self-Check: PASSED

All created files exist:
- src/config.rs ✓

All commits exist:
- e74b3d3 ✓
- a1fa6ab ✓
