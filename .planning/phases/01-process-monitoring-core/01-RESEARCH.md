# Phase 1: Process Monitoring Core - Research

**Researched:** 2026-02-02
**Domain:** System process monitoring and web-based real-time data display
**Confidence:** HIGH

## Summary

Process monitoring on Linux requires collecting system metrics (PID, name, CPU%, memory%, user, runtime) and displaying them in a searchable web interface. The standard approach uses the `sysinfo` crate for cross-platform process data collection with automatic refresh capabilities, combined with fuzzy search for filtering and real-time updates via periodic API polling.

The existing infrastructure already has DuckDB for storage, Axum for the web server, and Tabulator for data tables. This phase extends the pattern established by journal log collection to process monitoring, using the same database and web server but with a new data collection module and API endpoints.

**Primary recommendation:** Use `sysinfo` 0.38.0 for process collection, `fuzzy-matcher` 0.3.7 with SkimMatcherV2 for fzf-style search, and periodic API polling (3-5 second intervals) for auto-refresh without WebSockets.

## Standard Stack

The established libraries/tools for this domain:

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| sysinfo | 0.38.0 | System/process information collection | De facto standard for Rust system monitoring - cross-platform, actively maintained, handles CPU/memory metrics with refresh patterns |
| fuzzy-matcher | 0.3.7 | Fuzzy string matching (fzf-style) | Implements SkimMatcherV2 algorithm matching fzf behavior, well-established in Rust ecosystem |
| chrono | 0.4 (existing) | Timestamp handling for process runtime | Already in project, needed for calculating process uptime |
| serde/serde_json | 1.0 (existing) | API response serialization | Already in project for log data |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tokio::time | 1.49 (existing) | Interval-based data collection | Background task for periodic process collection |
| axum | 0.8.8 (existing) | API endpoints for process data | Already serving log search, extend for processes |
| DuckDB | 1.4.3 (existing) | Storage for process snapshots | Store historical process data if needed (optional for phase 1) |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| sysinfo | procfs (Linux-only) | procfs gives raw /proc access but Linux-only, more complex to use, sysinfo abstracts platform differences |
| fuzzy-matcher | sublime_fuzzy | sublime_fuzzy is faster but different scoring - fzf users expect skim/fzf behavior |
| Periodic polling | WebSockets | WebSockets add complexity, polling is simpler for 3-5s refresh rates and matches existing pattern |

**Installation:**
```bash
cargo add sysinfo@0.38
cargo add fuzzy-matcher@0.3.7
# chrono, tokio, axum, serde already present
```

## Architecture Patterns

### Recommended Project Structure
```
src/
├── process_monitor.rs   # Process data collection module
├── web_server.rs        # Extend with new API endpoints
├── main.rs              # Add process monitoring to ApplicationController
└── lib.rs               # Export process_monitor module
```

### Pattern 1: Process Collection with Refresh
**What:** Initialize System once, refresh periodically for CPU/memory metrics
**When to use:** Any process monitoring - sysinfo requires previous state for CPU calculations
**Example:**
```rust
// Source: https://docs.rs/sysinfo/0.38.0/sysinfo/
use sysinfo::{System, ProcessRefreshKind, ProcessesToUpdate};

pub struct ProcessMonitor {
    sys: System,
}

impl ProcessMonitor {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        Self { sys }
    }

    pub fn refresh(&mut self) {
        // Refresh processes and CPU info
        self.sys.refresh_processes(ProcessesToUpdate::All, ProcessRefreshKind::everything());
        self.sys.refresh_cpu_usage();
        
        // Important: Sleep briefly to get accurate CPU readings
        // CPU% requires time delta between measurements
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
    }

    pub fn get_processes(&self) -> Vec<ProcessInfo> {
        self.sys.processes()
            .iter()
            .map(|(pid, process)| ProcessInfo {
                pid: pid.as_u32(),
                name: process.name().to_string(),
                cpu_percent: process.cpu_usage(),
                memory_bytes: process.memory(),
                user_id: process.user_id().map(|u| u.to_string()),
                runtime_secs: process.run_time(),
            })
            .collect()
    }
}
```

