# Architecture Research

**Domain:** Single-binary system monitoring tools (Rust + Parquet + DuckDB)
**Researched:** 2025-02-01
**Confidence:** MEDIUM

## Standard Architecture

### System Overview

Single-binary system monitoring tools follow a modular architecture with clear separation of concerns while packaging everything into a single executable.

```
┌─────────────────────────────────────────────────────────────────────┐
│                      CLI / Web Interface Layer                   │
├─────────────────────────────────────────────────────────────────────┤
│  ┌──────────┐  ┌──────────┐  ┌──────────┐        │
│  │ Commands  │  │   Query  │  │   View   │        │
│  │  Handler  │  │  Engine  │  │ Renderer │        │
│  └─────┬────┘  └────┬─────┘  └────┬─────┘        │
│        │             │              │                 │
├────────┼─────────────┼──────────────┼─────────────────┤
│        │             │              │                 │
│  ┌─────▼─────┐  ┌───▼────────┐  ┌───▼───────┐    │
│  │ Metrics    │  │   Storage   │  │  Query    │    │
│  │ Collector  │  │   Manager   │  │  Service  │    │
│  └─────┬─────┘  └─────┬──────┘  └────┬──────┘    │
│        │                │                 │             │
├────────┼────────────────┼─────────────────┼─────────────┤
│        │                │                 │             │
│  ┌─────▼─────────────────────────────────────▼─────┐   │
│  │             Data Processing Core               │   │
│  │  - Parquet Writer                       │   │
│  │  - DuckDB Integration                     │   │
│  │  - Time-series Data Management             │   │
│  └───────────────────────────────────────────────┘   │
│                                                  │
└──────────────────────────────────────────────────────────┘
       ↓                    ↓                    ↓
  System Metrics      Parquet Files       DuckDB Database
  (CPU/Mem/Disk)    (Columnar Store)    (OLAP Engine)
```

### Component Responsibilities

| Component | Responsibility | Typical Implementation |
|-----------|---------------|----------------------|
| **CLI Handler** | Parse arguments, dispatch commands, manage lifecycle | clap/clap_derive crate |
| **Metrics Collector** | Poll system metrics (CPU, memory, disk I/O, network) | sysinfo crate, async collection |
| **Storage Manager** | Write data to Parquet, manage file rotation | parquet crate, async file writes |
| **Query Service** | Interface with DuckDB, execute SQL queries | duckdb crate, prepared statements |
| **View Renderer** | Format and display results (terminal or web) | ratatui for TUI, embedded assets for web |

## Recommended Project Structure

```
src/
├── main.rs              # CLI entry point, command dispatch
├── cli/                 # Argument parsing and command handlers
│   ├── mod.rs
│   ├── args.rs         # clap derive structs
│   └── commands.rs      # command implementations
├── collectors/           # System metrics collection
│   ├── mod.rs
│   ├── cpu.rs
│   ├── memory.rs
│   ├── disk.rs
│   ├── network.rs
│   └── snapshot.rs     # Unified data snapshot
├── storage/              # Data persistence layer
│   ├── mod.rs
│   ├── parquet.rs       # Parquet file operations
│   ├── duckdb.rs        # DuckDB connection & queries
│   └── writer.rs        # Async write coordination
├── query/                # Query execution layer
│   ├── mod.rs
│   ├── engine.rs        # Query builder/executor
│   └── schema.rs        # Table definitions
├── ui/                   # Presentation layer
│   ├── mod.rs
│   ├── terminal.rs      # TUI (if applicable)
│   └── formatter.rs     # Output formatting
└── lib.rs               # Library crate for testing
```

### Structure Rationale

