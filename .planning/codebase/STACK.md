# Technology Stack

**Analysis Date:** 2026-02-01

## Languages

**Primary:**
- Rust 2024 Edition - All source code

**Secondary:**
- HTML/CSS/JavaScript - Web UI (embedded in web_server.rs)

## Runtime

**Environment:**
- Linux (systemd-based systems required)
- Standard Rust toolchain

**Package Manager:**
- Cargo
- Lockfile: Cargo.lock (present)

## Frameworks

**Core:**
- tokio 1.49 - Async runtime for concurrent operations and web server
- axum 0.8.8 - Web framework for HTTP server and API endpoints
- systemd 0.10 - Systemd journald integration for log reading

**Testing:**
- Built-in Rust test framework - Unit and integration tests
- tempfile 3.24 - Test isolation with temporary directories

**Build/Dev:**
- clap 4.0 - Command-line argument parsing
- anyhow 1.0 - Error handling and type aliases
- tracing/tracing-subscriber 0.1.44/0.3.22 - Structured logging
- log 0.4 - Fallback logging facade

## Key Dependencies

**Critical:**
- duckdb 1.4.3 - In-memory/on-disk OLAP database with bundled DuckDB, serde_json, r2d2, and parquet features
- chrono 0.4 - Date/time handling with serde support
- arrow 57.2.0 - Apache Arrow format integration
- serde/serde_json 1.0 - JSON serialization/deserialization

**Infrastructure:**
- signal-hook 0.3 - Unix signal handling for graceful shutdown
- tower-http 0.6.8 - HTTP utilities (file serving)
- gethostname 0.4 - System hostname retrieval

**Data Processing:**
- r2d2 (via duckdb features) - Database connection pooling
- parquet (via duckdb features) - Columnar storage format

## Configuration

**Environment:**
- Command-line arguments via clap (no environment variables required)
- Data directory configurable via `--data-dir` flag (default: `./data`)

**Build:**
- Cargo.toml - Package manifest with profile optimizations
- Custom rustflags for x86_64-linux-gnu target (linker: lld)
- DuckDB package optimized at level 3 in dev profile

## Platform Requirements

**Development:**
- Rust toolchain with 2024 edition support
- Linux system with systemd/journald
- Cargo for building and testing

**Production:**
- Linux with systemd (for journald access)
- Appropriate file permissions for data directory
- Network access for web server (127.0.0.1:3000 by default)

---

*Stack analysis: 2026-02-01*
