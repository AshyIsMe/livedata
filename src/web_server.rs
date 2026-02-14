use crate::config::Settings;
use crate::duckdb_buffer::{DuckDBBuffer, ProcessMetricRecord};
use crate::process_monitor::ProcessMonitor;
use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::get,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Application state shared across handlers
pub struct AppState {
    pub data_dir: String,
    pub buffer: Arc<Mutex<DuckDBBuffer>>,
    pub process_monitor: Arc<ProcessMonitor>,
    pub settings: Settings,
}

impl AppState {
    pub fn new(
        data_dir: &str,
        buffer: Arc<Mutex<DuckDBBuffer>>,
        process_monitor: Arc<ProcessMonitor>,
        settings: Settings,
    ) -> Self {
        Self {
            data_dir: data_dir.to_string(),
            buffer,
            process_monitor,
            settings,
        }
    }
}

/// Search parameters from query string
#[derive(Debug, Deserialize)]
pub struct SearchParams {
    /// Text search (MESSAGE field, case-insensitive ILIKE)
    #[serde(default)]
    pub q: Option<String>,
    /// Start time (ISO 8601 or relative: -1h, -15m, -7d)
    #[serde(default = "default_start")]
    pub start: String,
    /// End time (ISO 8601 or "now")
    #[serde(default = "default_end")]
    pub end: String,
    /// Filter by hostname (comma-separated)
    #[serde(default)]
    pub hostname: Option<String>,
    /// Filter by systemd unit (comma-separated)
    #[serde(default)]
    pub unit: Option<String>,
    /// Max priority level (0-7, lower = more severe)
    #[serde(default)]
    pub priority: Option<u8>,
    /// Results per page (default: 100, max: 100000)
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Pagination offset
    #[serde(default)]
    pub offset: usize,
    /// Sort column (timestamp, hostname, unit, priority, comm)
    #[serde(default = "default_sort")]
    pub sort: String,
    /// Sort direction (asc or desc)
    #[serde(default = "default_sort_dir")]
    pub sort_dir: String,
    /// Comma-separated list of columns to include
    #[serde(default)]
    pub columns: Option<String>,
}

fn default_start() -> String {
    "-1h".to_string()
}

fn default_end() -> String {
    "now".to_string()
}

fn default_limit() -> usize {
    1_000
}

fn default_sort() -> String {
    "timestamp".to_string()
}

fn default_sort_dir() -> String {
    "desc".to_string()
}

/// Search result entry (used for default columns)
#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResult {
    pub timestamp: String,
    pub hostname: Option<String>,
    pub unit: Option<String>,
    pub priority: Option<i32>,
    pub pid: Option<String>,
    pub comm: Option<String>,
    pub message: Option<String>,
}

/// Search response with dynamic results
#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResponse {
    pub results: Vec<serde_json::Value>,
    pub columns: Vec<String>,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
    pub query_time_ms: u128,
}

/// Column info for /api/columns endpoint
#[derive(Debug, Serialize, Deserialize)]
pub struct ColumnInfo {
    pub name: String,
    pub column_type: String,
    pub default: bool,
}

/// Internal columns to exclude from the column chooser
const EXCLUDED_COLUMNS: &[&str] = &["__CURSOR", "__MONOTONIC_TIMESTAMP", "minute_key"];

/// Default columns shown when no column selection is made
const DEFAULT_COLUMNS: &[&str] = &[
    "timestamp",
    "_hostname",
    "_systemd_unit",
    "priority",
    "_pid",
    "_comm",
    "message",
];

/// Filter values response
#[derive(Debug, Serialize, Deserialize)]
pub struct FilterValues {
    pub hostnames: Vec<String>,
    pub units: Vec<String>,
    pub priorities: Vec<PriorityOption>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PriorityOption {
    pub value: u8,
    pub label: String,
}

/// Health check response
#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub data_dir: String,
}

/// Process list API response
#[derive(Debug, Serialize)]
pub struct ProcessResponse {
    pub processes: Vec<ProcessMetricsRow>,
    pub timestamp: String,
    pub total: usize,
}

/// Process metrics row for API response (aligned with process_metrics table)
#[derive(Debug, Serialize)]
pub struct ProcessMetricsRow {
    pub timestamp: String,
    pub pid: u32,
    pub name: String,
    pub cpu_usage: f32,
    pub mem_usage: f64,
    pub user: Option<String>,
    pub runtime: u64,
}

fn to_process_row(r: ProcessMetricRecord) -> ProcessMetricsRow {
    ProcessMetricsRow {
        timestamp: r.timestamp,
        pid: r.pid,
        name: r.name,
        cpu_usage: r.cpu_usage,
        mem_usage: r.mem_usage,
        user: r.user,
        runtime: r.runtime,
    }
}

/// Storage health API response
#[derive(Debug, Serialize)]
pub struct StorageHealthResponse {
    pub database_size_bytes: u64,
    pub journal_log_count: i64,
    pub process_metric_count: i64,
    pub oldest_log_timestamp: Option<String>,
    pub newest_log_timestamp: Option<String>,
    pub retention_policy: RetentionPolicy,
}

#[derive(Debug, Serialize)]
pub struct RetentionPolicy {
    pub log_retention_days: u32,
    pub log_max_size_gb: f64,
    pub process_retention_days: u32,
    pub process_max_size_gb: f64,
}

/// Parse time string (ISO 8601 or relative like -1h, -15m, -7d)
fn parse_time(s: &str, now: DateTime<Utc>) -> Result<DateTime<Utc>, String> {
    if s == "now" {
        return Ok(now);
    }

    // Try relative time first
    if let Some(s) = s.strip_prefix('-') {
        let (num_str, unit) = if let Some(n) = s.strip_suffix('d') {
            (n, 'd')
        } else if let Some(n) = s.strip_suffix('h') {
            (n, 'h')
        } else if let Some(n) = s.strip_suffix('m') {
            (n, 'm')
        } else if let Some(n) = s.strip_suffix('s') {
            (n, 's')
        } else {
            return Err(format!("Invalid relative time format: -{}", s));
        };

        let num: i64 = num_str
            .parse()
            .map_err(|_| format!("Invalid number in relative time: {}", num_str))?;

        let duration = match unit {
            'd' => Duration::days(num),
            'h' => Duration::hours(num),
            'm' => Duration::minutes(num),
            's' => Duration::seconds(num),
            _ => unreachable!(),
        };

        return Ok(now - duration);
    }

    // Try ISO 8601
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| format!("Invalid time format: {} ({})", s, e))
}

/// Escape LIKE wildcards for safe SQL queries
fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// Get priority label
fn priority_label(p: u8) -> &'static str {
    match p {
        0 => "Emergency",
        1 => "Alert",
        2 => "Critical",
        3 => "Error",
        4 => "Warning",
        5 => "Notice",
        6 => "Info",
        7 => "Debug",
        _ => "Unknown",
    }
}

pub async fn run_web_server(
    data_dir: &str,
    buffer: Arc<Mutex<DuckDBBuffer>>,
    shutdown_signal: Arc<AtomicBool>,
    process_monitor: Arc<ProcessMonitor>,
    settings: Settings,
) {
    let state = Arc::new(AppState::new(data_dir, buffer, process_monitor, settings));

    let app = Router::new()
        .route("/", get(search_ui))
        .route("/api/search", get(api_search))
        .route("/api/columns", get(api_columns))
        .route("/api/filters", get(api_filters))
        .route("/api/processes", get(api_processes))
        .route("/api/storage/health", get(api_storage_health))
        .route("/health", get(health))
        // Static file routes for process monitoring UI
        .route("/index.html", get(serve_index_html))
        .route("/processes.html", get(serve_processes_html))
        .route("/processes.js", get(serve_processes_js))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    log::info!("Web server listening on {}", listener.local_addr().unwrap());

    // Run axum server with graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            // Poll the shutdown signal
            loop {
                if shutdown_signal.load(Ordering::Relaxed) {
                    log::info!("Web server received shutdown signal");
                    break;
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        })
        .await
        .unwrap();
}

