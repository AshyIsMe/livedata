# Project Research Summary

**Project:** livedata
**Domain:** Single-binary system monitoring tool
**Researched:** 2025-02-01
**Confidence:** MEDIUM

## Executive Summary

Livedata is a single-binary system monitoring tool designed for minimal overhead and zero-configuration deployment. Research indicates the optimal approach combines Rust with Axum for the web framework, Tokio for async runtime, DuckDB for in-process OLAP queries, and Parquet for columnar storage. This stack provides Splunk-like search capabilities without infrastructure complexity, positioning livedata between htop's simplicity and Splunk's power.

The recommended architecture follows a Collect-Process-Store pipeline with modular components: CLI handlers, metrics collectors (CPU/memory/disk/network), storage management (Parquet writer + DuckDB integration), query engine, and presentation layers. Critical risks include monitoring tool overhead exceeding system resources, the Parquet small file problem, and schema evolution breaking queries. These must be addressed in Phase 1 through resource budgeting, buffered writes, and schema versioning from day one.

## Key Findings

### Recommended Stack

Stack research (HIGH confidence) identifies Axum 0.8.8 as the web framework of choice, backed by the Tokio team with modern ergonomic API and Tower middleware ecosystem. Tokio 1.44+ is required as the async runtime with industry-standard ecosystem. DuckDB 1.4.4 provides in-process OLAP database capabilities as "SQLite for analytics," perfect for querying Parquet data without a separate server process. Parquet 57.2.0 from Apache Arrow enables columnar storage with 3x-9x faster metadata parsing through a custom Thrift parser.

**Core technologies:**
- **Axum**: Web framework — backed by Tokio team, modern ergonomic API, Tower middleware ecosystem, de facto standard for 2025
- **Tokio**: Async runtime — industry standard, mature ecosystem, excellent docs, required for Axum and most async libraries
- **DuckDB**: In-process OLAP database — embedded, columnar, SQL-compatible, perfect for querying Parquet data without separate server

Supporting libraries include sysinfo for cross-platform system metrics, systemd for journald integration on Linux, tracing for structured logging, clap for CLI argument parsing, and tower-http for HTTP-specific middleware (compression, CORS, trace propagation).

### Expected Features

Feature research (MEDIUM confidence) reveals table stakes that users expect: CPU, memory, disk, and network monitoring are essential baseline metrics. Process monitoring identifies resource-hungry processes. Historical data retention is required for diagnosing past issues. Basic visualization, alerting, multi-platform support, and log aggregation round out minimum viable functionality.

Differentiators that set livedata apart: single binary deployment for zero installation friction, zero-configuration with auto-detection and sensible defaults, DuckDB local storage with parquet export for fast SQL queries, Splunk-like search interface for power users, SSH-based remote collection to monitor fleets from one place, and no infrastructure overhead (no Docker/Kubernetes/cloud services needed).

**Must have (table stakes):**
- **CPU/memory/disk/network monitoring** — core metrics for system health, users expect these
- **Process monitoring** — identify resource-hungry processes, essential for troubleshooting
- **Historical data retention** — diagnose past issues and capacity planning
- **Single binary deployment** — zero installation friction, core value proposition

**Should have (competitive):**
- **DuckDB local storage** — fast SQL queries, parquet export, lightweight
- **Splunk-like search interface** — familiar power-user query language, differentiator
- **SSH remote collection** — monitor fleets from one place, no agents needed
- **Zero configuration** — works out of the box, auto-detects system

**Defer (v2+):**
- **Windows/macOS support** — Linux is primary target for v1
- **Custom collectors/plugins** — plugin architecture complexity
- **Dashboard builder** — nice-to-have, not core value

### Architecture Approach

Architecture research (MEDIUM confidence) recommends a modular single-binary architecture with clear separation of concerns. The Collect-Process-Store pipeline flows metrics from collectors through channels to storage in Parquet/DuckDB. Repository pattern abstracts data access behind traits for testability. Builder pattern provides fluent configuration for collectors and storage.

