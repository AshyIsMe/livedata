# External Integrations

**Analysis Date:** 2026-02-01

## APIs & External Services

**Systemd Journald:**
- systemd 0.10 - Systemd journal log source
  - Client: `systemd::journal::{Journal, OpenOptions}`
  - Purpose: Read system logs from systemd journal
  - Access: Requires appropriate permissions to read system logs
  - Implementation: `src/journal_reader.rs` - JournalLogReader struct

**None:**
- No external HTTP APIs or third-party services used
- All integrations are local system resources

## Data Storage

**Databases:**
- DuckDB (on-disk file-based)
  - Connection: `src/duckdb_buffer.rs` - DuckDBBuffer struct
  - Client: duckdb crate (bundled DuckDB library)
  - Database file: `livedata.duckdb` (located in configured data directory)
  - Features: bundled, serde_json, r2d2, parquet

**File Storage:**
- Local filesystem - Database file stored in `./data/` directory (configurable)

**Caching:**
- None - DuckDB provides direct access to stored data

## Authentication & Identity

**Auth Provider:**
- None - No authentication or authorization implemented
  - Implementation: Web server runs on localhost (127.0.0.1) only
  - No user management, sessions, or access control

## Monitoring & Observability

**Error Tracking:**
- None - Standard logging only

**Logs:**
- tracing/tracing-subscriber 0.1.44/0.3.22 - Structured logging to stdout
  - Implementation: `src/main.rs` - tracing_subscriber initialization
  - Output: Console with level filtering (default: info)
  - Environment: `RUST_LOG` environment variable for log level control

## CI/CD & Deployment

**Hosting:**
- None specified - Binary application intended for direct deployment

**CI Pipeline:**
- None detected - No GitHub Actions or CI configuration present

## Environment Configuration

**Required env vars:**
- None required for basic operation
- Optional: `RUST_LOG` - Set logging level (e.g., `RUST_LOG=debug`)

**Secrets location:**
- No secrets required - Application runs with system user permissions

## Webhooks & Callbacks

**Incoming:**
- None

**Outgoing:**
- None

**Network Services:**
- Web server (axum/tokio) on port 3000
  - Bind address: `127.0.0.1:3000` (localhost only)
  - Purpose: Log search UI and REST API
  - Endpoints: `/`, `/api/search`, `/api/columns`, `/api/filters`, `/health`
  - Implementation: `src/web_server.rs` - run_web_server function

---

*Integration audit: 2026-02-01*