/// Health check endpoint
async fn health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(HealthResponse {
        status: "ok".to_string(),
        data_dir: state.data_dir.clone(),
    })
}

/// Serve index.html static file
async fn serve_index_html() -> impl IntoResponse {
    match tokio::fs::read_to_string("static/index.html").await {
        Ok(content) => Html(content).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "index.html not found").into_response(),
    }
}

/// Serve processes.html static file
async fn serve_processes_html() -> impl IntoResponse {
    match tokio::fs::read_to_string("static/processes.html").await {
        Ok(content) => Html(content).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "processes.html not found").into_response(),
    }
}

/// Serve processes.js static file
async fn serve_processes_js() -> impl IntoResponse {
    match tokio::fs::read_to_string("static/processes.js").await {
        Ok(content) => ([("content-type", "application/javascript")], content).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "processes.js not found").into_response(),
    }
}

/// API endpoint returning current process snapshot
async fn api_processes(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ProcessResponse>, (StatusCode, String)> {
    let latest_timestamp = state
        .buffer
        .lock()
        .unwrap()
        .get_latest_process_timestamp()
        .ok()
        .flatten();

    if let Some(latest_timestamp) = latest_timestamp {
        let rows = state
            .buffer
            .lock()
            .unwrap()
            .get_process_metrics_for_timestamp(&latest_timestamp)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let processes: Vec<ProcessMetricsRow> = rows.into_iter().map(to_process_row).collect();

        let total = processes.len();
        return Ok(Json(ProcessResponse {
            processes,
            timestamp: latest_timestamp,
            total,
        }));
    }

    let snapshot = state.process_monitor.get_snapshot();
    let timestamp = chrono::Utc::now().to_rfc3339();
    let processes: Vec<ProcessMetricsRow> = snapshot
        .into_iter()
        .map(|process| {
            let user = process.user_id.as_ref().and_then(|uid_str| {
                uid_str
                    .strip_prefix("Uid(")
                    .and_then(|s| s.strip_suffix(')'))
                    .map(|s| s.to_string())
            });

            ProcessMetricsRow {
                timestamp: timestamp.clone(),
                pid: process.pid,
                name: process.name,
                cpu_usage: process.cpu_percent,
                mem_usage: process.memory_bytes as f64,
                user,
                runtime: process.runtime_secs,
            }
        })
        .collect();
    let total = processes.len();

    Ok(Json(ProcessResponse {
        processes,
        timestamp,
        total,
    }))
}

/// API endpoint returning storage health and statistics
async fn api_storage_health(
    State(state): State<Arc<AppState>>,
) -> Result<Json<StorageHealthResponse>, (StatusCode, String)> {
    // Get database file size
    let db_path = std::path::Path::new(&state.data_dir).join("livedata.duckdb");
    let database_size_bytes = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);

    let stats = state
        .buffer
        .lock()
        .unwrap()
        .get_storage_stats()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let retention_policy = RetentionPolicy {
        log_retention_days: state.settings.log_retention_days,
        log_max_size_gb: state.settings.log_max_size_gb,
        process_retention_days: state.settings.process_retention_days,
        process_max_size_gb: state.settings.process_max_size_gb,
    };

    Ok(Json(StorageHealthResponse {
        database_size_bytes,
        journal_log_count: stats.journal_log_count,
        process_metric_count: stats.process_metric_count,
        oldest_log_timestamp: stats.oldest_log_timestamp,
        newest_log_timestamp: stats.newest_log_timestamp,
        retention_policy,
    }))
}

/// Get valid column names from the journal_logs schema
fn get_schema_columns(buffer: &Arc<Mutex<DuckDBBuffer>>) -> Vec<(String, String)> {
    buffer.lock().unwrap().get_schema_columns()
}

/// Validate requested columns against the actual schema, returning SQL expressions
fn validate_columns(requested: &[&str], schema: &[(String, String)]) -> Vec<String> {
    let schema_names: std::collections::HashSet<&str> =
        schema.iter().map(|(name, _)| name.as_str()).collect();
    requested
        .iter()
        .filter(|col| schema_names.contains(**col))
        .map(|col| {
            // Cast certain columns for display
            match *col {
                "timestamp" => "CAST(timestamp AS VARCHAR)".to_string(),
                "_pid" => "CAST(_pid AS VARCHAR)".to_string(),
                other => other.to_string(),
            }
        })
        .collect()
}

/// Column alias for display (strips leading underscore, etc.)
fn column_display_name(col: &str) -> String {
    match col {
        "CAST(timestamp AS VARCHAR)" | "timestamp" => "timestamp".to_string(),
        "CAST(_pid AS VARCHAR)" | "_pid" => "pid".to_string(),
        "_hostname" => "hostname".to_string(),
        "_systemd_unit" => "unit".to_string(),
        "_comm" => "comm".to_string(),
        other => other.to_string(),
    }
}

/// API columns endpoint returning available columns
async fn api_columns(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ColumnInfo>>, (StatusCode, String)> {
    let schema = get_schema_columns(&state.buffer);

    let columns: Vec<ColumnInfo> = schema
        .iter()
        .filter(|(name, _)| !EXCLUDED_COLUMNS.contains(&name.as_str()))
        .map(|(name, col_type)| ColumnInfo {
            name: name.clone(),
            column_type: col_type.clone(),
            default: DEFAULT_COLUMNS.contains(&name.as_str()),
        })
        .collect();

    Ok(Json(columns))
}

/// API search endpoint returning JSON results
async fn api_search(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchParams>,
) -> Result<Json<SearchResponse>, (StatusCode, String)> {
    let start_time = std::time::Instant::now();
    let now = Utc::now();

    // Parse time range
    let start = parse_time(&params.start, now).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let end = parse_time(&params.end, now).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    // Validate and clamp limit
    let limit = params.limit.min(100_000);

    // Determine which columns to select
    let schema = get_schema_columns(&state.buffer);

    // If the table doesn't exist yet, return empty results
    if schema.is_empty() {
        return Ok(Json(SearchResponse {
            results: Vec::new(),
            columns: DEFAULT_COLUMNS
                .iter()
                .map(|c| column_display_name(c))
                .collect(),
            total: 0,
            limit,
            offset: params.offset,
            query_time_ms: start_time.elapsed().as_millis(),
        }));
    }

    let requested_cols: Vec<&str> = if let Some(ref cols) = params.columns
        && !cols.is_empty()
    {
        cols.split(',').map(|s| s.trim()).collect()
    } else {
        DEFAULT_COLUMNS.to_vec()
    };
    let select_exprs = validate_columns(&requested_cols, &schema);
    if select_exprs.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "No valid columns specified".into()));
    }

    // Build display names for the response
    let display_names: Vec<String> = select_exprs
        .iter()
        .map(|e| column_display_name(e))
        .collect();

    // Build SQL query against the journal_logs table
    let mut sql = format!(
        "SELECT {} FROM journal_logs WHERE timestamp >= '{}' AND timestamp < '{}'",
        select_exprs.join(", "),
        start.to_rfc3339(),
        end.to_rfc3339()
    );

    // Add text search filter
    if let Some(ref q) = params.q
        && !q.is_empty()
    {
        let escaped = escape_like(q);
        sql.push_str(&format!(" AND message ILIKE '%{}%' ESCAPE '\\'", escaped));
    }

    // Add hostname filter
    if let Some(ref hostname) = params.hostname
        && !hostname.is_empty()
    {
        let hosts: Vec<&str> = hostname.split(',').collect();
        let host_list: Vec<String> = hosts
            .iter()
            .map(|h| format!("'{}'", h.replace('\'', "''")))
            .collect();
        sql.push_str(&format!(" AND _hostname IN ({})", host_list.join(",")));
    }

    // Add unit filter
    if let Some(ref unit) = params.unit
        && !unit.is_empty()
    {
        let units: Vec<&str> = unit.split(',').collect();
        let unit_list: Vec<String> = units
            .iter()
            .map(|u| format!("'{}'", u.replace('\'', "''")))
            .collect();
        sql.push_str(&format!(" AND _systemd_unit IN ({})", unit_list.join(",")));
    }

    // Add priority filter
    if let Some(priority) = params.priority {
        sql.push_str(&format!(" AND CAST(priority AS INTEGER) <= {}", priority));
    }

    // Validate and build ORDER BY clause
    let sort_column = match params.sort.to_lowercase().as_str() {
        "timestamp" => "timestamp",
        "hostname" | "host" => "_hostname",
        "unit" => "_systemd_unit",
        "priority" | "pri" => "priority",
        "comm" => "_comm",
        _ => "timestamp",
    };

    let sort_direction = match params.sort_dir.to_lowercase().as_str() {
        "asc" => "ASC",
        "desc" => "DESC",
        _ => "DESC",
    };

    sql.push_str(&format!(
        " ORDER BY {} {} LIMIT {} OFFSET {}",
        sort_column, sort_direction, limit, params.offset
    ));

    // Execute query with dynamic column mapping
    let results: Vec<serde_json::Value> = state
        .buffer
        .lock()
        .unwrap()
        .query_json_rows(&sql, &display_names)
        .unwrap_or_default();

    let total = results.len();
    let query_time_ms = start_time.elapsed().as_millis();

    Ok(Json(SearchResponse {
        results,
        columns: display_names,
        total,
        limit,
        offset: params.offset,
        query_time_ms,
    }))
}