**Major components:**
1. **CLI Handler** — parse arguments with clap, dispatch commands, manage lifecycle
2. **Metrics Collector** — poll system metrics (CPU, memory, disk I/O, network) using sysinfo, async collection via tokio
3. **Storage Manager** — write data to Parquet with async file writes, manage file rotation, DuckDB integration for queries
4. **Query Service** — interface with DuckDB using prepared statements, execute SQL queries
5. **View Renderer** — format results (terminal or web), TUI with ratatui, embedded assets for web

Recommended project structure: src/cli/, src/collectors/, src/storage/, src/query/, src/ui/, src/lib.rs. Build order: Core types → Storage → Collectors → Query → CLI → UI to respect dependencies.

### Critical Pitfalls

Pitfall research (MEDIUM confidence) identifies five critical issues that cause rewrites or major problems:

1. **Monitoring tool overhead exceeding system resources** — establish resource budgets (<1% CPU, <50MB RSS), use sampling, implement adaptive collection rates, profile before release
2. **Parquet small file problem** — buffer data and write larger files (100MB-1GB), use time-based partitioning, implement background compaction, avoid frequent flushes
3. **Schema evolution breaking queries** — implement schema versioning from day one, use schema registry, design for backward compatibility, store schema metadata separately
4. **Time series query performance degradation** — implement time-based partitioning, create indexes on timestamp columns, use predicate pushdown, leverage Parquet row group statistics
5. **Memory leaks and unbounded growth** — implement strict retention policies with cleanup, profile memory usage under sustained load, set memory limits and backpressure

## Implications for Roadmap

Based on research, suggested phase structure:

### Phase 1: Data Collection Core
**Rationale:** Must establish resource budgeting and data ingestion patterns before adding features. Architecture research identifies Core Types and Storage Layer as foundational build order dependencies. Pitfall research flags monitoring overhead and small file problem as Phase 1 issues.
**Delivers:** Working metrics collectors (CPU, memory, disk, network), Parquet writer with buffering strategy, basic CLI for collection control, resource budgeting implementation
**Addresses:** System probes, single binary, Linux support (MVP features)
**Avoids:** Monitoring tool overhead (Pitfall 1), Parquet small file problem (Pitfall 2)

### Phase 2: Storage and Querying
**Rationale:** After collection works, need storage persistence and query capability to make data actionable. Build order requires Storage before Query. Pitfall research flags schema evolution and query performance as Phase 2 concerns.
**Delivers:** DuckDB integration with connection pooling, SQL query engine with time-based indexing, Parquet file compaction, schema versioning system
**Uses:** DuckDB crate, Parquet crate, Arrow integration (from STACK.md)
**Implements:** Storage layer, Query engine (from ARCHITECTURE.md)
**Addresses:** DuckDB storage, CLI search interface (MVP features)
**Avoids:** Schema evolution breaking queries (Pitfall 3), Time series query performance degradation (Pitfall 4)

### Phase 3: Web Interface and Remote Collection
**Rationale:** Once core data pipeline works, add UI for broader adoption and SSH remote monitoring for fleet scenarios. Architecture places UI as last build step since it's purely presentation.
**Delivers:** Axum web server with Splunk-like search UI, WebSocket/SSE for real-time updates, SSH remote collection with key-based auth, alerting system
**Uses:** Axum, tower-http, rust-embed (from STACK.md)
**Implements:** View Renderer, Web Interface (from ARCHITECTURE.md)
**Addresses:** Web interface, SSH remote collection, alerting (v1.x features)
**Avoids:** Memory leaks in long-running web processes (Pitfall 5)

### Phase 4: Advanced Features and Platform Expansion
**Rationale:** After product-market fit validation on Linux, expand to other platforms and add nice-to-have features. This phase contains deferred features from MVP definition.
**Delivers:** macOS support, Windows support, Parquet to S3 export, custom collectors API, dashboard builder
**Addresses:** v2+ features from FEATURES.md

