# Technology Stack

**Project:** livedata
**Domain:** Single-binary system monitoring tool
**Researched:** 2025-02-01
**Confidence:** HIGH

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|----------------|
| **Axum** | 0.8.8 | Web framework | Backed by Tokio team, modern ergonomic API, Tower middleware ecosystem, de facto standard for 2025. Actix-web is performant but Axum offers better developer experience and async patterns |
| **Tokio** | 1.44+ | Async runtime | Industry standard, mature ecosystem, excellent docs. Required for Axum, tracing, most async libraries. Use with `rt-multi-thread` feature |
| **DuckDB** | 1.4.4 | In-process OLAP database | "SQLite for analytics" - embedded, columnar, SQL-compatible. Perfect for querying Parquet data without separate server process. Rust client has ergonomic wrapper around C API |
| **Parquet** | 57.2.0 | Columnar storage format | Official Apache Arrow implementation. 3x-9x faster metadata parsing with custom Thrift parser. Integrates with Arrow for zero-copy data pipeline |

### Supporting Libraries

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| **sysinfo** | 0.38.0 | System metrics & process info | CPU, memory, disk, network, processes. Cross-platform (Linux, macOS, Windows, FreeBSD, NetBSD, Android, iOS, Raspberry Pi). Use `System::new_all()` and call `refresh_specifics()` for performance |
| **systemd** | 0.10.1 | Journald log reading | `systemd::journal::Journal` struct for reading systemd journal. Supports filters, seeking, real-time watching. Linux-only but essential for journald integration |
| **serde** | 1.0.228 | Serialization framework | De facto standard. Use derive macros for structs. Add `serde` feature to other crates for serialization support |
| **serde_json** | 1.0.149 | JSON serialization | Fast, reliable JSON (de)serialization. Required for DuckDB JSON export/import, web API responses |
| **tracing** | 0.1.44+ | Structured logging | Modern tracing framework preferred over `log`. Use `tracing-subscriber` with `env-filter` feature for RUST_LOG support |
| **clap** | 4.5.55 | CLI argument parsing | Derive-based API. Use `#[derive(Parser)]` on config struct. Features: `derive`, `cargo`, `env` |
| **chrono** | 0.4.43+ | Date/time handling | Standard timestamp library. Use for log timestamps, retention periods. Consider `chrono` feature on sysinfo for serialization |
| **toml** | 0.9.11+ | Config file parsing | Rust's TOML implementation. Use for livedata.conf. Add `serde` feature for struct deserialization |
| **config** | 0.15.19 | Advanced config management | Multi-format support (TOML, JSON, YAML, ENV). Use for complex config with layers, file watching, environment variable overrides |
| **tower** | 0.5.2+ | Middleware ecosystem | Composable middleware. Axum uses Tower for timeouts, compression, tracing, CORS. Required for production-ready web servers |
| **tower-http** | 0.6.8+ | HTTP-specific middleware | Adds compression, CORS, headers, trace propagation to Tower ecosystem. Use `tower_http::ServiceBuilderExt::map_response` pattern |
| **rust-embed** | 8.4.0 | Static asset embedding | Embed files at compile time for single binary. Use `#[derive(RustEmbed)]` on assets directory. Falls back to filesystem in dev |
| **anyhow** | 1.0.75 | Ergonomic error handling | Context-rich errors with `?` operator. Use for application errors. Use `thiserror` for library error types |
| **thiserror** | 1.0.61+ | Custom error types | Derive macros for error enums. Use for domain-specific errors with helpful `Display` messages |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| **rustfmt** | Code formatting | Use 4-space indentation, 100-char line limit (default) |
| **clippy** | Linting | Run `cargo clippy --all-targets -- -D warnings` for strict checking. Auto-fix suggestions with `--fix` |
| **cargo-nextest** | Test runner | Install with `cargo install cargo-nextest`. Use `cargo nextest run` for faster test feedback |

## Installation

```toml
[dependencies]
# Core runtime and web
tokio = { version = "1.44", features = ["full", "rt-multi-thread", "macros"] }
axum = { version = "0.8.8", features = ["tokio", "json", "query", "form"] }
tower = "0.5.2"
tower-http = { version = "0.6.8", features = ["fs", "trace", "cors", "compression-gzip"] }

# Database and storage
duckdb = "1.4.4"
parquet = { version = "57.2.0", features = ["arrow", "async"] }
arrow = "57.2.0"

# System monitoring
sysinfo = { version = "0.38.0", features = ["serde"] }
systemd = { version = "0.10.1", features = ["journal"] }

# Logging
tracing = "0.1.44"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }

# Serialization
serde = { version = "1.0.228", features = ["derive"] }
serde_json = "1.0.149"

# CLI and config
clap = { version = "4.5.55", features = ["derive", "env"] }
config = { version = "0.15.19", features = ["toml", "json"] }
toml = { version = "0.9.11", features = ["preserve_order"] }

# Time
chrono = { version = "0.4.43", features = ["serde"] }

# Error handling
anyhow = "1.0.75"
thiserror = "1.0.61"

# Static assets (dev)
rust-embed = { version = "8.4.0", optional = true }

[dev-dependencies]
cargo-nextest = "0.9"
```

**Feature flags for minimal overhead:**
```toml
# Disable multithreading in sysinfo for lower memory on some platforms
sysinfo = { version = "0.38.0", default-features = false, features = ["serde"] }

# Use minimal tower-http features
tower-http = { version = "0.6.8", default-features = false, features = ["fs", "trace"] }

# For static assets in release
[dependencies.rust-embed]
rust-embed = { version = "8.4.0", features = ["include-exclude"] }
```

## Alternatives Considered