/// API filters endpoint returning available filter values
async fn api_filters(
    State(state): State<Arc<AppState>>,
) -> Result<Json<FilterValues>, (StatusCode, String)> {
    // Get distinct hostnames
    let hostnames = state.buffer.lock().unwrap().query_distinct_strings(
        "SELECT DISTINCT _hostname FROM journal_logs WHERE _hostname IS NOT NULL ORDER BY _hostname",
    );

    // Get distinct units
    let units = state.buffer.lock().unwrap().query_distinct_strings(
        "SELECT DISTINCT _systemd_unit FROM journal_logs WHERE _systemd_unit IS NOT NULL ORDER BY _systemd_unit",
    );

    // Static priority options
    let priorities: Vec<PriorityOption> = (0..=7)
        .map(|p| PriorityOption {
            value: p,
            label: format!("{} - {}", p, priority_label(p)),
        })
        .collect();

    Ok(Json(FilterValues {
        hostnames,
        units,
        priorities,
    }))
}

/// Main search UI (HTML)
async fn search_ui(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchParams>,
) -> impl IntoResponse {
    let now = Utc::now();

    // Parse time range for display
    let start = parse_time(&params.start, now).unwrap_or(now - Duration::hours(1));
    let end = parse_time(&params.end, now).unwrap_or(now);

    let limit = params.limit.min(100_000);

    // Determine which columns to select
    let schema = get_schema_columns(&state.buffer);
    let requested_cols: Vec<&str> = if let Some(ref cols) = params.columns
        && !cols.is_empty()
    {
        cols.split(',').map(|s| s.trim()).collect()
    } else {
        DEFAULT_COLUMNS.to_vec()
    };
    let select_exprs = validate_columns(&requested_cols, &schema);
    let select_list = if select_exprs.is_empty() {
        DEFAULT_COLUMNS
            .iter()
            .filter_map(|c| {
                let exprs = validate_columns(&[c], &schema);
                exprs.into_iter().next()
            })
            .collect::<Vec<_>>()
    } else {
        select_exprs
    };
    let display_names: Vec<String> = select_list.iter().map(|e| column_display_name(e)).collect();

    // Get all available columns for the column chooser
    let all_columns: Vec<ColumnInfo> = schema
        .iter()
        .filter(|(name, _)| !EXCLUDED_COLUMNS.contains(&name.as_str()))
        .map(|(name, col_type)| ColumnInfo {
            name: name.clone(),
            column_type: col_type.clone(),
            default: DEFAULT_COLUMNS.contains(&name.as_str()),
        })
        .collect();

    let mut sql = format!(
        "SELECT {} FROM journal_logs WHERE timestamp >= '{}' AND timestamp < '{}'",
        select_list.join(", "),
        start.to_rfc3339(),
        end.to_rfc3339()
    );

    // Add text search filter
    if let Some(ref q) = params.q
        && !q.is_empty()
    {
        let escaped = escape_like(q);
        sql.push_str(&format!(" AND message ILIKE '%{}%' ESCAPE '\\'", escaped));
    }

    // Add hostname filter
    if let Some(ref hostname) = params.hostname
        && !hostname.is_empty()
    {
        let hosts: Vec<&str> = hostname.split(',').collect();
        let host_list: Vec<String> = hosts
            .iter()
            .map(|h| format!("'{}'", h.replace('\'', "''")))
            .collect();
        sql.push_str(&format!(" AND _hostname IN ({})", host_list.join(",")));
    }

    // Add unit filter
    if let Some(ref unit) = params.unit
        && !unit.is_empty()
    {
        let units: Vec<&str> = unit.split(',').collect();
        let unit_list: Vec<String> = units
            .iter()
            .map(|u| format!("'{}'", u.replace('\'', "''")))
            .collect();
        sql.push_str(&format!(" AND _systemd_unit IN ({})", unit_list.join(",")));
    }

    // Add priority filter
    if let Some(priority) = params.priority {
        sql.push_str(&format!(" AND CAST(priority AS INTEGER) <= {}", priority));
    }

    // Count total results
    let count_sql = format!(
        "SELECT COUNT(*) FROM journal_logs WHERE {}",
        sql.split("WHERE ")
            .nth(1)
            .unwrap_or("1=1")
            .split(" ORDER BY")
            .next()
            .unwrap_or("1=1")
    );

    // Validate and build ORDER BY clause
    let sort_column = match params.sort.to_lowercase().as_str() {
        "timestamp" => "timestamp",
        "hostname" | "host" => "_hostname",
        "unit" => "_systemd_unit",
        "priority" | "pri" => "priority",
        "comm" => "_comm",
        _ => "timestamp",
    };

    let sort_direction = match params.sort_dir.to_lowercase().as_str() {
        "asc" => "ASC",
        "desc" => "DESC",
        _ => "DESC",
    };

    sql.push_str(&format!(
        " ORDER BY {} {} LIMIT {} OFFSET {}",
        sort_column, sort_direction, limit, params.offset
    ));

    // Get total count
    let total_count = state.buffer.lock().unwrap().query_usize(&count_sql);

    // Execute main query with dynamic columns
    let results = state
        .buffer
        .lock()
        .unwrap()
        .query_json_rows(&sql, &display_names)
        .unwrap_or_default();

    // Build histogram queries for both minute and hour granularity
    let mut hist_where = format!(
        "FROM journal_logs WHERE timestamp >= '{}' AND timestamp < '{}'",
        start.to_rfc3339(),
        end.to_rfc3339()
    );

    if let Some(ref q) = params.q
        && !q.is_empty()
    {
        let escaped = escape_like(q);
        hist_where.push_str(&format!(" AND message ILIKE '%{}%' ESCAPE '\\'", escaped));
    }
    if let Some(ref hostname) = params.hostname
        && !hostname.is_empty()
    {
        let hosts: Vec<&str> = hostname.split(',').collect();
        let host_list: Vec<String> = hosts
            .iter()
            .map(|h| format!("'{}'", h.replace('\'', "''")))
            .collect();
        hist_where.push_str(&format!(" AND _hostname IN ({})", host_list.join(",")));
    }
    if let Some(ref unit) = params.unit
        && !unit.is_empty()
    {
        let units: Vec<&str> = unit.split(',').collect();
        let unit_list: Vec<String> = units
            .iter()
            .map(|u| format!("'{}'", u.replace('\'', "''")))
            .collect();
        hist_where.push_str(&format!(" AND _systemd_unit IN ({})", unit_list.join(",")));
    }
    if let Some(priority) = params.priority {
        hist_where.push_str(&format!(" AND CAST(priority AS INTEGER) <= {}", priority));
    }

    let mut histogram_both: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();

    for bin_unit in &["minute", "hour"] {
        let hist_sql = format!(
            "SELECT CAST(date_trunc('{}', timestamp) AS VARCHAR) as bin, COUNT(*) as count {} GROUP BY bin ORDER BY bin",
            bin_unit, hist_where
        );

        let rows = state.buffer.lock().unwrap().query_histogram_rows(&hist_sql);

        histogram_both.insert(bin_unit.to_string(), serde_json::Value::Array(rows));
    }

    let histogram_json = serde_json::to_string(&histogram_both)
        .unwrap_or_else(|_| "{}".to_string())
        .replace("</", "<\\/");

    // Pick default bin unit based on range
    let range_seconds = (end - start).num_seconds();
    let default_bin = if range_seconds <= 7200 {
        "minute"
    } else {
        "hour"
    };

    // Get filter options
    let hostnames = state.buffer.lock().unwrap().query_distinct_strings(
        "SELECT DISTINCT _hostname FROM journal_logs WHERE _hostname IS NOT NULL ORDER BY _hostname",
    );

    let units = state.buffer.lock().unwrap().query_distinct_strings(
        "SELECT DISTINCT _systemd_unit FROM journal_logs WHERE _systemd_unit IS NOT NULL ORDER BY _systemd_unit",
    );

    // Build HTML
    let start_iso = start.to_rfc3339();
    let end_iso = end.to_rfc3339();
    let html = build_search_html(
        &params,
        &results,
        &display_names,
        &all_columns,
        total_count,
        &hostnames,
        &units,
        &histogram_json,
        default_bin,
        &start_iso,
        &end_iso,
    );

    Html(html)
}

