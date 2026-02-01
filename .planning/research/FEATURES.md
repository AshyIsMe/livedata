# Feature Research

**Domain:** System Monitoring / Observability
**Researched:** 2025-02-01
**Confidence:** MEDIUM

## Feature Landscape

### Table Stakes (Users Expect These)

Features users assume exist. Missing these = product feels incomplete.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| CPU monitoring | Core metric for system health | LOW | User, system, idle percentages by core |
| Memory monitoring | Essential for detecting leaks and exhaustion | LOW | Used, available, cached, swap |
| Disk monitoring | Detect I/O bottlenecks and capacity issues | MEDIUM | Read/write throughput, usage by mount point |
| Network monitoring | Track connectivity and bandwidth | MEDIUM | TX/RX bytes, packets, errors by interface |
| Process monitoring | Identify resource-hungry processes | MEDIUM | CPU%, memory%, PID, user, runtime |
| Historical data retention | Diagnose past issues and capacity plan | HIGH | Requires time-series database or parquet storage |
| Basic visualization | Humans need graphs, not just raw data | LOW | Line charts, bar charts, gauges |
| Alerting | Proactive issue detection | MEDIUM | Threshold-based, notification delivery |
| Multi-platform support | Modern infra is hybrid | HIGH | Linux, macOS, Windows at minimum |
| Log aggregation | Correlate metrics with events | MEDIUM | journald, syslog, Windows Event Log |

### Differentiators (Competitive Advantage)

Features that set the product apart. Not required, but valuable.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Single binary deployment | Zero installation friction, runs everywhere | LOW | No dependencies, no config files needed |
| Zero configuration | Works out of the box, no learning curve | MEDIUM | Auto-detect system, sensible defaults |
| DuckDB local storage | Fast SQL queries, parquet export, lightweight | MEDIUM | Columnar, in-process, no external DB |
| Splunk-like search interface | Familiar power-user query language | HIGH | Time range, filters, aggregations, stats |
| SSH-based remote collection | Monitor fleets from one place, no agents | MEDIUM | Uses SSH keys, stores data locally |
| fzf-style fuzzy search | Fast keyboard-driven exploration | MEDIUM | Incremental filtering, intuitive UX |
| No infrastructure overhead | No Docker, Kubernetes, cloud services needed | N/A | Runs directly on host, minimal resources |
| Web + CLI interfaces | Use whatever tool fits the workflow | HIGH | Same backend, multiple frontends |
| Parquet export to S3 | Long-term retention, separate storage | MEDIUM | Push historical data to object storage |

### Anti-Features (Commonly Requested, Often Problematic)

Features that seem good but create problems.

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Enterprise dashboards by default | Users want pretty graphs out of the box | Overwhelms new users, bloats codebase | Simple CLI/TUI first, dashboards as plugin |
| Hundreds of integrations | "Monitor everything" appeal | Massive maintenance burden, security surface | Focus on platform metrics, extensible collector |
| Complex query language | Power users want SQL-like capabilities | Steep learning curve, slows adoption | Simple search syntax, optional advanced mode |
| Multi-node clustering | High availability requirement | Violates single-binary simplicity, adds complexity | Single-node monitoring is acceptable, use external HA |
| Built-in alert delivery channels | Users want alerts via Slack/email/Telegram | Integrations break, maintenance overhead | Webhook notifications only, let user handle routing |
| Custom plugin system | "Users can extend it" appeal | Plugin API complexity, versioning nightmares | Simple config file for collectors, no code plugins |
| Real-time streaming UI | Everything should be live | Performance overhead, unnecessary for system monitoring | Periodic refresh (1-5s), live tail for logs only |
| RBAC and multi-tenancy | Teams want to share monitoring | Adds auth complexity, breaks single-binary model | Single-user design, multiple instances for multi-user |
| AI/ML anomaly detection | "It should tell me what's wrong" | False positives, complex tuning, opaque behavior | Threshold-based alerting, manual investigation |

## Feature Dependencies

```
[Metrics Collection]
    └──requires──> [System Probes (CPU, Mem, Disk, Net)]
    └──requires──> [Process Scanner]

[Historical Storage]
    └──requires──> [Time-Series Database (DuckDB/Parquet)]
    └──requires──> [Data Ingestion Pipeline]

[Search Interface]
    └──requires──> [Historical Storage]
    └──enhances──> [Metrics Collection]

[Alerting]
    └──requires──> [Metrics Collection]
    └──requires──> [Historical Storage]
    └──requires──> [Notification Delivery]

[Remote Monitoring (SSH)]
    └──requires──> [Metrics Collection]
    └──enhances──> [Historical Storage]

[Parquet Export to S3]
    └──requires──> [Historical Storage]
```

### Dependency Notes

- **[Metrics Collection] requires [System Probes]:** Cannot collect metrics without OS-specific probes for CPU, memory, disk, and network
- **[Search Interface] requires [Historical Storage]:** Searching requires data to be stored, not just streamed
- **[Search Interface] enhances [Metrics Collection]:** Makes collected metrics actionable through queries
- **[Alerting] requires [Metrics Collection]:** Cannot alert on data you don't collect
- **[Alerting] requires [Historical Storage]:** Many alert conditions require historical context (e.g., "5 minute average")
- **[Remote Monitoring (SSH)] enhances [Historical Storage]:** Aggregating data from multiple hosts increases value of storage
- **[Parquet Export] requires [Historical Storage]:** Cannot export what hasn't been stored

