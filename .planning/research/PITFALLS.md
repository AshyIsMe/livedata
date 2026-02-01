# Pitfalls Research

**Domain:** System Monitoring Tools (Single Binary, Minimal Overhead)
**Researched:** 2026-02-01
**Confidence:** MEDIUM

## Critical Pitfalls

Mistakes that cause rewrites or major issues.

### Pitfall 1: Monitoring Tool Overhead Exceeding System Resources

**What goes wrong:**
The monitoring tool consumes significant CPU, memory, and I/O resources on the monitored system, causing performance degradation of the system it's meant to observe. Users experience slower applications, increased latency, and in severe cases, system crashes from resource exhaustion. In monitoring literature, this is known as the "Heisenberg effect" — the act of measuring changes the system being measured.

**Why it happens:**
Developers implement comprehensive data collection without careful resource budgeting. High-frequency metric collection, excessive logging, and inefficient data encoding/decoding paths consume substantial resources. Code instrumentation (e.g., auto-instrumentation libraries) wraps application code with monitoring logic, adding overhead to every function call or operation. Some APM tools have been measured to add over 44% overhead in certain scenarios.

**How to avoid:**
- Establish clear resource budgets from the start (e.g., < 1% CPU, < 50MB RSS memory)
- Use sampling instead of full collection for high-frequency events
- Implement adaptive collection rates that scale back under load
- Leverage eBPF or other kernel-level instrumentation where possible to avoid user-space overhead
- Profile and benchmark the monitoring tool under realistic loads before release
- Use zero-copy data structures and avoid unnecessary allocations in the hot path

**Warning signs:**
- System performance noticeably degrades when monitoring tool starts
- Monitoring tool shows high CPU usage in its own metrics
- Other applications report increased latency or reduced throughput
- OOM kills or CPU throttling occur under monitoring load
- Network traffic from monitoring exceeds data volume expectations

**Phase to address:**
Phase 1 (Data Collection) — Resource budgeting must be established before building collectors.

---

### Pitfall 2: Parquet Small File Problem

**What goes wrong:**
Data is stored in many small Parquet files rather than fewer large ones. This causes inefficient queries, longer read times, increased I/O operations, and metadata overhead. Each file requires separate metadata reads, independent opening, and separate statistical analysis. Querying thousands of small files can take orders of magnitude longer than querying one large file with equivalent data.

**Why it happens:**
Implementers write Parquet files on every flush interval (e.g., every minute) without considering file size. Real-time log streaming produces small batches that get written immediately. Poor partitioning strategies (e.g., partitioning by high-cardinality fields like user IDs) spread data across many files. The desire for "fresh" data leads to frequent file creation.

**How to avoid:**
- Implement buffering and write larger files (100MB-1GB) instead of frequent small writes
- Use in-memory buffers that accumulate data before flushing to Parquet
- Design partitioning around time buckets (hourly/daily) rather than high-cardinality fields
- Implement background compaction that merges small files into larger ones
- Configure minimum file size thresholds before writing
- Consider hybrid approach: recent data in write-optimized format, compacted to Parquet periodically

**Warning signs:**
- File count grows faster than data volume suggests
- Queries take longer despite small data volume
- Directory listings return thousands of files
- Filesystem metadata (inode count) grows rapidly
- Query engine reports many files scanned

**Phase to address:**
Phase 1 (Data Collection) — File writing strategy must be designed from the start.

---

### Pitfall 3: Schema Evolution Breaking Queries

**What goes wrong:**
When monitoring needs evolve and schema changes (adding fields, renaming columns, changing types), existing queries fail and data becomes unreadable. New columns don't appear in historical queries, renamed columns cause "column not found" errors, and type changes produce parse failures. Data that was previously queryable becomes inaccessible.

**Why it happens:**
Schema is treated as static rather than evolutionary. No schema registry or versioning is implemented. Changes are applied to new data without considering compatibility with historical data. Parquet files with different schemas exist side-by-side without proper handling. Query engines assume uniform schema across all files.

**How to avoid:**
- Implement schema versioning from day one
- Use schema registry to track changes and ensure compatibility
- Design for backward compatibility (new fields optional in old readers)
- Use schema-on-read with careful handling of missing/new fields
- Store schema metadata separately (e.g., in DuckDB or external catalog)
- Implement schema migration scripts for breaking changes
- Test queries against historical data after schema changes

**Warning signs:**
- Query errors appear after software updates
- New fields don't show in historical queries
- "Column not found" errors across time ranges
- Need to rewrite queries when adding new data sources
- Manual data intervention required after schema changes