#[allow(clippy::too_many_arguments)]
fn build_search_html(
    params: &SearchParams,
    results: &[serde_json::Value],
    display_names: &[String],
    all_columns: &[ColumnInfo],
    total_count: usize,
    hostnames: &[String],
    units: &[String],
    histogram_json: &str,
    bin_unit: &str,
    start_iso: &str,
    end_iso: &str,
) -> String {
    let query_value = params.q.as_deref().unwrap_or("");
    let hostname_value = params.hostname.as_deref().unwrap_or("");
    let unit_value = params.unit.as_deref().unwrap_or("");
    let priority_value = params.priority;

    // Build hostname options
    let hostname_options: String = hostnames
        .iter()
        .map(|h| {
            let selected = if hostname_value == h { " selected" } else { "" };
            format!(
                "<option value=\"{}\"{}>{}</option>",
                html_escape(h),
                selected,
                html_escape(h)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Build unit options
    let unit_options: String = units
        .iter()
        .map(|u| {
            let selected = if unit_value == u { " selected" } else { "" };
            format!(
                "<option value=\"{}\"{}>{}</option>",
                html_escape(u),
                selected,
                html_escape(u)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Build priority options
    let priority_options: String = (0..=7)
        .map(|p| {
            let selected = if priority_value == Some(p) {
                " selected"
            } else {
                ""
            };
            format!(
                "<option value=\"{}\"{}>{} - {}</option>",
                p,
                selected,
                p,
                priority_label(p)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Calculate pagination
    let limit = params.limit.min(100_000);
    let current_page = params.offset / limit + 1;
    let total_pages = total_count.div_ceil(limit);

    // Build pagination links
    let pagination = if total_pages > 1 {
        let mut links = Vec::new();

        // Previous link
        if current_page > 1 {
            let prev_offset = (current_page - 2) * limit;
            links.push(format!(
                r#"<a href="?q={}&start={}&end={}&hostname={}&unit={}{}&limit={}&offset={}&sort={}&sort_dir={}" class="page-link">&laquo; Prev</a>"#,
                url_encode(query_value),
                url_encode(&params.start),
                url_encode(&params.end),
                url_encode(hostname_value),
                url_encode(unit_value),
                priority_value.map(|p| format!("&priority={}", p)).unwrap_or_default(),
                limit,
                prev_offset,
                url_encode(&params.sort),
                url_encode(&params.sort_dir),
            ));
        }

        // Page info
        links.push(format!(
            "<span class=\"page-info\">Page {} of {} ({} results)</span>",
            current_page, total_pages, total_count
        ));

        // Next link
        if current_page < total_pages {
            let next_offset = current_page * limit;
            links.push(format!(
                r#"<a href="?q={}&start={}&end={}&hostname={}&unit={}{}&limit={}&offset={}&sort={}&sort_dir={}" class="page-link">Next &raquo;</a>"#,
                url_encode(query_value),
                url_encode(&params.start),
                url_encode(&params.end),
                url_encode(hostname_value),
                url_encode(unit_value),
                priority_value.map(|p| format!("&priority={}", p)).unwrap_or_default(),
                limit,
                next_offset,
                url_encode(&params.sort),
                url_encode(&params.sort_dir),
            ));
        }

        format!("<div class=\"pagination\">{}</div>", links.join(" "))
    } else {
        format!(
            "<div class=\"pagination\"><span class=\"page-info\">{} results</span></div>",
            total_count
        )
    };

    // Serialize results as JSON for Tabulator
    let results_json = serde_json::to_string(results)
        .unwrap_or_else(|_| "[]".to_string())
        .replace("</", "<\\/");

    // Serialize column info and active display names for the frontend
    let columns_json = serde_json::to_string(all_columns)
        .unwrap_or_else(|_| "[]".to_string())
        .replace("</", "<\\/");
    let active_columns_json = serde_json::to_string(display_names)
        .unwrap_or_else(|_| "[]".to_string())
        .replace("</", "<\\/");

    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Livedata - Log Search</title>
    <link href="https://unpkg.com/tabulator-tables@6.3.1/dist/css/tabulator_midnight.min.css" rel="stylesheet">
    <script src="https://cdn.jsdelivr.net/npm/chart.js@4/dist/chart.umd.min.js"></script>
    <style>
        * {{
            box-sizing: border-box;
            margin: 0;
            padding: 0;
        }}
        body {{
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
            background-color: #1a1a2e;
            color: #eee;
            line-height: 1.5;
        }}
        .global-header {{
            background-color: #0f3460;
            border-bottom: 2px solid #00d9ff;
            margin-bottom: 0;
        }}
        .global-header nav {{
            display: flex;
            gap: 5px;
            padding: 12px 20px;
            max-width: 100%;
        }}
        .global-header nav a {{
            color: #00d9ff;
            text-decoration: none;
            font-size: 15px;
            font-weight: 500;
            padding: 8px 15px;
            border-radius: 4px;
            transition: background-color 0.2s;
        }}
        .global-header nav a:hover {{
            background-color: #16213e;
        }}
        .global-header nav a.active {{
            background-color: #00d9ff;
            color: #1a1a2e;
        }}
        .container {{
            max-width: 100%;
            padding: 20px;
        }}
        #storage-health {{
            background-color: #16213e;
            padding: 12px 20px;
            border-radius: 8px;
            margin-bottom: 20px;
            font-size: 0.9rem;
            border: 1px solid #0f3460;
        }}
        #storage-health .health-item {{
            display: inline-block;
            margin-right: 25px;
        }}
        #storage-health .health-label {{
            color: #888;
            margin-right: 5px;
        }}
        #storage-health .status-good {{ color: #28a745; }}
        #storage-health .status-warning {{ color: #ffc107; }}
        #storage-health .status-critical {{ color: #dc3545; }}
        header {{
            background-color: #16213e;
            padding: 15px 20px;
            border-bottom: 2px solid #0f3460;
            margin-bottom: 20px;
        }}
        header h1 {{
            color: #00d9ff;
            font-size: 1.5rem;
            font-weight: 600;
        }}
        .search-form {{
            background-color: #16213e;
            padding: 20px;
            border-radius: 8px;
            margin-bottom: 20px;
        }}
        .search-row {{
            display: flex;
            gap: 15px;
            flex-wrap: wrap;
            margin-bottom: 15px;
        }}
        .search-row:last-child {{
            margin-bottom: 0;
        }}
        .form-group {{
            display: flex;
            flex-direction: column;
            gap: 5px;
        }}
        .form-group.search-input {{
            flex: 1;
            min-width: 300px;
        }}
        .form-group label {{
            color: #888;
            font-size: 0.85rem;
            font-weight: 500;
        }}
        input, select {{
            background-color: #0f3460;
            border: 1px solid #1a1a2e;
            color: #eee;
            padding: 10px 12px;
            border-radius: 4px;
            font-size: 0.95rem;
        }}
        input:focus, select:focus {{
            outline: none;
            border-color: #00d9ff;
        }}
        input[type="text"] {{
            width: 100%;
        }}
        select {{
            min-width: 150px;
        }}
        button {{
            background-color: #00d9ff;
            color: #1a1a2e;
            border: none;
            padding: 10px 25px;
            border-radius: 4px;
            font-size: 0.95rem;
            font-weight: 600;
            cursor: pointer;
            transition: background-color 0.2s;
        }}
        button:hover {{
            background-color: #00b8d4;
        }}
        .time-presets {{
            display: flex;
            gap: 8px;
            align-items: flex-end;
        }}
        .time-preset {{
            background-color: #0f3460;
            color: #00d9ff;
            border: 1px solid #00d9ff;
            padding: 8px 12px;
            border-radius: 4px;
            font-size: 0.85rem;
            cursor: pointer;
            transition: all 0.2s;
        }}
        .time-preset:hover, .time-preset.active {{
            background-color: #00d9ff;
            color: #1a1a2e;
        }}
        .results-table {{
            width: 100%;
            border-collapse: collapse;
            font-size: 0.9rem;
            table-layout: fixed;
        }}
        .results-table th {{
            background-color: #16213e;
            padding: 12px 10px;
            text-align: left;
            font-weight: 600;
            color: #00d9ff;
            border-bottom: 2px solid #0f3460;
            position: sticky;
            top: 0;
        }}
        .results-table th a {{
            color: #00d9ff;
            text-decoration: none;
            display: inline-block;
            cursor: pointer;
            user-select: none;
        }}
        .results-table th a:hover {{
            color: #fff;
            text-decoration: underline;
        }}
        .results-table td {{
            padding: 10px;
            border-bottom: 1px solid #0f3460;
            vertical-align: top;
            overflow: hidden;
            text-overflow: ellipsis;
        }}
        .results-table tr:hover {{
            background-color: #16213e;
        }}
        .results-table .timestamp {{
            width: 200px;
            font-family: monospace;
            white-space: nowrap;
            color: #888;
        }}
        .results-table .priority {{
            width: 60px;
            text-align: center;
        }}
        .results-table .message {{
            font-family: monospace;
            white-space: pre-wrap;
            word-break: break-all;
        }}
        .priority-critical {{
            background-color: rgba(255, 0, 0, 0.15);
        }}
        .priority-critical .priority {{
            color: #ff4444;
            font-weight: bold;
        }}
        .priority-error {{
            background-color: rgba(255, 100, 0, 0.1);
        }}
        .priority-error .priority {{
            color: #ff8800;
        }}
        .priority-warning {{
            background-color: rgba(255, 200, 0, 0.05);
        }}
        .priority-warning .priority {{
            color: #ffcc00;
        }}
        .pagination {{
            display: flex;
            justify-content: center;
            align-items: center;
            gap: 15px;
            padding: 20px;
            background-color: #16213e;
            border-radius: 8px;
            margin-top: 20px;
        }}
        .page-link {{
            color: #00d9ff;
            text-decoration: none;
            padding: 8px 15px;
            border: 1px solid #00d9ff;
            border-radius: 4px;
            transition: all 0.2s;
        }}
        .page-link:hover {{
            background-color: #00d9ff;
            color: #1a1a2e;
        }}
        .page-info {{
            color: #888;
        }}
        .no-results {{
            text-align: center;
            padding: 40px;
            color: #888;
        }}
        .keyboard-hint {{
            color: #666;
            font-size: 0.8rem;
            margin-left: 5px;
        }}
        @media (max-width: 768px) {{
            .search-row {{
                flex-direction: column;
            }}
            .form-group.search-input {{
                min-width: 100%;
            }}
            .time-presets {{
                flex-wrap: wrap;
            }}
        }}
        #timechart-container {{
            background-color: #16213e;
            border-radius: 8px;
            padding: 15px;
            margin-bottom: 20px;
            max-height: 250px;
        }}
        #timechart-container canvas {{
            max-height: 200px;
        }}
        #bin-toggle {{
            display: flex;
            justify-content: center;
            gap: 8px;
            margin-top: 8px;
        }}
        .bin-btn {{
            background-color: #0f3460;
            color: #00d9ff;
            border: 1px solid #00d9ff;
            padding: 4px 14px;
            border-radius: 4px;
            font-size: 0.8rem;
            cursor: pointer;
            transition: all 0.2s;
        }}
        .bin-btn:hover, .bin-btn.active {{
            background-color: #00d9ff;
            color: #1a1a2e;
        }}
        #results-table {{
            height: 600px;
            background-color: #16213e;
            border-radius: 8px;
            overflow: hidden;
        }}
        .column-chooser-wrapper {{
            position: relative;
            display: inline-block;
        }}
        .column-chooser-btn {{
            background-color: #0f3460;
            color: #00d9ff;
            border: 1px solid #00d9ff;
            padding: 8px 14px;
            border-radius: 4px;
            font-size: 0.85rem;
            cursor: pointer;
            transition: all 0.2s;
        }}
        .column-chooser-btn:hover {{
            background-color: #00d9ff;
            color: #1a1a2e;
        }}
        .column-chooser-panel {{
            display: none;
            position: absolute;
            top: 100%;
            left: 0;
            z-index: 100;
            background-color: #16213e;
            border: 1px solid #0f3460;
            border-radius: 8px;
            padding: 12px;
            min-width: 250px;
            max-height: 400px;
            overflow-y: auto;
            box-shadow: 0 4px 12px rgba(0,0,0,0.5);
        }}
        .column-chooser-panel.open {{
            display: block;
        }}
        .column-chooser-panel label {{
            display: flex;
            align-items: center;
            gap: 8px;
            padding: 4px 0;
            color: #ccc;
            font-size: 0.85rem;
            cursor: pointer;
        }}
        .column-chooser-panel label:hover {{
            color: #fff;
        }}
        .column-chooser-panel input[type="checkbox"] {{
            accent-color: #00d9ff;
            width: 16px;
            height: 16px;
        }}
        .column-chooser-panel .col-type {{
            color: #666;
            font-size: 0.75rem;
            margin-left: auto;
        }}
        .column-chooser-panel .col-actions {{
            display: flex;
            gap: 8px;
            margin-top: 8px;
            padding-top: 8px;
            border-top: 1px solid #0f3460;
        }}
        .column-chooser-panel .col-actions button {{
            flex: 1;
            padding: 6px 10px;
            font-size: 0.8rem;
        }}
        .tabulator {{
            background-color: #16213e;
            border: none;
            border-radius: 8px;
        }}
        .tabulator .tabulator-header {{
            background-color: #16213e;
            border-bottom: 2px solid #0f3460;
        }}
        .tabulator .tabulator-header .tabulator-col {{
            background-color: #16213e;
            border-right-color: #0f3460;
        }}
        .tabulator .tabulator-header .tabulator-col .tabulator-col-title {{
            color: #00d9ff;
        }}
        .tabulator .tabulator-tableholder .tabulator-table .tabulator-row {{
            border-bottom: 1px solid #0f3460;
        }}
        .tabulator .tabulator-tableholder .tabulator-table .tabulator-row .tabulator-cell {{
            border-right-color: #0f3460;
        }}
    </style>
