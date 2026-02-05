# Phase 2: Storage Enhancements - Context

**Gathered:** 2026-02-05
**Status:** Ready for planning

<domain>
## Phase Boundary

Users can control data retention and schema changes. The application will automatically manage its storage footprint based on user-defined limits (time and size) and handle database schema evolution on startup.

</domain>

<decisions>
## Implementation Decisions

### Retention Granularity
- **Per data type:** Separate retention settings for logs and process metrics.
- **Dual Limits:** Retention defined by both Time (e.g., 30d) and Size (e.g., 1GB). Data is deleted when either limit is hit.
- **Default:** 30 days if not otherwise specified.
- **Silent Drop:** Old data is dropped silently without user prompts during rotation.

### Cleanup Strategy
- **Continuous:** Checks run both on startup and periodically (every 5-15 minutes).
- **High Priority:** Cleanup tasks should run at high priority to complete quickly once triggered.
- **Uninterruptible:** Once a cleanup cycle starts, it should complete the full cycle rather than yielding to system load.

### Config Interface
- **CLI Precedence:** CLI flags override settings in the configuration file.
- **Persistence:** Environment variables are supported (`LIVEDATA_RETENTION_*`).
- **Location:** Default config file lives in the user's home directory (e.g., `~/.livedata/config.toml`).
- **Lifecycle:** Restart is required for retention/storage changes; no hot-reloading.

### Schema Evolution UX
- **Automatic:** System auto-migrates schema on startup if a version mismatch is detected.
- **Safety First:** Always perform an auto-backup of the database file before starting a migration.
- **Handling Drift:** Attempt to preserve data, but drop incompatible data if it cannot be mapped to the new schema.
- **Visibility:** Verbose progress reporting in the console during migration.

### UI Additions (Navigation & Health)
- **Global Navigation:** A header link to the processes page will be added to the log search page (and vice-versa).
- **Persistence:** The link is always visible, not contextual.
- **Behavior:** Links open in a **new tab** to preserve user search context.
- **Visibility:** Add a storage health/retention status indicator to the UI.

### Claude's Discretion
- Exact format of the storage health indicator.
- Internal mapping logic for schema drift.
- Frequency tuning for periodic cleanup within the 5-15 min range.

</decisions>

<specifics>
## Specific Ideas

- "I want it to feel like it's managing itself once I set the limits."
- Navigation should be effortless between the two main data views (Logs and Processes).
- Storage stats in the UI help the user understand why data might be missing (e.g., "Retention: 7 days, currently using 800MB/1GB").

</specifics>

<deferred>
## Deferred Ideas

- **Contextual/Deep Linking:** Clicking a process name in a log entry to jump to that process's metrics (Deferred to a future UX/Correlation phase).
- **Hot-reloading config:** Dynamic retention changes without restart (Low priority, deferred).

</deferred>

---

*Phase: 02-storage-enhancements*
*Context gathered: 2026-02-05*