## MVP Definition

### Launch With (v1)

Minimum viable product — what's needed to validate the concept.

- [ ] **System Probes** — Core data collection (CPU, memory, disk, network, processes) - essential
- [ ] **DuckDB Storage** — Local time-series storage with parquet export - needed for search
- [ ] **CLI Search Interface** — Basic query interface with filters and time ranges - differentiator
- [ ] **Single Binary** — No dependencies, runs out of the box - core value prop
- [ ] **Linux Support** — Primary target platform - where most server monitoring happens

### Add After Validation (v1.x)

Features to add once core is working.

- [ ] **Web Interface** — Splunk-like UI for broader adoption - trigger: users request GUI
- [ ] **SSH Remote Collection** — Monitor fleets from one place - trigger: user has >1 machine
- [ ] **Alerting** — Threshold-based notifications - trigger: users miss issues proactively
- [ ] **macOS Support** — Expand platform coverage - trigger: macOS user demand
- [ ] **Log Aggregation** — journald integration - trigger: users want logs + metrics

### Future Consideration (v2+)

Features to defer until product-market fit is established.

- [ ] **Windows Support** — Full platform parity - defer: different OS API, complexity
- [ ] **Parquet to S3 Export** — Long-term retention - defer: storage management complexity
- [ ] **Custom Collectors** — User-defined metrics - defer: plugin architecture needed
- [ ] **Dashboard Builder** — Visual dashboards - defer: nice-to-have, not core
- [ ] **API** — Programmatic access - defer: build for integrations later

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| System Probes (CPU, Mem, Disk, Net) | HIGH | LOW | P1 |
| Single Binary Deployment | HIGH | LOW | P1 |
| CLI Search Interface | HIGH | MEDIUM | P1 |
| DuckDB Storage | HIGH | MEDIUM | P1 |
| Linux Support | HIGH | MEDIUM | P1 |
| Web Interface | HIGH | HIGH | P2 |
| SSH Remote Collection | HIGH | MEDIUM | P2 |
| Alerting | MEDIUM | MEDIUM | P2 |
| Log Aggregation | MEDIUM | HIGH | P2 |
| macOS Support | MEDIUM | MEDIUM | P3 |
| Windows Support | MEDIUM | HIGH | P3 |
| Parquet to S3 Export | MEDIUM | MEDIUM | P3 |
| Custom Collectors | LOW | HIGH | P3 |
| Dashboard Builder | LOW | HIGH | P3 |
| API | LOW | HIGH | P3 |

**Priority key:**
- P1: Must have for launch
- P2: Should have, add when possible
- P3: Nice to have, future consideration

## Competitor Feature Analysis

| Feature | Splunk | Grafana/Prometheus | htop/btop | Netdata | livedata (Our Approach) |
|---------|--------|-------------------|-----------|---------|-------------------------|
| Deployment | Heavy infrastructure (servers, forwarders) | Prometheus + Grafana + multiple services | Single binary | Agent + optional cloud | Single binary, zero deps |
| Storage | Proprietary TSDB | Prometheus TSDB (local) or remote storage | None (real-time only) | Local DB + optional cloud | DuckDB local + parquet export |
| Query Interface | SPL (complex, powerful) | PromQL (complex) + Grafana UI | Interactive TUI | Web UI (rich) | CLI search (simple) + Web UI |
| Configuration | Complex, enterprise | YAML configs for everything | None | Auto-detects mostly | Zero config, auto-detect |
| Historical Data | Yes (expensive) | Yes (limited without remote) | No | Yes (auto-retention) | Yes (local, user-controlled) |
| Multi-host | Native (forwarders) | Native (service discovery) | Single host | Yes (agents + parents) | SSH-based (push model) |
| Learning Curve | Steep | Steep | Shallow | Shallow | Shallow (CLI) to medium (Web) |
| Pricing | Expensive (per GB ingestion) | Open source + paid cloud | Free | Open source + paid cloud | Open source (local data) |

## Sources

- WebSearch: "Splunk features system monitoring 2025" - LOW confidence (marketing pages)
- WebSearch: "Grafana features Prometheus metrics visualization" - LOW confidence (community articles)
- WebSearch: "htop system monitoring features CLI tools" - LOW confidence (tutorials)
- WebSearch: "Datadog vs New Relic comparison 2025" - LOW confidence (vendor comparisons)
- WebSearch: "Top infrastructure monitoring tools 2026" - LOW confidence (comparison articles)
- WebSearch: "New table stakes of observability" - MEDIUM confidence (industry analysis)
- WebFetch: https://vector.dev/ - MEDIUM confidence (official docs for comparison)
- WebFetch: https://www.netdata.cloud/ - MEDIUM confidence (official competitor site)
- WebFetch: https://github.com/flo-at/minmon - MEDIUM confidence (lightweight monitoring tool)
- WebFetch: https://github.com/henrygd/beszel - MEDIUM confidence (lightweight monitoring hub)
- WebFetch: https://github.com/guackamolly/zero-monitor - MEDIUM confidence (zero-config monitor)
- WebFetch: https://machaddr.substack.com/p/a-guide-to-linux-system-monitoring - MEDIUM confidence (comparison)
- WebFetch: https://clickhouse.com/resources/engineering/top-infrastructure-monitoring-tools-comparison - MEDIUM confidence (technical comparison)

---
*Feature research for: System Monitoring / Observability*
*Researched: 2025-02-01*