</head>
<body>
    <div class="global-header">
        <nav>
            <a href="/" target="_blank" class="active">Log Search</a>
            <a href="/processes.html" target="_blank">Processes</a>
        </nav>
    </div>
    <div class="container">
        <div id="storage-health">
            <span class="health-item">
                <span class="health-label">Storage:</span>
                <span id="storage-info">Loading...</span>
            </span>
            <span class="health-item">
                <span class="health-label">Retention:</span>
                <span id="retention-info">Loading...</span>
            </span>
        </div>
        <header>
            <h1>Livedata Log Search</h1>
        </header>
        <form class="search-form" method="get" action="/">
            <div class="search-row">
                <div class="form-group search-input">
                    <label for="q">Search <span class="keyboard-hint">(Press / to focus)</span></label>
                    <input type="text" id="q" name="q" value="{}" placeholder="Search log messages...">
                </div>
                <div class="form-group">
                    <label>&nbsp;</label>
                    <button type="submit">Search</button>
                </div>
                <div class="form-group">
                    <label>&nbsp;</label>
                    <div class="column-chooser-wrapper">
                        <button type="button" class="column-chooser-btn" id="col-chooser-toggle">Columns</button>
                        <div class="column-chooser-panel" id="col-chooser-panel"></div>
                    </div>
                </div>
            </div>
            <div class="search-row">
                <div class="form-group">
                    <label for="start">Start Time</label>
                    <input type="text" id="start" name="start" value="{}" placeholder="-1h">
                </div>
                <div class="form-group">
                    <label for="end">End Time</label>
                    <input type="text" id="end" name="end" value="{}" placeholder="now">
                </div>
                <div class="time-presets">
                    <button type="button" class="time-preset" onclick="setTimeRange('-15m', 'now')">15m</button>
                    <button type="button" class="time-preset" onclick="setTimeRange('-1h', 'now')">1h</button>
                    <button type="button" class="time-preset" onclick="setTimeRange('-4h', 'now')">4h</button>
                    <button type="button" class="time-preset" onclick="setTimeRange('-24h', 'now')">24h</button>
                    <button type="button" class="time-preset" onclick="setTimeRange('-7d', 'now')">7d</button>
                </div>
            </div>
            <div class="search-row">
                <div class="form-group">
                    <label for="hostname">Hostname</label>
                    <select id="hostname" name="hostname">
                        <option value="">All Hosts</option>
                        {}
                    </select>
                </div>
                <div class="form-group">
                    <label for="unit">Systemd Unit</label>
                    <select id="unit" name="unit">
                        <option value="">All Units</option>
                        {}
                    </select>
                </div>
                <div class="form-group">
                    <label for="priority">Max Priority</label>
                    <select id="priority" name="priority">
                        <option value="">All Priorities</option>
                        {}
                    </select>
                </div>
            </div>
            <input type="hidden" name="limit" value="{}">
            <input type="hidden" name="offset" value="0">
            <input type="hidden" name="columns" id="columns-input" value="{}">
        </form>

        {}

        {}

        <div id="timechart-container">
            <canvas id="timechart"></canvas>
            <div id="bin-toggle">
                <button type="button" class="bin-btn" data-bin="minute">Minutes</button>
                <button type="button" class="bin-btn" data-bin="hour">Hours</button>
            </div>
        </div>

        <div id="results-table"></div>

        {}
    </div>

    <script type="application/json" id="results-data">
        {}
    </script>

    <script type="application/json" id="histogram-data"
            data-start="{}" data-end="{}" data-default-bin="{}">
        {}
    </script>

    <script type="application/json" id="columns-data">{}</script>
    <script type="application/json" id="active-columns-data">{}</script>

    <script>
        // Initialize timechart
        (function() {{
            const histEl = document.getElementById('histogram-data');
            const rangeStart = new Date(histEl.dataset.start);
            const rangeEnd = new Date(histEl.dataset.end);
            const defaultBin = histEl.dataset.defaultBin;
            let allHist = {{}};
            if (histEl && histEl.textContent.trim()) {{
                try {{ allHist = JSON.parse(histEl.textContent); }} catch(e) {{}}
            }}

            const months = ['Jan','Feb','Mar','Apr','May','Jun','Jul','Aug','Sep','Oct','Nov','Dec'];

            function toKey(dt) {{
                return dt.getUTCFullYear() + '-' +
                    String(dt.getUTCMonth()+1).padStart(2,'0') + '-' +
                    String(dt.getUTCDate()).padStart(2,'0') + ' ' +
                    String(dt.getUTCHours()).padStart(2,'0') + ':' +
                    String(dt.getUTCMinutes()).padStart(2,'0') + ':' +
                    String(dt.getUTCSeconds()).padStart(2,'0');
            }}

            function truncate(dt, unit) {{
                const d = new Date(dt);
                if (unit === 'minute') {{
                    d.setUTCSeconds(0, 0);
                }} else {{
                    d.setUTCMinutes(0, 0, 0);
                }}
                return d;
            }}

            function formatLabel(dt, unit) {{
                if (unit === 'minute') {{
                    return String(dt.getUTCHours()).padStart(2,'0') + ':' +
                           String(dt.getUTCMinutes()).padStart(2,'0');
                }} else {{
                    return months[dt.getUTCMonth()] + ' ' + dt.getUTCDate() + ' ' +
                           String(dt.getUTCHours()).padStart(2,'0') + ':00';
                }}
            }}

            function buildSeries(unit) {{
                const histData = allHist[unit] || [];
                const countMap = {{}};
                histData.forEach(d => {{ countMap[d.bin] = d.count; }});
                const binMs = unit === 'minute' ? 60000 : 3600000;
                const labels = [];
                const counts = [];
                let cur = truncate(rangeStart, unit);
                const endT = rangeEnd.getTime();
                while (cur.getTime() <= endT) {{
                    labels.push(formatLabel(cur, unit));
                    counts.push(countMap[toKey(cur)] || 0);
                    cur = new Date(cur.getTime() + binMs);
                }}
                return {{ labels, counts }};
            }}

            const container = document.getElementById('timechart-container');
            let chart = null;

            function renderChart(unit) {{
                const series = buildSeries(unit);
                if (series.labels.length === 0) {{
                    container.style.display = 'none';
                    return;
                }}
                container.style.display = '';

                // Update active button
                document.querySelectorAll('.bin-btn').forEach(b => {{
                    b.classList.toggle('active', b.dataset.bin === unit);
                }});

                if (chart) {{
                    chart.data.labels = series.labels;
                    chart.data.datasets[0].data = series.counts;
                    chart.update();
                }} else {{
                    chart = new Chart(document.getElementById('timechart'), {{
                        type: 'bar',
                        data: {{
                            labels: series.labels,
                            datasets: [{{
                                label: 'Events',
                                data: series.counts,
                                backgroundColor: 'rgba(0, 217, 255, 0.7)',
                                borderColor: 'rgba(0, 217, 255, 1)',
                                borderWidth: 1,
                            }}]
                        }},
                        options: {{
                            responsive: true,
                            maintainAspectRatio: false,
                            plugins: {{
                                legend: {{ display: false }},
                            }},
                            scales: {{
                                x: {{
                                    ticks: {{ color: '#888', maxRotation: 45, maxTicksLimit: 30 }},
                                    grid: {{ color: 'rgba(255,255,255,0.05)' }},
                                }},
                                y: {{
                                    beginAtZero: true,
                                    ticks: {{ color: '#888' }},
                                    grid: {{ color: 'rgba(255,255,255,0.05)' }},
                                }}
                            }}
                        }}
                    }});
                }}
            }}

            // Bind toggle buttons
            document.querySelectorAll('.bin-btn').forEach(btn => {{
                btn.addEventListener('click', () => renderChart(btn.dataset.bin));
            }});

            // Initial render
            renderChart(defaultBin);
        }})();
    </script>

    <script type="module">
        import {{TabulatorFull as Tabulator}} from "https://unpkg.com/tabulator-tables@6.3.1/dist/js/tabulator_esm.min.js";

        // Focus search on / key
        document.addEventListener('keydown', function(e) {{
            if (e.key === '/' && document.activeElement.tagName !== 'INPUT') {{
                e.preventDefault();
                document.getElementById('q').focus();
            }}
        }});

        // Time preset buttons
        function setTimeRange(start, end) {{
            document.getElementById('start').value = start;
            document.getElementById('end').value = end;
            document.querySelector('form').submit();
        }}
        window.setTimeRange = setTimeRange;

        // Initialize Tabulator
        const dataElement = document.getElementById('results-data');
        let data = [];
        if (dataElement && dataElement.textContent.trim()) {{
            try {{
                data = JSON.parse(dataElement.textContent);
            }} catch (parseError) {{
                console.error('Failed to parse JSON:', parseError);
            }}
        }}

        // Build dynamic column definitions
        const activeColumns = JSON.parse(document.getElementById('active-columns-data').textContent || '[]');
        const knownWidths = {{timestamp: 200, hostname: 120, unit: 150, priority: 70, comm: 120, pid: 80}};
        const knownAlign = {{priority: "center"}};

        function buildColumns(cols) {{
            return cols.map(field => {{
                const def = {{title: field.charAt(0).toUpperCase() + field.slice(1), field: field}};
                if (knownWidths[field]) def.width = knownWidths[field];
                if (knownAlign[field]) def.hozAlign = knownAlign[field];
                return def;
            }});
        }}

        if (data.length > 0) {{
            new Tabulator("#results-table", {{
                data: data,
                layout: "fitColumns",
                height: "600px",
                columns: buildColumns(activeColumns),
                rowFormatter: function(row) {{
                    const p = row.getData().priority;
                    if (p !== undefined && p !== null) {{
                        if (p <= 2) {{
                            row.getElement().style.backgroundColor = "rgba(255, 0, 0, 0.15)";
                        }} else if (p <= 3) {{
                            row.getElement().style.backgroundColor = "rgba(255, 100, 0, 0.1)";
                        }} else if (p <= 4) {{
                            row.getElement().style.backgroundColor = "rgba(255, 200, 0, 0.05)";
                        }}
                    }}
                }},
            }});
        }}

        // Column chooser logic
        (function() {{
            const allCols = JSON.parse(document.getElementById('columns-data').textContent || '[]');
            const panel = document.getElementById('col-chooser-panel');
            const toggle = document.getElementById('col-chooser-toggle');
            const columnsInput = document.getElementById('columns-input');
            const form = document.querySelector('form');

            // Load saved preferences from localStorage
            let savedCols = null;
            try {{
                const stored = localStorage.getItem('livedata-columns');
                if (stored) savedCols = JSON.parse(stored);
            }} catch(e) {{}}

            // Determine which columns are currently active
            const currentActive = new Set(
                columnsInput.value ? columnsInput.value.split(',') : allCols.filter(c => c.default).map(c => c.name)
            );

            // Build checkboxes
            allCols.forEach(col => {{
                const label = document.createElement('label');
                const cb = document.createElement('input');
                cb.type = 'checkbox';
                cb.value = col.name;
                cb.checked = currentActive.has(col.name);
                const nameSpan = document.createElement('span');
                nameSpan.textContent = col.name;
                const typeSpan = document.createElement('span');
                typeSpan.className = 'col-type';
                typeSpan.textContent = col.column_type;
                label.appendChild(cb);
                label.appendChild(nameSpan);
                label.appendChild(typeSpan);
                panel.appendChild(label);
            }});

            // Add Apply / Reset buttons
            const actions = document.createElement('div');
            actions.className = 'col-actions';
            const applyBtn = document.createElement('button');
            applyBtn.type = 'button';
            applyBtn.textContent = 'Apply';
            applyBtn.addEventListener('click', () => {{
                const checked = Array.from(panel.querySelectorAll('input[type=checkbox]:checked')).map(cb => cb.value);
                if (checked.length === 0) return;
                localStorage.setItem('livedata-columns', JSON.stringify(checked));
                columnsInput.value = checked.join(',');
                form.submit();
            }});
            const resetBtn = document.createElement('button');
            resetBtn.type = 'button';
            resetBtn.textContent = 'Reset';
            resetBtn.style.backgroundColor = '#0f3460';
            resetBtn.style.color = '#ccc';
            resetBtn.addEventListener('click', () => {{
                localStorage.removeItem('livedata-columns');
                columnsInput.value = '';
                form.submit();
            }});
            actions.appendChild(applyBtn);
            actions.appendChild(resetBtn);
            panel.appendChild(actions);

            // Toggle panel
            toggle.addEventListener('click', (e) => {{
                e.stopPropagation();
                panel.classList.toggle('open');
            }});
            document.addEventListener('click', (e) => {{
                if (!panel.contains(e.target) && e.target !== toggle) {{
                    panel.classList.remove('open');
                }}
            }});

            // On page load, if localStorage has saved columns, set the hidden input
            if (savedCols && !columnsInput.value) {{
                columnsInput.value = savedCols.join(',');
            }}
        }})();

        // Storage health indicator
        (async function() {{
            async function updateStorageHealth() {{
                try {{
                    const response = await fetch('/api/storage/health');
                    if (!response.ok) throw new Error('Failed to fetch storage health');

                    const data = await response.json();

                    // Calculate storage usage percentage
                    const sizeGB = (data.database_size_bytes / (1024 * 1024 * 1024)).toFixed(2);
                    const maxSizeGB = Math.max(data.retention_policy.log_max_size_gb, data.retention_policy.process_max_size_gb);
                    const usagePercent = (parseFloat(sizeGB) / maxSizeGB) * 100;

                    // Determine status color
                    let statusClass = 'status-good';
                    if (usagePercent >= 90) statusClass = 'status-critical';
                    else if (usagePercent >= 75) statusClass = 'status-warning';

                    // Update storage info
                    document.getElementById('storage-info').innerHTML =
                        `<span class="${{statusClass}}">${{sizeGB}}GB / ${{maxSizeGB}}GB</span> (${{data.journal_log_count.toLocaleString()}} logs, ${{data.process_metric_count.toLocaleString()}} metrics)`;

                    // Update retention info
                    document.getElementById('retention-info').textContent =
                        `${{data.retention_policy.log_retention_days}}d logs / ${{data.retention_policy.process_retention_days}}d proc`;

                }} catch (error) {{
                    console.error('Failed to fetch storage health:', error);
                    document.getElementById('storage-info').innerHTML = '<span class="status-critical">Error loading</span>';
                    document.getElementById('retention-info').textContent = 'N/A';
                }}
            }}

            // Update on load
            await updateStorageHealth();

            // Refresh every 30 seconds
            setInterval(updateStorageHealth, 30000);
        }})();
    </script>