**Phase to address:**
Phase 2 (Storage & Querying) — Schema handling is critical for long-term usability.

---

### Pitfall 4: Time Series Query Performance Degradation

**What goes wrong:**
Query performance degrades exponentially as data grows. Queries that took milliseconds on small datasets become seconds or minutes as data accumulates. Time range queries scan entire datasets instead of using efficient indexes. Aggregations over time become prohibitively slow, making trend analysis impractical.

**Why it happens:**
Lack of proper indexing on time columns. Queries don't leverage time-based partitioning or pruning. Full table scans occur even for time-filtered queries. Statistics and metadata aren't generated or used for query planning. Predicate pushdown isn't implemented, so filters are applied after loading data.

**How to avoid:**
- Implement time-based partitioning from the start
- Create indexes on timestamp columns (or leverage Parquet row group statistics)
- Use predicate pushdown to apply time filters at the storage layer
- Implement Bloom filters for frequently queried non-time columns
- Leverage DuckDB's time series optimizations
- Store min/max statistics per file/row group for pruning
- Test query performance with realistic data volumes (not synthetic small datasets)

**Warning signs:**
- Query time grows linearly with data volume
- EXPLAIN/EXPLAIN ANALYZE shows full scans for time-filtered queries
- No indexes visible on timestamp columns
- Statistics missing in Parquet footers or DuckDB metadata
- Historical queries significantly slower than recent queries

**Phase to address:**
Phase 2 (Storage & Querying) — Query performance is the core user experience.

---

### Pitfall 5: Memory Leaks and Unbounded Growth

**What goes wrong:**
The monitoring tool's memory usage grows unboundedly over time, eventually causing OOM kills or system instability. Memory usage increases monotonically even during idle periods. The tool starts at 50MB RSS, grows to 2GB after 24 hours, and crashes the monitored system. This is particularly acute in Rust where memory management is manual and allocators may not release memory back to the OS.

**Why it happens:**
Data structures accumulate without cleanup. Circular references prevent memory reclamation. Logging or metrics data is held in memory longer than necessary. Buffers for batch writes grow without bounds. String allocations and cloning in hot paths cause heap fragmentation. Custom global allocators don't release freed pages back to the OS.

**How to avoid:**
- Implement strict data retention policies with active cleanup
- Use weak references or Arc/Weak for cached data
- Profile memory usage under sustained load (use heaptrack, dhat, jemalloc-pprof)
- Set memory limits and implement backpressure when approached
- Use arenas or allocators for batch operations that free in bulk
- Avoid unnecessary cloning and String allocations
- Monitor memory usage and alert on growth anomalies
- Consider memory limit enforcement (e.g., cgroups, ulimit)

**Warning signs:**
- RSS memory grows monotonically over time
- Memory usage doesn't decrease even after data is flushed to disk
- Valgrind/massif/dhat shows growing heap allocations
- System swap usage increases with monitoring tool running
- OOM kills occur after extended uptime

**Phase to address:**
Phase 1 (Data Collection) — Memory management is foundational for long-running processes.

---

## Technical Debt Patterns

Shortcuts that seem reasonable but create long-term problems.

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|-----------------|-----------------|
| Write raw logs to files, query via grep later | Immediate ingest, simple storage | Slow queries, no correlation, manual investigation | Never — defeats the purpose of a monitoring tool |
| Use string serialization (JSON) for all data | No schema design needed, flexible | Large storage, slow queries, no type safety | Only in MVP development phase, must migrate |
| Collect every metric available | No decision about what to collect | Massive storage, poor performance, noise | Never — decide what matters, don't hoard |
| Skip compaction during development | Faster writes, simpler code | Small file problem, slow queries over time | Acceptable in early prototyping, must implement before production |
| Use blocking I/O in collector threads | Simple code, easier debugging | Poor throughput, thread exhaustion under load | Never — use async I/O from day one |
| Ignore schema versioning | Immediate data write, no compatibility checks | Breaking changes require manual intervention | Never — schema evolution is inevitable |
| Sample all data uniformly | Simple implementation | Miss rare events, incomplete picture | Acceptable for high-frequency metrics, not for events/logs |
| Store data unencrypted | No crypto overhead, faster I/O | Security risk, compliance issues | Only in trusted local-only deployments |
| Skip unit tests for hot paths | Faster initial development | Bugs in production are harder to find | Never — test the code that runs most frequently |

---

## Integration Gotchas