### Pattern 2: Fuzzy Search with SkimMatcherV2
**What:** Client-side or server-side fuzzy filtering of process list
**When to use:** User types search query, filter processes by name/user/PID
**Example:**
```rust
// Source: https://docs.rs/fuzzy-matcher/0.3.7/fuzzy_matcher/
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;

pub fn filter_processes(processes: &[ProcessInfo], query: &str) -> Vec<ProcessInfo> {
    if query.is_empty() {
        return processes.to_vec();
    }

    let matcher = SkimMatcherV2::default();
    
    processes.iter()
        .filter_map(|proc| {
            // Search across multiple fields
            let search_text = format!("{} {} {} {}", 
                proc.pid, proc.name, proc.user_id.as_deref().unwrap_or(""), proc.cpu_percent);
            
            matcher.fuzzy_match(&search_text, query)
                .map(|score| (proc.clone(), score))
        })
        .map(|(proc, _score)| proc)
        .collect()
}
```

### Pattern 3: Auto-Refresh via Periodic Polling
**What:** Frontend polls API endpoint every N seconds, updates table
**When to use:** Real-time monitoring without WebSocket complexity
**Example:**
```javascript
// Frontend auto-refresh pattern (already used in existing log UI)
let refreshInterval = 5000; // 5 seconds default
let autoRefreshEnabled = true;

function startAutoRefresh() {
    setInterval(async () => {
        if (!autoRefreshEnabled) return;
        
        const response = await fetch('/api/processes');
        const data = await response.json();
        updateTable(data.processes);
        updateTimestamp(data.timestamp);
    }, refreshInterval);
}
```

### Pattern 4: DuckDB Schema for Process History (Optional)
**What:** Store process snapshots for historical analysis
**When to use:** If showing "process appeared/disappeared" or trends over time
**Example:**
```rust
// Optional - not required for phase 1 but shows extension pattern
conn.execute(
    "CREATE TABLE IF NOT EXISTS process_snapshots (
        snapshot_time TIMESTAMP NOT NULL,
        pid INTEGER NOT NULL,
        name VARCHAR,
        cpu_percent REAL,
        memory_bytes BIGINT,
        user_id VARCHAR,
        runtime_secs BIGINT,
        PRIMARY KEY (snapshot_time, pid)
    )",
    [],
)?;

conn.execute(
    "CREATE INDEX IF NOT EXISTS idx_snapshot_time 
     ON process_snapshots(snapshot_time DESC)",
    [],
)?;
```

### Anti-Patterns to Avoid
- **Creating new System instance per refresh:** System holds state needed for CPU calculations - reuse it
- **Not sleeping between CPU measurements:** CPU% will be inaccurate without time delta
- **Blocking web server with process collection:** Run collection in background task, serve cached data
- **Per-process fuzzy matching:** Match against combined string, not individual fields (performance)

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Reading /proc filesystem | Custom /proc parsers | sysinfo crate | Cross-platform, handles edge cases (zombie processes, permission errors, missing fields), battle-tested |
| CPU percentage calculation | Manual tick counting | sysinfo's cpu_usage() | Requires tracking previous CPU time, handling wrap-around, normalizing across cores |
| Fuzzy string matching | Simple contains() | fuzzy-matcher SkimMatcherV2 | fzf-style scoring is complex (word boundaries, consecutive matches, case bonuses) |
| Process runtime formatting | Manual duration math | chrono::Duration | Handles edge cases, localization, formatting options |
| Real-time updates | Custom WebSocket | Periodic HTTP polling | Simpler, matches existing pattern, sufficient for 3-5s refresh |

**Key insight:** Process monitoring looks deceptively simple but has many edge cases - processes can disappear mid-read, CPU metrics need state, permissions vary by user. Using sysinfo abstracts all platform-specific details.

## Common Pitfalls

### Pitfall 1: Inaccurate CPU Percentages
**What goes wrong:** CPU% shows 0 or wildly incorrect values
**Why it happens:** sysinfo calculates CPU% as delta between two measurements - first call after refresh has no baseline
**How to avoid:** 
- Call `refresh_cpu_usage()` at least twice with sleep between (see MINIMUM_CPU_UPDATE_INTERVAL)
- Initialize System at startup, keep it alive
- Document that first snapshot may show 0% CPU
**Warning signs:** All processes show 0% CPU, or values fluctuate wildly