</body>
</html>"##,
        html_escape(query_value),                // {0} search input value
        html_escape(&params.start),              // {1} start time
        html_escape(&params.end),                // {2} end time
        hostname_options,                        // {3} hostname options
        unit_options,                            // {4} unit options
        priority_options,                        // {5} priority options
        params.limit.min(100_000),               // {6} limit
        params.columns.as_deref().unwrap_or(""), // {7} columns hidden input
        pagination.clone(),                      // {8} top pagination
        if results.is_empty() {
            // {9} no-results message
            "<div class=\"no-results\">No results found. Try adjusting your search or time range.</div>".to_string()
        } else {
            "".to_string()
        },
        pagination,          // {10} bottom pagination
        results_json,        // {11} results JSON
        start_iso,           // {12} histogram start
        end_iso,             // {13} histogram end
        bin_unit,            // {14} default bin
        histogram_json,      // {15} histogram data
        columns_json,        // {16} all columns info
        active_columns_json, // {17} active column names
    )
}

/// HTML escape helper
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// URL encode helper
fn url_encode(s: &str) -> String {
    let mut result = String::with_capacity(s.len() * 3);
    for c in s.chars() {
        match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => result.push(c),
            ' ' => result.push('+'),
            _ => {
                for b in c.to_string().as_bytes() {
                    result.push_str(&format!("%{:02X}", b));
                }
            }
        }
    }
    result
}