- **cli/**: Separates user interface from business logic, enables CLI and programmatic usage
- **collectors/**: Isolated metric collection, easily testable, can add new metrics without touching core
- **storage/**: Abstracted persistence, swap backends without breaking collectors
- **query/**: Centralized query logic, DuckDB-specific implementation isolated
- **ui/**: Presentation independence, can swap TUI/web/formats

## Architectural Patterns

### Pattern 1: Collect-Process-Store Pipeline

**What:** Metrics flow from collection through optional processing to storage in a pipeline
**When to use:** All system monitoring scenarios
**Trade-offs:**
- Pros: Clean separation, easy to add processing stages, testable components
- Cons: Channel overhead for high-frequency metrics

**Example:**
```rust
// collector sends metrics via channel
let (tx, rx) = mpsc::channel::<Metric>(100);

tokio::spawn(async move {
    let collector = CpuCollector::new();
    loop {
        let metrics = collector.collect().await?;
        tx.send(metrics).await?;
    }
});

// storage receives and persists
tokio::spawn(async move {
    let storage = StorageManager::new("./data");
    while let Some(metrics) = rx.recv().await {
        storage.write_parquet(&metrics).await?;
    }
});
```

### Pattern 2: Repository Pattern for Data Access

**What:** Abstract storage behind trait, enable DuckDB + Parquet dual backend
**When to use:** When needing testable data access or multiple storage backends
**Trade-offs:**
- Pros: Easy mocking for tests, swap storage implementations
- Cons: Extra abstraction layer for simple cases

**Example:**
```rust
#[async_trait]
pub trait MetricRepository: Send + Sync {
    async fn store(&self, metrics: &[Metric]) -> Result<()>;
    async fn query(&self, sql: &str) -> Result<Vec<MetricRow>>;
}

pub struct ParquetRepository { /* ... */ }
pub struct DuckDBRepository { /* ... */ }
```

### Pattern 3: Builder Pattern for Configuration

**What:** Fluent configuration builders for collectors and storage
**When to use:** Complex configuration with many optional parameters
**Trade-offs:**
- Pros: Self-documenting API, defaults handling, validation
- Cons: More boilerplate than simple struct init

**Example:**
```rust
let collector = CollectorBuilder::new()
    .interval(Duration::from_secs(1))
    .include_network(true)
    .include_disk(true)
    .build()?;
```

## Data Flow

### Collection Flow

```
[System] → [Collector] → [Channel] → [Storage Manager] → [Parquet/DuckDB]
    ↓          ↓              ↓               ↓                ↓
  /proc      poll()      mpsc::channel()  write_parquet()   data file
  /sys      metrics      async buffer    write_duckdb()    in-memory DB
```

### Query Flow

```
[User Query] → [CLI Handler] → [Query Engine] → [DuckDB] → [Results]
     ↓              ↓                ↓            ↓            ↓
  CLI args      parse SQL       prepare()      execute()    map to structs
  / Search      build AST      bind params    fetch rows   format output