### Pitfall 2: Blocking Web Server Thread
**What goes wrong:** Web UI becomes unresponsive during process refresh
**Why it happens:** `System::refresh_processes()` can take 50-200ms with hundreds of processes
**How to avoid:**
- Run process collection in background tokio task
- Store latest snapshot in Arc<Mutex<>> or similar
- API endpoint serves cached snapshot, not live collection
**Warning signs:** HTTP requests timeout, UI freezes during refresh

### Pitfall 3: Memory Leaks from Dead Processes
**What goes wrong:** Process list keeps growing, memory usage increases
**Why it happens:** sysinfo keeps dead process info until explicitly refreshed with correct update strategy
**How to avoid:**
- Use `ProcessesToUpdate::All` to get current process list
- Don't accumulate processes yourself - rely on sysinfo's snapshot
**Warning signs:** Process count never decreases, memory grows over time

### Pitfall 4: Search Debounce Timing
**What goes wrong:** Either too laggy (slow debounce) or too many requests (fast debounce)
**Why it happens:** Balancing responsiveness vs server load
**How to avoid:**
- Start with 300-500ms debounce (industry standard)
- Make it configurable via constant
- Consider doing fuzzy match client-side for small process lists (<1000 processes)
**Warning signs:** User complaints about lag, or excessive API requests in logs

### Pitfall 5: Runtime Calculation Errors
**What goes wrong:** Process runtime shows negative or overflow values
**Why it happens:** sysinfo's `run_time()` returns seconds since process start - can be very large for long-running processes
**How to avoid:**
- Use u64 for runtime storage
- Format display as days/hours/minutes, not just seconds
- Handle boot time changes (uptime counter resets)
**Warning signs:** Negative runtime, runtime jumps unexpectedly

### Pitfall 6: Cross-Platform Assumptions
**What goes wrong:** Code works on Linux but fails on macOS/Windows
**Why it happens:** Some fields (user_id, group_id) may not be available on all platforms
**How to avoid:**
- Check `sysinfo::IS_SUPPORTED_SYSTEM` 
- Use Option<> for platform-specific fields
- Test on target platforms or use defaults
**Warning signs:** panic on process.user_id().unwrap() on non-Linux systems

## Code Examples

Verified patterns from official sources:

### Complete ProcessMonitor Module
```rust
// Combining official sysinfo patterns with existing project structure
use serde::{Deserialize, Serialize};
use sysinfo::{System, Pid, ProcessRefreshKind, ProcessesToUpdate};
use std::sync::{Arc, Mutex};
use tokio::time::{interval, Duration};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu_percent: f32,
    pub memory_bytes: u64,
    pub user_id: Option<String>,
    pub runtime_secs: u64,
}

pub struct ProcessMonitor {
    system: Arc<Mutex<System>>,
    snapshot: Arc<Mutex<Vec<ProcessInfo>>>,
}

impl ProcessMonitor {
    pub fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_all();
        
        Self {
            system: Arc::new(Mutex::new(system)),
            snapshot: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Start background collection task (run once at startup)
    pub fn start_collection(&self, interval_secs: u64) {
        let system = self.system.clone();
        let snapshot = self.snapshot.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(interval_secs));
            
            loop {
                interval.tick().await;
                
                let mut sys = system.lock().unwrap();
                sys.refresh_processes(ProcessesToUpdate::All, ProcessRefreshKind::everything());
                sys.refresh_cpu_usage();
                
                // Collect snapshot
                let processes: Vec<ProcessInfo> = sys.processes()
                    .iter()
                    .map(|(pid, process)| ProcessInfo {
                        pid: pid.as_u32(),
                        name: process.name().to_string_lossy().to_string(),
                        cpu_percent: process.cpu_usage(),
                        memory_bytes: process.memory(),
                        user_id: process.user_id().map(|u| format!("{:?}", u)),
                        runtime_secs: process.run_time(),
                    })
                    .collect();

                *snapshot.lock().unwrap() = processes;
            }
        });
    }

    /// Get current process snapshot (called by API handler)
    pub fn get_snapshot(&self) -> Vec<ProcessInfo> {
        self.snapshot.lock().unwrap().clone()
    }
}
```