Common mistakes when connecting to external services.

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| DuckDB | Opening new connection per query | Use connection pooling or single persistent connection |
| Parquet files | Writing files without size consideration | Buffer and batch writes to reach target file sizes |
| System logs | Reading entire log files on every check | Use inotify/tail to read only new entries |
| Process metrics | Reading /proc for every process every second | Cache process lists, sample less frequently |
| Journalctl | Using full JSON output for all events | Filter and request only needed fields |
| Web UI | Polling backend for updates | Use WebSockets or SSE for real-time updates |
| Filesystem | Using stat() on every file for changes | Use inotify for event-driven monitoring |

---

## Performance Traps

Patterns that work at small scale but fail as usage grows.

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|-----------------|
| Inefficient data structures | CPU usage spikes at specific data volumes | Profile before committing to data structures; use hash maps for lookups | At ~1M entries for naive structures |
| Missing indexes | Query time degrades with data volume | Add indexes on time and frequently queried columns | At ~100MB-1GB of data for time-based queries |
| Excessive logging | Write bandwidth saturated, disk I/O bottleneck | Log only what's needed; implement log levels | At ~10K log lines/second on spinning disks |
| No query caching | Repeated expensive queries | Implement result caching for common queries | At >10 queries/second with same parameters |
| Synchronous writes | Collection threads blocked on I/O | Use async write-back with bounded buffer | At >1K writes/second |
| No data pruning | Storage grows unboundedly | Implement configurable retention and active cleanup | After weeks of continuous collection |
| Poor partitioning | Partition scanning overhead | Partition by time ranges that match query patterns | At >1000 partitions or >10 partition levels |
| No predicate pushdown | Scanning entire datasets for filtered queries | Implement filters at storage layer | At >1GB of data with selective queries |

---

## Security Mistakes

Domain-specific security issues beyond general web security.

| Mistake | Risk | Prevention |
|---------|------|------------|
| Log sensitive data without redaction | PII exposure, compliance violations | Implement sanitization pipeline before storage |
| Web UI without authentication | Unauthorized access to system logs | Add authentication (even simple password for local use) |
| Parquet files world-readable | Information leakage | Set restrictive filesystem permissions (600/640) |
| Query injection via SPL | Arbitrary code execution | Validate and sanitize all user queries; use prepared statements |
| No input validation on search | Resource exhaustion via malicious queries | Limit query complexity, timeout long-running queries |
| Plaintext storage of credentials | Credential theft if system compromised | Use encrypted storage or environment variables |
| No rate limiting on API | DoS via query flood | Implement request limiting and backpressure |
| Binary not verifiable | Supply chain attacks | Provide checksums/signatures for distribution |

---

## UX Pitfalls

Common user experience mistakes in this domain.

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Overwhelming dashboard | Analysis paralysis, ignore alerts | Progressive disclosure: start simple, expand on demand |
| No query history | Retyping common queries | Save and quick-load recent queries |
| No context for alerts | Unclear what's wrong | Include time ranges, system state, and related metrics |
| Complex query language only | Non-technical users can't investigate | Hybrid interface: simple search + power-user mode |
| No correlation between logs/metrics | Root cause analysis requires manual correlation | Click-through from log entries to related metrics |
| Slow page loads | Users give up, miss insights | Virtual scrolling, pagination, incremental loading |
| No help for query syntax | Trial and error frustration | Query builder UI, autocomplete, syntax highlighting |
| No drill-down capability | Can't investigate interesting anomalies | Click any data point to see underlying logs/events |

---

## "Looks Done But Isn't" Checklist

Things that appear complete but are missing critical pieces.

- [ ] **Real-time updates:** Web UI appears to query data but polls every 5 seconds — verify WebSocket/SSE for true real-time behavior
- [ ] **Historical queries:** Queries work on today's data but fail on month-old data — verify Parquet file compaction and schema compatibility across time
- [ ] **Resource limits:** Tool runs fine for hours but crashes after days — verify memory leak testing and backpressure under sustained load
- [ ] **Query performance:** Demo queries return in milliseconds but production queries take seconds — test with realistic data volumes (GBs, not MBs)
- [ ] **Data retention:** Configuration option exists but data never deleted — verify active cleanup processes actually run
- [ ] **Error handling:** Normal operation looks fine but malformed input crashes process — verify panic recovery and graceful degradation
- [ ] **Cross-process correlation:** Logs and metrics exist but can't link them together — verify common IDs/timestamps across data types
- [ ] **Query caching:** Second run of same query is same speed as first — verify caching strategy works for repeated queries
- [ ] **Resource isolation:** Collector runs fine but affects system under load — verify profiling with system under stress, not idle
- [ ] **Web UI authentication:** Login exists but any user can access all data — verify proper access controls and isolation between users (if multi-user)
- [ ] **Parquet compression:** Files written but storage grows faster than expected — verify compression is actually applied and effective
- [ ] **DuckDB connection pooling:** Single query works but concurrent requests fail — verify connection pooling doesn't have race conditions or exhaustion