| Category | Recommended | Alternative | Why Not |
|----------|-------------|------------|---------|
| **Web Framework** | Axum | Actix-web: More mature ecosystem, but Axum has better ergonomics and Tower integration. Rocket: Type-safe but macro-heavy and async story less mature. Warp: Functional style, steeper learning curve |
| **Async Runtime** | Tokio | async-std: More modern design, but Tokio has 10x+ ecosystem adoption, better docs, and Axum requires it. smol: Excellent but smaller ecosystem, Axum integration |
| **System Metrics** | sysinfo | heim: Async and well-designed, but less actively maintained (last release 2022). sys_metrics: Less mature, smaller community |
| **Database** | DuckDB | SQLite: No columnar/OLAP features. Postgres/MySQL: External dependency, adds deployment complexity |
| **Storage** | Parquet | JSON: Human-readable but inefficient for large datasets. CSV: No schema enforcement, slow for querying |
| **Logging** | tracing | log crate: Simpler but no structured spans/tracing context. env_logger: Good for simple apps, less flexible than tracing-subscriber |
| **Serialization** | serde | manual: Too verbose, error-prone. bincode: Fast but non-human-readable |
| **CLI** | clap | pico: Smaller but less features. structopt: Older, less ergonomic derive API |

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| **actix-web** | Requires separate middleware ecosystem (Actix), more boilerplate than Axum + Tower | Axum with tower-http |
| **async-std** | Smaller ecosystem, many libraries don't support it, Axum tied to Tokio | Tokio |
| **log crate** | No structured tracing support, less powerful than tracing | tracing with tracing-subscriber |
| **slog** | Older ecosystem, less active development | tracing |
| **env_logger** | Limited configuration options, no JSON output format | tracing-subscriber with env-filter |
| **JSON for logs** | Splunk-like tools use structured logs, JSON adds parsing overhead | tracing with structured fields, optionally to JSON via tracing-subscriber |
| **manual config parsing** | Error-prone, verbose | config crate with multiple format support |
| **blocking system calls** | In async context, blocks the runtime | Tokio's blocking task spawns (`tokio::task::spawn_blocking`) or async alternatives where available |

## Stack Patterns by Variant

**If minimal binary size is critical:**
- Use `lto = true` in release profile
- Strip symbols: `strip = true`
- Disable default features: `default-features = false` on crates
- Use `rust-embed` only for essential assets
- Consider `jemallocator` for memory efficiency but increases binary size

**If fastest runtime performance is critical:**
- Use Tokio with `rt-multi-thread`
- Enable Parquet compression features
- Use DuckDB's Appender API for bulk inserts
- Enable `sysinfo` multithreading (default) for faster process enumeration
- Consider `mimalloc` or `jemallocator` for optimized allocation

**If zero-config is critical:**
- Provide sensible defaults for all settings
- Auto-detect available log sources (journald vs syslog)
- Use `config` crate with layered configuration (env file → /etc/livedata.conf → defaults)
- Auto-create data directories with `dirs` crate (e.g., `dirs::data_local_dir()`)
- Graceful degradation if log sources unavailable

## Version Compatibility

| Package A | Compatible With | Notes |
|-----------|----------------|-------|
| axum 0.8.x | tokio 1.0+, http 1.0, tower 0.5.x | tokio feature required |
| duckdb-rs 1.4.x | duckdb C API 1.4.x | Bundled with crate, no external dependency management needed |
| parquet 57.2.x | arrow 57.2.x | Use arrow feature for Arrow integration |
| sysinfo 0.38.x | libc 0.2.173+, platform-specific syscalls | Works on non-supported platforms but returns empty values |
| tracing 0.1.x | tracing-subscriber 0.3.x | Use matching minor versions |
| systemd 0.10.x | libsystemd-sys 0.9.x | Linux-only, requires journal feature |
| rust-embed 8.4.x | rust 1.70+ | Use include-exclude to reduce binary size for optional assets |

## Sources

- **Axum**: https://docs.rs/axum/latest/axum/ (HIGH confidence - official docs)
- **Tokio**: https://tokio.rs/tokio/tutorial (HIGH confidence - official docs)
- **DuckDB**: https://duckdb.org/docs/stable/clients/rust.html (HIGH confidence - official docs)
- **Parquet**: https://arrow.apache.org/blog/2025/10/23/rust-parquet-metadata/ (HIGH confidence - official Apache blog)
- **sysinfo**: https://docs.rs/sysinfo/latest/sysinfo/ (HIGH confidence - official docs)
- **systemd**: https://docs.rs/systemd/latest/systemd/journal/struct.Journal.html (HIGH confidence - official docs)
- **serde**: https://docs.rs/serde/latest/serde/ (HIGH confidence - official docs)
- **tracing**: https://docs.rs/tracing/latest/tracing/ (HIGH confidence - official docs)
- **clap**: https://docs.rs/clap/latest/clap/_tutorial/index.html (HIGH confidence - official docs)
- **config**: https://docs.rs/config/latest/config/ (HIGH confidence - official docs)
- **chrono**: https://docs.rs/chrono/latest/chrono/ (HIGH confidence - official docs)
- **Web Framework Comparison**: https://ritik-chopra28.medium.com/rust-web-frameworks-in-2025-actix-vs-axum-a-data-backed-verdict-b956eb1c094e (MEDIUM confidence - blog post from Sep 2025)
- **Tower**: https://docs.rs/tower/latest/tower/ (HIGH confidence - official docs)
- **rust-embed**: https://lib.rs/crates/rust-embed (MEDIUM confidence - crate registry)

---
*Stack research for: Single-binary system monitoring tool (livedata)*
*Researched: 2025-02-01*
