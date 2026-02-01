# livedata

## What This Is

A single binary system monitoring tool that provides instant observability for local machines. Download and run to immediately see system logs, metrics (CPU/memory/process stats), and search through them with a hybrid interface (simple for quick queries, power-user SQL/SPL for deep analysis). Web interface first, desktop GUI later.

## Core Value

Instant system observability without infrastructure overhead — download, run, and understand what's happening.

## Requirements

### Validated

(None yet — ship to validate)

### Active

- [ ] Collect system logs (journald, syslog, application logs)
- [ ] Collect system metrics (CPU, memory, disk, network)
- [ ] Collect process information (running processes, resource usage)
- [ ] Provide web UI for viewing current system state
- [ ] Support simple text-based search for quick investigations
- [ ] Support power-user SQL/SPL queries for deep analysis
- [ ] Store data locally with configurable retention
- [ ] Correlate logs with metrics for root cause analysis
- [ ] Single binary deployment (zero-config installation)
- [ ] Minimal resource overhead (don't burden the monitored system)

### Out of Scope

- [Remote monitoring via SSH] — v2 feature, focus on local first
- [Desktop GUI application] — v2 feature, web UI first
- [macOS support] — v2 feature, Linux first
- [Windows support] — v2+ feature, cross-platform later
- [Distributed/clustered monitoring] — Out of scope, single-machine focus
- [Alerting and notifications] — v2+ feature, observability first

## Context

**Technical environment:**
- Rust-based single binary application
- Parquet for data storage (columnar, efficient for time-series data)
- DuckDB for querying (fast analytics engine)
- Web-based search interface inspired by Splunk

**Problem motivation:**
Existing observability tools (Splunk, Grafana, Prometheus stack) require significant infrastructure and setup time. When investigating system issues, you need answers fast — not time spent configuring and deploying monitoring infrastructure.

**Inspiration:**
- Splunk's search language and investigation speed
- Grafana's dashboard visualization
- Single-binary tools like `htop` that just work

**Existing codebase:**
Project already has Rust infrastructure with parquet and DuckDB integration ready for data storage and querying.

## Constraints

- **Platform**: Linux first — must work on modern Linux distributions (Ubuntu, Debian, RHEL, etc.) — Rationale: Start with most common server OS, expand to macOS/Windows later

- **Installation**: Zero-config, single binary — User downloads and runs with no setup steps — Rationale: "Instant" means no friction between download and seeing data

- **Resource overhead**: Minimal — The tool must not significantly impact the system it monitors — Rationale: Monitoring tools that burden systems create the problems they're meant to help investigate

- **Architecture**: Local-first — All data collection, storage, and querying happens on the monitored machine — Rationale: Simplicity, privacy, no network dependency for basic use

- **Data retention**: Configurable — User can control how long data is kept — Rationale: Different use cases need different retention (live debugging vs trend analysis)

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Single binary architecture | Zero-config deployment, portable, no runtime dependencies | — Pending |
| Local data storage | Privacy, no network dependency, simple deployment | — Pending |
| Parquet + DuckDB stack | Columnar storage for time-series, fast SQL queries, proven in Rust ecosystem | — Pending |
| Hybrid search interface | Simple for quick use cases, powerful for deep investigations | — Pending |
| Web UI first | Easier to iterate, works across browsers, desktop GUI can wrap it | — Pending |

---
*Last updated: 2026-02-01 after initialization*