### Phase Ordering Rationale

This order respects architecture build order (Core → Storage → Collectors → Query → CLI → UI), addresses critical pitfalls early (overhead, small files, schema evolution in Phases 1-2), and delivers MVP functionality (system probes, storage, search) before nice-to-have features. Grouping collection/storage separately from UI avoids UI rework when data models evolve. Web interface waits until data pipeline is stable to prevent feature creep.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 2 (Storage and Querying):** DuckDB query optimization patterns for time series, Parquet compaction strategies, schema migration handling — complex integration, some gaps in research
- **Phase 3 (Web Interface):** Splunk-like search UI UX patterns, WebSocket vs SSE decision, real-time update architecture — niche domain, sparse documentation

Phases with standard patterns (skip research-phase):
- **Phase 1 (Data Collection):** Well-documented patterns for metrics collection with sysinfo, async channels, Parquet writing — established patterns in research
- **Phase 4 (Platform Expansion):** Standard cross-platform Rust patterns, S3 integration via AWS SDK — standard tooling

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Verified with official docs for Axum, Tokio, DuckDB, Parquet, sysinfo |
| Features | MEDIUM | Competitive analysis from multiple sources, some inference about user expectations |
| Architecture | MEDIUM | Verified open-source codebases (Simon, Rezolus), standard patterns identified |
| Pitfalls | MEDIUM | Groundcover APM research verified, Parquet pitfalls from Apache community, some Rust-specific issues need validation |

**Overall confidence:** MEDIUM

### Gaps to Address

- **DuckDB time series optimization:** Research covers basic usage but not advanced time series query patterns for large datasets — verify during Phase 2 planning with benchmarking
- **Parquet compaction strategy:** Research identifies need for compaction but not specific implementation approaches — evaluate options (offline batch job vs background daemon) in Phase 2
- **Web UI UX patterns:** Splunk-like search interface design has sparse documentation — reference existing tools (Netdata, Grafana) during Phase 3 UI design
- **Resource budgeting benchmarks:** Research identifies 1% CPU/50MB RSS as targets but lacks real-world data for Rust-based monitors — establish baseline metrics during Phase 1 with profiling

## Sources

### Primary (HIGH confidence)
- Axum Official Docs — web framework patterns, tower middleware ecosystem
- Tokio Official Tutorial — async runtime, rt-multi-thread, spawn_blocking patterns
- DuckDB Official Docs (Rust client) — in-process OLAP, connection patterns, Appender API
- Apache Arrow/Parquet Docs — columnar storage, metadata parsing, row group statistics
- sysinfo Crate Docs — cross-platform metrics, System::refresh_specifics() for performance
- systemd Crate Docs — Journal struct for journald reading, filters, seeking

### Secondary (MEDIUM confidence)
- Groundcover Blog (2025) — APM tool overhead, Heisenberg effect in monitoring
- Simon GitHub (alibahamyar/simon) — Verified single-binary monitor codebase
- Rezolus GitHub (iopsystems/rezolus) — High-resolution telemetry architecture
- Puneet Agarwal Medium (2024) — Parquet small file problem, schema evolution
- Netdata Website — Single-binary competitor architecture, auto-detection patterns
- henrygd/beszel GitHub — Lightweight monitoring hub approach
- machaddr Substack — Linux system monitoring tool comparison
- ClickHouse Engineering Blog — Infrastructure monitoring tools comparison

### Tertiary (LOW confidence)
- Ritik Chopra Medium (Sep 2025) — Axum vs Actix-web comparison, needs validation
- WebSearch results — Splunk features, Grafana patterns, competitor analysis, needs verification against official sources
- WebSearch results — Rust memory management, allocator behavior, needs validation with profiling tools

---
*Research completed: 2025-02-01*
*Ready for roadmap: yes*