/// Create router for testing
#[cfg(test)]
fn create_test_app(data_dir: &str) -> Router {
    let process_monitor = Arc::new(ProcessMonitor::new());
    let settings = Settings::default();
    let buffer = Arc::new(Mutex::new(
        DuckDBBuffer::new(data_dir).expect("Failed to create test buffer"),
    ));
    let state = Arc::new(AppState::new(data_dir, buffer, process_monitor, settings));
    Router::new()
        .route("/", get(search_ui))
        .route("/api/search", get(api_search))
        .route("/api/columns", get(api_columns))
        .route("/api/filters", get(api_filters))
        .route("/api/processes", get(api_processes))
        .route("/api/storage/health", get(api_storage_health))
        .route("/health", get(health))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode as AxumStatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[test]
    fn test_parse_relative_time() {
        let now = Utc::now();

        let result = parse_time("-1h", now).unwrap();
        assert!((now - result).num_minutes() >= 59);
        assert!((now - result).num_minutes() <= 61);

        let result = parse_time("-15m", now).unwrap();
        assert!((now - result).num_minutes() >= 14);
        assert!((now - result).num_minutes() <= 16);

        let result = parse_time("-7d", now).unwrap();
        assert!((now - result).num_days() >= 6);
        assert!((now - result).num_days() <= 8);
    }

    #[test]
    fn test_parse_now() {
        let now = Utc::now();
        let result = parse_time("now", now).unwrap();
        assert_eq!(result, now);
    }

    #[test]
    fn test_escape_like() {
        assert_eq!(escape_like("test"), "test");
        assert_eq!(escape_like("test%value"), "test\\%value");
        assert_eq!(escape_like("test_value"), "test\\_value");
        assert_eq!(escape_like("test\\value"), "test\\\\value");
    }

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("<script>"), "&lt;script&gt;");
        assert_eq!(html_escape("a & b"), "a &amp; b");
        assert_eq!(html_escape("\"quoted\""), "&quot;quoted&quot;");
    }

    #[test]
    fn test_url_encode() {
        assert_eq!(url_encode("hello"), "hello");
        assert_eq!(url_encode("hello world"), "hello+world");
        assert_eq!(url_encode("a=b&c=d"), "a%3Db%26c%3Dd");
    }

    #[test]
    fn test_priority_label() {
        assert_eq!(priority_label(0), "Emergency");
        assert_eq!(priority_label(3), "Error");
        assert_eq!(priority_label(6), "Info");
        assert_eq!(priority_label(7), "Debug");
    }

    // API Tests

    #[tokio::test]
    async fn test_health_endpoint() {
        let temp_dir = tempfile::tempdir().unwrap();
        let app = create_test_app(temp_dir.path().to_str().unwrap());

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), AxumStatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let health: HealthResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(health.status, "ok");
    }

    #[tokio::test]
    async fn test_api_search_empty_results() {
        let temp_dir = tempfile::tempdir().unwrap();
        let app = create_test_app(temp_dir.path().to_str().unwrap());

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/search?start=-1h&end=now")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), AxumStatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let search_response: SearchResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(search_response.total, 0);
        assert!(search_response.results.is_empty());
    }

    #[tokio::test]
    async fn test_api_search_with_query_param() {
        let temp_dir = tempfile::tempdir().unwrap();
        let app = create_test_app(temp_dir.path().to_str().unwrap());

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/search?q=error&start=-1h&end=now&limit=10")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), AxumStatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let search_response: SearchResponse = serde_json::from_slice(&body).unwrap();
        // Empty dir should have no results
        assert_eq!(search_response.total, 0);
    }

    #[tokio::test]
    async fn test_api_filters_endpoint() {
        let temp_dir = tempfile::tempdir().unwrap();
        let app = create_test_app(temp_dir.path().to_str().unwrap());

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/filters")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), AxumStatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let filters: FilterValues = serde_json::from_slice(&body).unwrap();
        // Empty dir should have empty filter lists
        assert!(filters.hostnames.is_empty());
        assert!(filters.units.is_empty());
    }

    #[tokio::test]
    async fn test_search_ui_returns_html() {
        let temp_dir = tempfile::tempdir().unwrap();
        let app = create_test_app(temp_dir.path().to_str().unwrap());

        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), AxumStatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let html = String::from_utf8(body.to_vec()).unwrap();
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("Livedata"));
    }

    #[tokio::test]
    async fn test_api_search_with_priority_filter() {
        let temp_dir = tempfile::tempdir().unwrap();
        let app = create_test_app(temp_dir.path().to_str().unwrap());

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/search?start=-1h&end=now&priority=3")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), AxumStatusCode::OK);
    }

    #[tokio::test]
    async fn test_api_search_response_structure() {
        let temp_dir = tempfile::tempdir().unwrap();
        let app = create_test_app(temp_dir.path().to_str().unwrap());

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/search?start=-1h&end=now")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        // Verify response structure
        assert!(json.get("results").is_some());
        assert!(json.get("total").is_some());
        assert!(json.get("query_time_ms").is_some());
    }
}