### API Endpoint Handler
```rust
// Extend web_server.rs with process endpoint (following existing pattern)
use axum::{Json, extract::State};

#[derive(Serialize)]
pub struct ProcessResponse {
    pub processes: Vec<ProcessInfo>,
    pub timestamp: String,
    pub total: usize,
}

async fn api_processes(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ProcessResponse>, (StatusCode, String)> {
    let processes = state.process_monitor.get_snapshot();
    let total = processes.len();

    Ok(Json(ProcessResponse {
        processes,
        timestamp: chrono::Utc::now().to_rfc3339(),
        total,
    }))
}
```

### Frontend Debounced Search
```javascript
// Client-side fuzzy search with debounce
let debounceTimer;
const DEBOUNCE_MS = 300;

document.getElementById('search-input').addEventListener('input', (e) => {
    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => {
        filterProcesses(e.target.value);
    }, DEBOUNCE_MS);
});

function filterProcesses(query) {
    if (!query) {
        table.setData(allProcesses);
        return;
    }
    
    // Simple client-side filter - could use fuzzy-matcher on server for large lists
    const filtered = allProcesses.filter(proc => 
        proc.name.toLowerCase().includes(query.toLowerCase()) ||
        proc.pid.toString().includes(query) ||
        (proc.user_id && proc.user_id.toLowerCase().includes(query.toLowerCase()))
    );
    
    table.setData(filtered);
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Manual /proc parsing | sysinfo crate | ~2018 | Cross-platform support, much simpler code |
| WebSockets for real-time | Server-Sent Events or polling | ~2020 | Simpler for read-only data, less overhead |
| String::contains() search | Fuzzy matching (fzf/skim) | ~2019 | Better UX - users can type partial/misordered matches |
| Individual CPU core metrics | Aggregate CPU% per process | Always | More relevant for monitoring (total resource usage) |

**Deprecated/outdated:**
- procinfo crate: Unmaintained, use sysinfo instead
- psutil (Python port): Incomplete, use native sysinfo
- Direct /proc reads: Platform-specific, fragile

## Open Questions

Things that couldn't be fully resolved:

1. **Storage strategy for process history**
   - What we know: DuckDB can store snapshots, existing schema pattern works
   - What's unclear: Performance impact of storing every refresh (potentially 100s of processes every 3-5 seconds)
   - Recommendation: Start with in-memory only (no storage), add storage in later phase if historical analysis is needed. If storing, use minute-level aggregation like log collection.

2. **Client-side vs server-side fuzzy search**
   - What we know: fuzzy-matcher works server-side, client-side is simpler for small datasets
   - What's unclear: Performance threshold - when does client-side become too slow?
   - Recommendation: Start with server-side for consistency with log search pattern. Reconsider if process count is consistently <500.

3. **Refresh interval configurability**
   - What we know: User wants configurable refresh in UI
   - What's unclear: Should backend collection interval change, or just frontend polling?
   - Recommendation: Backend collects at fixed 5s interval (balance accuracy vs overhead), frontend polls at user-configured rate (3-30s range). Backend can always serve latest snapshot.

## Sources

### Primary (HIGH confidence)
- https://docs.rs/sysinfo/0.38.0/sysinfo/ - Official sysinfo documentation, API examples
- https://docs.rs/fuzzy-matcher/0.3.7/fuzzy_matcher/ - Official fuzzy-matcher documentation
- Existing codebase: src/web_server.rs, src/duckdb_buffer.rs - Established patterns for API and storage

### Secondary (MEDIUM confidence)
- sysinfo examples directory (via docs.rs) - Real-world usage patterns
- Tabulator table library (already in use) - Column sorting, data display

### Tertiary (LOW confidence - noted for validation)
- Refresh interval best practices: 3-5 seconds is industry standard for process monitoring (htop uses 3s default)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - sysinfo is de facto standard, well-documented, mature (v0.38)
- Architecture: HIGH - Patterns verified from official docs and existing project structure
- Pitfalls: MEDIUM - Based on sysinfo docs notes and common monitoring issues, not firsthand experience

**Research date:** 2026-02-02
**Valid until:** 2026-03-02 (30 days - sysinfo is stable, slow-moving crate)