```

### Key Data Flows

1. **Metrics Collection Loop:** Collector polls → Channel buffer → Storage batches writes → Parquet file
2. **Query Execution:** User input → SQL parsing → DuckDB query → Arrow data → Formatted output
3. **Historical Query:** Parquet files → DuckDB scan → Time-range filter → Aggregation → Display

## Scaling Considerations

| Scale | Architecture Adjustments |
|-------|--------------------------|
| 0-1 users (local) | Single-threaded collector, synchronous writes, in-memory DuckDB |
| 1K-100K metrics/sec | Multi-threaded collectors, async channels, batch writes, columnar Parquet compression |
| 100K+ metrics/sec | Ring buffers, selective sampling, distributed storage, time-series partitioning |

### Scaling Priorities

1. **First bottleneck:** Disk I/O from frequent Parquet writes → Fix: Batch writes, increase flush interval
2. **Second bottleneck:** DuckDB query performance on large datasets → Fix: Time-based partitioning, create indexes

## Anti-Patterns

### Anti-Pattern 1: Monolithic main.rs

**What people do:** Put all logic in a single main.rs file
**Why it's wrong:** Unmaintainable, untestable, can't reuse code as library
**Do this instead:** Split into modules (cli/, collectors/, storage/, query/)

### Anti-Pattern 2: Direct System Calls Everywhere

**What people do:** Call /proc, /sys, and libduckdb directly from collector code
**Why it's wrong:** Hard to mock for tests, couples collectors to OS specifics, no portability
**Do this instead:** Create trait-based abstraction for metric sources

### Anti-Pattern 3: No Batching for High-Frequency Metrics

**What people do:** Write every metric individually to Parquet
**Why it's wrong:** Terrible I/O performance, high disk fragmentation, slow queries
**Do this instead:** Buffer metrics in memory, batch writes every N seconds or M rows

### Anti-Pattern 4: Synchronous Blocking Collection

**What people do:** Use blocking sysinfo calls in async runtime
**Why it's wrong:** Blocks executor thread pool, prevents concurrent operations
**Do this instead:** Wrap blocking calls in `spawn_blocking`, use async-aware collectors

## Integration Points

### External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| sysinfo crate | Direct calls to System trait | Cross-platform abstraction for /proc and /sys |
| DuckDB | Embedded in-process database | Use `Connection::open_in_memory()` for fast queries, `open()` for persistence |
| Parquet files | Direct file I/O with arrow-rs | Use ParquetWriter for columnar storage, compress with snappy/zstd |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| collectors ↔ storage | tokio::mpsc channels | Async buffer, backpressure handling |
| storage ↔ query | trait abstraction (MetricRepository) | Enables swapping Parquet/DuckDB, testable |
| cli ↔ collectors | function calls | CLI instantiates collectors with config, runs collection loop |

## Build Order Implications

For single-binary system monitoring tools with Rust + Parquet + DuckDB:

```
1. Core Types & Traits (lib.rs)
   └─> Defines Metric, MetricRow, MetricRepository traits
   └─> NO external dependencies (or minimal: thiserror, chrono)

2. Storage Layer (storage/)
   └─> parquet.rs: Parquet writer implementation
   └─> duckdb.rs: DuckDB connection and query execution
   └─> DEPENDS: Core Types, parquet crate, duckdb crate

3. Collectors (collectors/)
   └─> cpu.rs, memory.rs, disk.rs, network.rs
   └─> DEPENDS: Core Types, sysinfo crate, tokio

4. Query Engine (query/)
   └─> engine.rs: SQL parsing, query building
   └─> DEPENDS: Storage Layer traits

5. CLI Layer (cli/)
   └─> args.rs: clap derive structs
   └─> commands.rs: command implementations
   └─> DEPENDS: All previous layers

6. UI/Formatting (ui/)
   └─> formatter.rs: output formatting
   └─> DEPENDS: Query Engine results

7. Main Entry (main.rs)
   └─> Ties everything together
   └─> DEPENDS: All layers
```

### Build Order Rationale

- **Core types first:** Establish the contract between all layers early
- **Storage second:** Data persistence is critical for everything else, test early
- **Collectors third:** Need storage to write to, independent of CLI
- **Query fourth:** Depends on storage schema, needed for CLI
- **CLI fifth:** Orchestration layer, needs all capabilities
- **UI last:** Purely presentation, least dependencies

### Parallelization Opportunities

The following can build/test in parallel:
- Core Types + Storage Layer (no dependencies between them)
- Collectors (each collector independent)
- CLI argument parsing (no runtime dependencies)

## Sources

- [Building a Real-Time System Monitor in Rust Terminal](https://thenewstack.io/building-a-real-time-system-monitor-in-rust-terminal/) - HIGH confidence (direct source)
- [Simon - Single Binary System Monitor](https://github.com/alibahmanyar/simon) - HIGH confidence (verified codebase)
- [Rezolus - High-Resolution Telemetry](https://github.com/iopsystems/rezolus) - HIGH confidence (verified codebase)
- [Rust Project Structure Best Practices](https://www.djamware.com/post/68b2c7c451ce620c6f5efc56/rust-project-structure-and-best-practices-for-clean-scalable-code) - MEDIUM confidence (blog post)
- [DuckDB Rust Client Documentation](https://duckdb.org/docs/stable/clients/rust.html) - HIGH confidence (official docs)
- [Cargo Workspaces](https://doc.rust-lang.org/book/ch14-03-cargo-workspaces.html) - HIGH confidence (official docs)
