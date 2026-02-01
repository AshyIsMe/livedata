# Requirements: livedata

**Defined:** 2025-02-01
**Core Value:** Instant system observability without infrastructure overhead — download, run, and understand what's happening

## Requirements

### Validated

<!-- Shipped and confirmed valuable. -->

- ✓ **LOG-01**: Application collects system logs from journald — existing
- ✓ **STOR-01**: System data is stored locally in DuckDB for querying — existing
- ✓ **WEB-01**: Application provides web interface for data exploration — existing
- ✓ **SEARCH-01**: User can search logs using simple text-based queries — existing
- ✓ **SEARCH-02**: User can filter search results by time range — existing
- ✓ **DEPL-01**: Application deploys as single binary with no external dependencies — existing
- ✓ **DEPL-03**: Application runs on Linux systems — existing

### Active

<!-- Current scope. Building toward these. -->

### Process Monitoring

- [ ] **PROCE-01**: User can view list of running processes with PID, name, CPU percentage, memory percentage, user, and runtime
- [ ] **PROCE-02**: User can search/filter processes using fuzzy search (fzf-style)
- [ ] **PROCE-03**: Process data is collected at configurable intervals

### Data Storage

- [ ] **STOR-02**: Data retention is configurable by user
- [ ] **STOR-03**: Storage layer handles schema evolution for backward compatibility

### Deployment

- [ ] **DEPL-02**: Application requires zero configuration - works out of box

### v2+ Considerations

## v2 Requirements

Deferred to future release. Tracked but not in current roadmap.

### System Metrics

- **SYS-01**: User can view CPU usage (user, system, idle) by core
- **SYS-02**: User can view memory usage (used, available, cached, swap)
- **SYS-03**: User can view disk I/O (read/write throughput, usage by mount point)
- **SYS-04**: User can view network statistics (TX/RX bytes, packets, errors by interface)

### Advanced Query

- **QUERY-01**: User can execute SQL queries directly against data
- **QUERY-02**: User can use Splunk-like query syntax (pipelines, aggregations, stats)

### Visualization

- **VIS-01**: User can view data as line charts
- **VIS-02**: User can view data as bar charts
- **VIS-03**: User can view data as gauges
- **VIS-04**: User can view web interface for data exploration

### Log Aggregation

- **LOG-01**: Application collects system logs from journald
- **LOG-02**: Application collects system logs from syslog
- **LOG-03**: User can correlate logs with metrics for root cause analysis

### Remote Monitoring

- **REMOTE-01**: User can monitor remote machines via SSH (agentless)
- **REMOTE-02**: SSH authentication uses user's SSH keys
- **REMOTE-03**: Remote data is stored locally for querying

### Alerting

- **ALERT-01**: User can configure threshold-based alerts
- **ALERT-02**: Alerts trigger webhook notifications
- **ALERT-03**: Alert conditions support historical context (e.g., 5-minute average)

### Platform Expansion

- **PLAT-01**: Application runs on macOS
- **PLAT-02**: Application runs on Windows
- **PLAT-03**: Application exports data to Parquet for external storage

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| Multi-node clustering | Violates single-binary simplicity, adds complexity |
| Built-in alert delivery (email/Slack) | Integrations break, maintenance overhead |
| Custom plugin system | Plugin API complexity, versioning nightmares |
| Real-time streaming UI | Performance overhead, periodic refresh sufficient |
| RBAC and multi-tenancy | Adds auth complexity, breaks single-binary model |
| AI/ML anomaly detection | False positives, complex tuning, opaque behavior |
| Enterprise dashboards by default | Overwhelms new users, bloats codebase |
| Hundreds of integrations | Massive maintenance burden, security surface |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| LOG-01 | Existing | Complete |
| STOR-01 | Existing | Complete |
| WEB-01 | Existing | Complete |
| SEARCH-01 | Existing | Complete |
| SEARCH-02 | Existing | Complete |
| DEPL-01 | Existing | Complete |
| DEPL-03 | Existing | Complete |
| PROCE-01 | Phase 1 | Pending |
| PROCE-02 | Phase 1 | Pending |
| PROCE-03 | Phase 1 | Pending |
| STOR-02 | Phase 2 | Pending |
| STOR-03 | Phase 2 | Pending |
| DEPL-02 | Phase 3 | Pending |

**Coverage:**
- Active requirements: 6 total
- Validated requirements: 7 complete
- Mapped to phases: 6/6 ✓
- Unmapped: 0

---
*Requirements defined: 2025-02-01*
*Last updated: 2026-02-02 after roadmap creation*