---

## Recovery Strategies

When pitfalls occur despite prevention, how to recover.

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Small file problem | HIGH | 1. Stop collection 2. Implement compaction job 3. Batch-merge files to target size 4. Test queries on compacted data 5. Resume collection with new buffering strategy |
| Memory leak | HIGH | 1. Kill and restart process 2. Enable heap profiling 3. Run under production load with profiling 4. Identify leak source 5. Fix and redeploy |
| Schema breaking | MEDIUM | 1. Identify affected queries 2. Create migration script 3. Back up existing data 4. Apply migration to rewrite schemas 5. Test queries against migrated data |
| Query performance | MEDIUM | 1. Identify slow queries via logs 2. Run EXPLAIN ANALYZE 3. Add missing indexes or partitions 4. Rewrite queries for better predicate pushdown 5. Verify improvement |
| Resource exhaustion | HIGH | 1. Kill monitoring process to restore system 2. Reduce collection frequency/scope 3. Add resource limits (cgroups, ulimit) 4. Add backpressure under load 5. Gradually re-enable features with monitoring |
| Authentication bypass | HIGH | 1. Immediately disable public access 2. Audit all access logs 3. Fix authentication implementation 4. Add automated security testing 5. Rotate all credentials |
| Data corruption | CRITICAL | 1. Stop all writes 2. Identify corrupt files 3. Restore from backup 4. Implement write-ahead-log for future 5. Add integrity checks on read |
| Storage explosion | MEDIUM | 1. Implement emergency retention policy 2. Run cleanup job immediately 3. Archive old data to cold storage 4. Add automated monitoring of storage growth 5. Set up alerts for disk usage |

---

## Pitfall-to-Phase Mapping

How roadmap phases should address these pitfalls.

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Monitoring overhead | Phase 1 (Data Collection) | Measure tool's CPU/memory under load; ensure < 1% overhead |
| Parquet small file problem | Phase 1 (Data Collection) | Verify file sizes stay in 100MB-1GB range after 24 hours |
| Schema evolution breaking | Phase 2 (Storage & Querying) | Test queries across files with schema version differences |
| Time series query performance | Phase 2 (Storage & Querying) | Benchmark queries on 1GB+ historical data; ensure < 100ms common queries |
| Memory leaks | Phase 1 (Data Collection) | Run 24-hour soak test with heap profiling; verify stable RSS |
| Missing indexes | Phase 2 (Storage & Querying) | Confirm EXPLAIN shows index usage for time-filtered queries |
| Authentication/authorization | Phase 3 (Web Interface) | Verify unauthorized access is blocked for protected resources |
| Data retention | Phase 2 (Storage & Querying) | Confirm old data is deleted per configured retention policy |
| Query caching | Phase 2 (Storage & Querying) | Verify identical second query is faster than first |
| Real-time updates | Phase 3 (Web Interface) | Verify data appears without polling (WebSocket/SSE active) |
| Log/metric correlation | Phase 2 (Storage & Querying) | Verify linking from log entries to related metrics works |

---

## Sources

- **APM Overheads** (MEDIUM) — Groundcover blog on monitoring tool overhead, resource impact, and measurement challenges. Published 2025.
- **Parquet Pitfalls** (MEDIUM) — Puneet Agarwal's Medium article on common Parquet problems including small file problem, schema evolution, and performance issues. Published 2024.
- **AWS Monitoring Anti-Patterns** (HIGH) — Official AWS documentation on monitoring anti-patterns including blame culture, inadequate coverage, and noisy alarms. Official source.
- **Parquet Query Optimization** (MEDIUM) — InfluxDB technical article on optimizing Parquet queries including projection/predicate pushdown, page pruning, and I/O optimization. Published 2022.
- **Monitoring Anti-Patterns** (MEDIUM) — Various sources on common monitoring mistakes including alert fatigue, incorrect metric selection, and reactive vs proactive approaches.
- **Rust Memory Management** (LOW) — Articles and forum discussions on Rust-specific memory issues, allocator behavior, and profiling tools. WebSearch only, needs verification.
- **Time Series Query Performance** (LOW) — Stack Overflow and documentation on indexing and query optimization for time series data. WebSearch only, needs verification.
- **Splunk Implementation Mistakes** (LOW) — Articles on common Splunk deployment and configuration issues. WebSearch only, needs verification.

---

*Pitfalls research for: System Monitoring Tools (Single Binary, Minimal Overhead)*
*Researched: 2026-02-01*
