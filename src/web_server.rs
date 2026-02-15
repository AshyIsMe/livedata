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
use tower_http::trace::TraceLayer;
use tracing::info;

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

#[derive(Debug, Deserialize)]
pub struct ProcessTableParams {
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default = "default_process_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
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

fn default_process_limit() -> usize {
    100
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
    listen_all: bool,
) {
    let state = Arc::new(AppState::new(data_dir, buffer, process_monitor, settings));

    let app = Router::new()
        .route("/", get(search_ui))
        .route("/htmx/logs/chunk", get(htmx_logs_chunk))
        .route("/api/search", get(api_search))
        .route("/api/columns", get(api_columns))
        .route("/api/filters", get(api_filters))
        .route("/api/processes", get(api_processes))
        .route("/htmx/processes/chunk", get(htmx_processes_chunk))
        .route("/api/storage/health", get(api_storage_health))
        .route("/health", get(health))
        // Static file routes for process monitoring UI
        .route("/index.html", get(serve_index_html))
        .route("/processes.html", get(processes_ui))
        .layer(
            TraceLayer::new_for_http()
                .on_request(|request: &axum::http::Request<_>, _span: &tracing::Span| {
                    info!(
                        method = %request.method(),
                        path = %request.uri().path(),
                        query = request.uri().query().unwrap_or(""),
                        "http request started"
                    );
                })
                .on_response(
                    |response: &axum::http::Response<_>,
                     latency: std::time::Duration,
                     _span: &tracing::Span| {
                        info!(
                            status = response.status().as_u16(),
                            latency_ms = latency.as_millis(),
                            "http request completed"
                        );
                    },
                ),
        )
        .with_state(state);

    let bind_addr = if listen_all {
        "0.0.0.0:3000"
    } else {
        "127.0.0.1:3000"
    };

    let listener = tokio::net::TcpListener::bind(bind_addr).await.unwrap();
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

/// Serve process monitor page rendered with HTMX table fragments
async fn processes_ui() -> impl IntoResponse {
    Html(build_processes_html())
}

/// API endpoint returning current process snapshot
async fn api_processes(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ProcessResponse>, (StatusCode, String)> {
    let (processes, timestamp) = get_current_process_rows(&state)?;
    let total = processes.len();
    Ok(Json(ProcessResponse {
        processes,
        timestamp,
        total,
    }))
}

fn get_current_process_rows(
    state: &Arc<AppState>,
) -> Result<(Vec<ProcessMetricsRow>, String), (StatusCode, String)> {
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
        return Ok((processes, latest_timestamp));
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
    Ok((processes, timestamp))
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

async fn htmx_logs_chunk(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchParams>,
) -> impl IntoResponse {
    match query_log_results(&state, &params) {
        Ok((results, display_names, total_count)) => Html(render_log_chunk_fragment(
            &params,
            &results,
            &display_names,
            total_count,
        ))
        .into_response(),
        Err((status, msg)) => (status, msg).into_response(),
    }
}

async fn htmx_processes_chunk(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ProcessTableParams>,
) -> impl IntoResponse {
    let (mut processes, timestamp) = match get_current_process_rows(&state) {
        Ok(data) => data,
        Err((status, msg)) => return (status, msg).into_response(),
    };

    processes.sort_by(|a, b| b.cpu_usage.total_cmp(&a.cpu_usage));

    if let Some(q) = params.q.as_deref()
        && !q.trim().is_empty()
    {
        let needle = q.to_lowercase();
        processes.retain(|p| {
            let haystack = format!(
                "{} {} {} {:.1} {}",
                p.pid,
                p.name,
                p.user.as_deref().unwrap_or(""),
                p.cpu_usage,
                p.timestamp
            )
            .to_lowercase();
            fuzzy_match(&needle, &haystack)
        });
    }

    let total_count = processes.len();
    let limit = params.limit.clamp(10, 1000);
    let start = params.offset.min(total_count);
    let end = (start + limit).min(total_count);
    let page = &processes[start..end];

    Html(render_process_chunk_fragment(
        &params,
        page,
        total_count,
        &timestamp,
    ))
    .into_response()
}

fn query_log_results(
    state: &Arc<AppState>,
    params: &SearchParams,
) -> Result<(Vec<serde_json::Value>, Vec<String>, usize), (StatusCode, String)> {
    let now = Utc::now();
    let start = parse_time(&params.start, now).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let end = parse_time(&params.end, now).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let limit = params.limit.min(100_000);

    let schema = get_schema_columns(&state.buffer);
    if schema.is_empty() {
        return Ok((
            Vec::new(),
            DEFAULT_COLUMNS
                .iter()
                .map(|c| column_display_name(c))
                .collect(),
            0,
        ));
    }

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

    let mut where_sql = format!(
        "timestamp >= '{}' AND timestamp < '{}'",
        start.to_rfc3339(),
        end.to_rfc3339()
    );

    if let Some(ref q) = params.q
        && !q.is_empty()
    {
        let escaped = escape_like(q);
        where_sql.push_str(&format!(" AND message ILIKE '%{}%' ESCAPE '\\'", escaped));
    }
    if let Some(ref hostname) = params.hostname
        && !hostname.is_empty()
    {
        let hosts: Vec<&str> = hostname.split(',').collect();
        let host_list: Vec<String> = hosts
            .iter()
            .map(|h| format!("'{}'", h.replace('\'', "''")))
            .collect();
        where_sql.push_str(&format!(" AND _hostname IN ({})", host_list.join(",")));
    }
    if let Some(ref unit) = params.unit
        && !unit.is_empty()
    {
        let units: Vec<&str> = unit.split(',').collect();
        let unit_list: Vec<String> = units
            .iter()
            .map(|u| format!("'{}'", u.replace('\'', "''")))
            .collect();
        where_sql.push_str(&format!(" AND _systemd_unit IN ({})", unit_list.join(",")));
    }
    if let Some(priority) = params.priority {
        where_sql.push_str(&format!(" AND CAST(priority AS INTEGER) <= {}", priority));
    }

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

    let count_sql = format!("SELECT COUNT(*) FROM journal_logs WHERE {}", where_sql);
    let sql = format!(
        "SELECT {} FROM journal_logs WHERE {} ORDER BY {} {} LIMIT {} OFFSET {}",
        select_list.join(", "),
        where_sql,
        sort_column,
        sort_direction,
        limit,
        params.offset
    );

    let total_count = state.buffer.lock().unwrap().query_usize(&count_sql);
    let results = state
        .buffer
        .lock()
        .unwrap()
        .query_json_rows(&sql, &display_names)
        .unwrap_or_default();

    Ok((results, display_names, total_count))
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
    let (results, display_names, total_count) =
        query_log_results(&state, &params).unwrap_or_default();

    // Get filter options
    let hostnames = state.buffer.lock().unwrap().query_distinct_strings(
        "SELECT DISTINCT _hostname FROM journal_logs WHERE _hostname IS NOT NULL ORDER BY _hostname",
    );

    let units = state.buffer.lock().unwrap().query_distinct_strings(
        "SELECT DISTINCT _systemd_unit FROM journal_logs WHERE _systemd_unit IS NOT NULL ORDER BY _systemd_unit",
    );

    let html = build_search_html(
        &params,
        &results,
        &display_names,
        total_count,
        &hostnames,
        &units,
    );

    Html(html)
}

#[allow(clippy::too_many_arguments)]
fn build_search_html(
    params: &SearchParams,
    results: &[serde_json::Value],
    display_names: &[String],
    total_count: usize,
    hostnames: &[String],
    units: &[String],
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

    let chunk_fragment = render_log_chunk_fragment(params, results, display_names, total_count);

    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Livedata - Log Search</title>
    <script src="https://unpkg.com/htmx.org@1.9.12"></script>
    <style>
        :root {{
            --bg: #272822;
            --surface: #2d2e27;
            --surface-alt: #3e3d32;
            --text: #f8f8f2;
            --muted: #a59f85;
            --border: #49483e;
            --accent: #a6e22e;
            --accent-2: #66d9ef;
            --warn: #fd971f;
            --danger: #f92672;
        }}
        body.theme-light {{
            --bg: #f8f8f2;
            --surface: #efefe7;
            --surface-alt: #ffffff;
            --text: #272822;
            --muted: #6f6b57;
            --border: #b7b39e;
            --accent: #3b7d15;
            --accent-2: #0f8395;
            --warn: #c15d00;
            --danger: #c2175b;
        }}
        * {{
            box-sizing: border-box;
            margin: 0;
            padding: 0;
        }}
        body {{
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
            background-color: var(--bg);
            color: var(--text);
            line-height: 1.5;
        }}
        .global-header {{
            background-color: var(--surface);
            border-bottom: 2px solid var(--accent);
            margin-bottom: 0;
        }}
        .global-header nav {{
            display: flex;
            gap: 5px;
            padding: 12px 20px;
            max-width: 100%;
        }}
        .global-header nav a {{
            color: var(--accent-2);
            text-decoration: none;
            font-size: 15px;
            font-weight: 500;
            padding: 8px 15px;
            border-radius: 4px;
            transition: background-color 0.2s;
        }}
        .global-header nav a:hover {{
            background-color: var(--surface-alt);
        }}
        .global-header nav a.active {{
            background-color: var(--accent);
            color: var(--bg);
        }}
        .container {{
            max-width: 100%;
            padding: 20px;
        }}
        #storage-health {{
            background-color: var(--surface);
            padding: 12px 20px;
            border-radius: 8px;
            margin-bottom: 20px;
            font-size: 0.9rem;
            border: 1px solid var(--border);
        }}
        #storage-health .health-item {{
            display: inline-block;
            margin-right: 25px;
        }}
        #storage-health .health-label {{
            color: var(--muted);
            margin-right: 5px;
        }}
        #storage-health .status-good {{ color: var(--accent); }}
        #storage-health .status-warning {{ color: var(--warn); }}
        #storage-health .status-critical {{ color: var(--danger); }}
        header {{
            background-color: var(--surface);
            padding: 15px 20px;
            border-bottom: 2px solid var(--border);
            margin-bottom: 20px;
        }}
        header h1 {{
            color: var(--accent-2);
            font-size: 1.5rem;
            font-weight: 600;
        }}
        .search-form {{
            background-color: var(--surface);
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
            color: var(--muted);
            font-size: 0.85rem;
            font-weight: 500;
        }}
        input, select {{
            background-color: var(--surface-alt);
            border: 1px solid var(--border);
            color: var(--text);
            padding: 10px 12px;
            border-radius: 4px;
            font-size: 0.95rem;
        }}
        input:focus, select:focus {{
            outline: none;
            border-color: var(--accent-2);
        }}
        input[type="text"] {{
            width: 100%;
        }}
        select {{
            min-width: 150px;
        }}
        button {{
            background-color: var(--accent);
            color: var(--bg);
            border: none;
            padding: 10px 25px;
            border-radius: 4px;
            font-size: 0.95rem;
            font-weight: 600;
            cursor: pointer;
            transition: background-color 0.2s;
        }}
        button:hover {{
            background-color: var(--accent-2);
        }}
        .time-presets {{
            display: flex;
            gap: 8px;
            align-items: flex-end;
        }}
        .time-preset {{
            background-color: var(--surface-alt);
            color: var(--accent-2);
            border: 1px solid var(--accent-2);
            padding: 8px 12px;
            border-radius: 4px;
            font-size: 0.85rem;
            cursor: pointer;
            transition: all 0.2s;
        }}
        .time-preset:hover, .time-preset.active {{
            background-color: var(--accent-2);
            color: var(--bg);
        }}
        .results-table-wrap {{
            border: 1px solid var(--border);
            border-radius: 8px;
            overflow: hidden;
            background-color: var(--surface);
        }}
        .results-table {{
            width: 100%;
            border-collapse: collapse;
            font-size: 0.9rem;
            table-layout: auto;
        }}
        .results-table th {{
            background-color: var(--surface-alt);
            padding: 12px 10px;
            text-align: left;
            font-weight: 600;
            color: var(--accent-2);
            border-bottom: 2px solid var(--border);
        }}
        .results-table td {{
            padding: 10px;
            border-bottom: 1px solid var(--border);
            vertical-align: top;
            word-break: break-word;
        }}
        .results-table tr:hover {{
            background-color: var(--surface-alt);
        }}
        .priority-critical {{
            background-color: rgba(255, 0, 0, 0.15);
        }}
        .priority-error {{
            background-color: rgba(255, 100, 0, 0.1);
        }}
        .priority-warning {{
            background-color: rgba(255, 200, 0, 0.05);
        }}
        .summary {{
            padding: 12px 16px;
            background-color: var(--surface);
            border: 1px solid var(--accent-2);
            border-radius: 8px;
            color: var(--text);
            margin-bottom: 12px;
        }}
        .no-results {{
            text-align: center;
            padding: 40px;
            color: var(--muted);
        }}
        .load-row {{
            text-align: center;
            padding: 14px;
        }}
        .load-row button {{
            border: 1px solid var(--accent-2);
            background: transparent;
            color: var(--accent-2);
        }}
        .load-row button:hover {{
            background: var(--accent-2);
            color: var(--bg);
        }}
        .keyboard-hint {{
            color: var(--muted);
            font-size: 0.8rem;
            margin-left: 5px;
        }}
        .theme-toggle {{
            position: fixed;
            top: 10px;
            right: 12px;
            z-index: 1000;
            border: 1px solid var(--accent-2);
            background: var(--surface-alt);
            color: var(--accent-2);
            width: 36px;
            height: 36px;
            border-radius: 18px;
            padding: 0;
            display: inline-flex;
            align-items: center;
            justify-content: center;
            font-size: 18px;
            line-height: 1;
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
    </style>
</head>
<body class="theme-dark">
    <button id="theme-toggle" class="theme-toggle" type="button" aria-label="Toggle theme">&#9680;</button>
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
            <input type="hidden" name="columns" value="{}">
        </form>

        <div class="summary">
            Showing {} loaded rows out of {} total
        </div>

        <div class="results-table-wrap">
            <table class="results-table">
                <thead>
                    <tr>{}</tr>
                </thead>
                <tbody id="log-rows">
                    {}
                </tbody>
            </table>
        </div>
    </div>

    <script>
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

        // Theme toggle (dark default)
        (function() {{
            const key = 'livedata-theme';
            const body = document.body;
            const btn = document.getElementById('theme-toggle');
            const iconDark = '&#9680;';
            const iconLight = '&#9681;';

            function apply(theme) {{
                body.classList.remove('theme-dark', 'theme-light');
                body.classList.add(theme === 'light' ? 'theme-light' : 'theme-dark');
                btn.innerHTML = theme === 'light' ? iconLight : iconDark;
            }}

            const saved = localStorage.getItem(key);
            apply(saved === 'light' ? 'light' : 'dark');

            btn.addEventListener('click', function() {{
                const next = body.classList.contains('theme-dark') ? 'light' : 'dark';
                localStorage.setItem(key, next);
                apply(next);
            }});
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
        results.len(),                           // {8} loaded row count
        total_count,                             // {9} total count
        display_names
            .iter()
            .map(|name| format!("<th>{}</th>", html_escape(name)))
            .collect::<Vec<_>>()
            .join(""), // {10} table headers
        chunk_fragment,                          // {11} rows fragment
    )
}

fn render_log_chunk_fragment(
    params: &SearchParams,
    results: &[serde_json::Value],
    display_names: &[String],
    total_count: usize,
) -> String {
    let rows = render_log_rows(results, display_names);
    let loaded_end = params.offset + results.len();
    let col_span = display_names.len().max(1);
    let load_more_row = if loaded_end < total_count {
        let next_offset = loaded_end;
        let next_url = build_log_chunk_url(params, next_offset);
        format!(
            r##"<tr id="load-more-logs"><td class="load-row" colspan="{}"><button hx-get="{}" hx-target="#load-more-logs" hx-swap="outerHTML">Load more</button></td></tr>"##,
            col_span, next_url
        )
    } else if total_count == 0 {
        format!(
            r##"<tr id="load-more-logs"><td class="no-results" colspan="{}">No results found</td></tr>"##,
            col_span
        )
    } else {
        format!(
            r##"<tr id="load-more-logs"><td class="load-row" colspan="{}">End of results</td></tr>"##,
            col_span
        )
    };

    format!("{}{}", rows, load_more_row)
}

fn render_log_rows(results: &[serde_json::Value], display_names: &[String]) -> String {
    let mut out = String::new();

    for row in results {
        let Some(obj) = row.as_object() else {
            continue;
        };

        let priority_value = obj.get("priority").and_then(|v| {
            v.as_i64()
                .or_else(|| v.as_str().and_then(|s| s.parse::<i64>().ok()))
        });
        let row_class = match priority_value {
            Some(p) if p <= 2 => "priority-critical",
            Some(3) => "priority-error",
            Some(4) => "priority-warning",
            _ => "",
        };

        out.push_str(&format!("<tr class=\"{}\">", row_class));
        for col in display_names {
            let value = obj.get(col).unwrap_or(&serde_json::Value::Null);
            let text = match value {
                serde_json::Value::Null => String::new(),
                serde_json::Value::String(s) => s.clone(),
                _ => value.to_string(),
            };
            out.push_str(&format!("<td>{}</td>", html_escape(&text)));
        }
        out.push_str("</tr>");
    }

    out
}

fn build_log_chunk_url(params: &SearchParams, offset: usize) -> String {
    format!(
        "/htmx/logs/chunk?q={}&start={}&end={}&hostname={}&unit={}&limit={}&offset={}&sort={}&sort_dir={}{}{}",
        url_encode(params.q.as_deref().unwrap_or("")),
        url_encode(&params.start),
        url_encode(&params.end),
        url_encode(params.hostname.as_deref().unwrap_or("")),
        url_encode(params.unit.as_deref().unwrap_or("")),
        params.limit.min(100_000),
        offset,
        url_encode(&params.sort),
        url_encode(&params.sort_dir),
        params
            .priority
            .map(|p| format!("&priority={}", p))
            .unwrap_or_default(),
        params
            .columns
            .as_deref()
            .map(|c| format!("&columns={}", url_encode(c)))
            .unwrap_or_default()
    )
}

fn build_processes_html() -> String {
    r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>livedata - Process Monitor</title>
    <script src="https://unpkg.com/htmx.org@1.9.12"></script>
    <style>
        :root {
            --bg: #272822;
            --surface: #2d2e27;
            --surface-alt: #3e3d32;
            --text: #f8f8f2;
            --muted: #a59f85;
            --border: #49483e;
            --accent: #a6e22e;
            --accent-2: #66d9ef;
            --warn: #fd971f;
            --danger: #f92672;
        }
        body.theme-light {
            --bg: #f8f8f2;
            --surface: #efefe7;
            --surface-alt: #ffffff;
            --text: #272822;
            --muted: #6f6b57;
            --border: #b7b39e;
            --accent: #3b7d15;
            --accent-2: #0f8395;
            --warn: #c15d00;
            --danger: #c2175b;
        }
        body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; max-width: 1400px; margin: 0 auto; padding: 0; background-color: var(--bg); color: var(--text); }
        .global-header { background: var(--surface); border-bottom: 2px solid var(--accent); box-shadow: 0 2px 4px rgba(0,0,0,0.2); margin-bottom: 20px; }
        .global-header nav { display: flex; gap: 5px; padding: 15px 20px; max-width: 1400px; margin: 0 auto; }
        .global-header nav a { color: var(--accent-2); text-decoration: none; font-size: 16px; font-weight: 500; padding: 10px 18px; border-radius: 4px; }
        .global-header nav a:hover { background: var(--surface-alt); }
        .global-header nav a.active { background-color: var(--accent); color: var(--bg); }
        .container { padding: 0 20px 20px; }
        .controls { display: flex; gap: 12px; align-items: end; margin-bottom: 14px; flex-wrap: wrap; }
        input, button { padding: 8px 12px; border: 1px solid var(--border); border-radius: 4px; font-size: 14px; }
        input { background: var(--surface-alt); color: var(--text); }
        button { background-color: var(--accent); color: var(--bg); border: none; cursor: pointer; }
        button:hover { background: var(--accent-2); }
        table { width: 100%; border-collapse: collapse; background: var(--surface); box-shadow: 0 1px 3px rgba(0,0,0,0.25); }
        th, td { padding: 10px; border-bottom: 1px solid var(--border); text-align: left; }
        th { background: var(--surface-alt); color: var(--accent-2); }
        .muted { color: var(--muted); }
        .load-row { text-align: center; padding: 12px; }
        .load-row button { background: var(--surface-alt); color: var(--accent-2); border: 1px solid var(--accent-2); }
        .load-row button:hover { background: var(--accent-2); color: var(--bg); }
        #storage-health { margin-bottom: 20px; padding: 15px 20px; background: var(--surface); border-radius: 4px; box-shadow: 0 1px 3px rgba(0,0,0,0.2); font-size: 14px; color: var(--text); border: 1px solid var(--border); }
        #storage-health .health-item { display: inline-block; margin-right: 25px; }
        #storage-health .health-label { font-weight: 600; margin-right: 5px; color: var(--muted); }
        #storage-health .status-good { color: var(--accent); }
        #storage-health .status-warning { color: var(--warn); }
        #storage-health .status-critical { color: var(--danger); }
        .theme-toggle { position: fixed; top: 10px; right: 12px; z-index: 1000; border: 1px solid var(--accent-2); background: var(--surface-alt); color: var(--accent-2); width: 36px; height: 36px; border-radius: 18px; padding: 0; display: inline-flex; align-items: center; justify-content: center; font-size: 18px; line-height: 1; }
    </style>
</head>
<body class="theme-dark">
    <button id="theme-toggle" class="theme-toggle" type="button" aria-label="Toggle theme">&#9680;</button>
    <div class="global-header">
        <nav>
            <a href="/" target="_blank">Log Search</a>
            <a href="/processes.html" target="_blank" class="active">Processes</a>
        </nav>
    </div>
    <div class="container">
        <div id="storage-health">
            <span class="health-item"><span class="health-label">Storage:</span><span id="storage-info">Loading...</span></span>
            <span class="health-item"><span class="health-label">Retention:</span><span id="retention-info">Loading...</span></span>
        </div>

        <h1>Process Monitor</h1>

        <form class="controls" hx-get="/htmx/processes/chunk" hx-target="#processes-body" hx-swap="innerHTML">
            <div>
                <label for="q">Search</label><br>
                <input id="q" name="q" type="text" placeholder="Fuzzy search processes..." />
            </div>
            <div>
                <label for="limit">Rows</label><br>
                <input id="limit" name="limit" type="number" min="10" max="1000" value="100" />
            </div>
            <div>
                <input name="offset" type="hidden" value="0" />
                <button type="submit">Refresh</button>
            </div>
            <div class="muted" id="last-updated">Never</div>
        </form>

        <table>
            <thead>
                <tr>
                    <th>Timestamp</th>
                    <th>PID</th>
                    <th>Name</th>
                    <th>CPU %</th>
                    <th>Memory</th>
                    <th>User</th>
                    <th>Runtime</th>
                </tr>
            </thead>
            <tbody id="processes-body" hx-get="/htmx/processes/chunk?offset=0&limit=100" hx-trigger="load" hx-swap="innerHTML"></tbody>
        </table>
    </div>

    <script>
        (function() {
            const key = 'livedata-theme';
            const body = document.body;
            const btn = document.getElementById('theme-toggle');
            const iconDark = '&#9680;';
            const iconLight = '&#9681;';

            function apply(theme) {
                body.classList.remove('theme-dark', 'theme-light');
                body.classList.add(theme === 'light' ? 'theme-light' : 'theme-dark');
                btn.innerHTML = theme === 'light' ? iconLight : iconDark;
            }

            const saved = localStorage.getItem(key);
            apply(saved === 'light' ? 'light' : 'dark');

            btn.addEventListener('click', function() {
                const next = body.classList.contains('theme-dark') ? 'light' : 'dark';
                localStorage.setItem(key, next);
                apply(next);
            });
        })();

        (async function() {
            async function updateStorageHealth() {
                try {
                    const response = await fetch('/api/storage/health');
                    if (!response.ok) throw new Error('Failed to fetch storage health');
                    const data = await response.json();
                    const sizeGB = (data.database_size_bytes / (1024 * 1024 * 1024)).toFixed(2);
                    const maxSizeGB = Math.max(data.retention_policy.log_max_size_gb, data.retention_policy.process_max_size_gb);
                    const usagePercent = (parseFloat(sizeGB) / maxSizeGB) * 100;
                    let statusClass = 'status-good';
                    if (usagePercent >= 90) statusClass = 'status-critical';
                    else if (usagePercent >= 75) statusClass = 'status-warning';
                    document.getElementById('storage-info').innerHTML =
                        `<span class="${statusClass}">${sizeGB}GB / ${maxSizeGB}GB</span> (${data.journal_log_count.toLocaleString()} logs, ${data.process_metric_count.toLocaleString()} metrics)`;
                    document.getElementById('retention-info').textContent =
                        `${data.retention_policy.log_retention_days}d logs / ${data.retention_policy.process_retention_days}d proc`;
                } catch (error) {
                    document.getElementById('storage-info').innerHTML = '<span class="status-critical">Error loading</span>';
                    document.getElementById('retention-info').textContent = 'N/A';
                }
            }
            await updateStorageHealth();
            setInterval(updateStorageHealth, 30000);
        })();
    </script>
</body>
</html>"##
        .to_string()
}

fn render_process_chunk_fragment(
    params: &ProcessTableParams,
    rows: &[ProcessMetricsRow],
    total_count: usize,
    timestamp: &str,
) -> String {
    let mut html = String::new();
    for p in rows {
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{:.1}%</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            html_escape(timestamp),
            p.pid,
            html_escape(&p.name),
            p.cpu_usage,
            html_escape(&format_bytes(p.mem_usage)),
            html_escape(p.user.as_deref().unwrap_or("-")),
            html_escape(&format_runtime(p.runtime))
        ));
    }

    let loaded_end = params.offset + rows.len();
    if loaded_end < total_count {
        let next_offset = loaded_end;
        let next_url = format!(
            "/htmx/processes/chunk?q={}&limit={}&offset={}",
            url_encode(params.q.as_deref().unwrap_or("")),
            params.limit.clamp(10, 1000),
            next_offset
        );
        html.push_str(&format!(
            r##"<tr id="load-more-processes"><td class="load-row" colspan="7"><button hx-get="{}" hx-target="#load-more-processes" hx-swap="outerHTML">Load more</button></td></tr>"##,
            next_url
        ));
    } else if total_count == 0 {
        html.push_str(
            r##"<tr id="load-more-processes"><td class="load-row muted" colspan="7">No processes found</td></tr>"##,
        );
    } else {
        html.push_str(
            r##"<tr id="load-more-processes"><td class="load-row muted" colspan="7">End of results</td></tr>"##,
        );
    }

    html
}

fn fuzzy_match(query: &str, text: &str) -> bool {
    let mut text_chars = text.chars();
    for qc in query.chars() {
        if !text_chars.any(|tc| tc == qc) {
            return false;
        }
    }
    true
}

fn format_bytes(bytes: f64) -> String {
    if bytes <= 0.0 {
        return "-".to_string();
    }
    let units = ["B", "KB", "MB", "GB", "TB", "PB"];
    let mut value = bytes;
    let mut idx = 0usize;
    while value >= 1024.0 && idx < units.len() - 1 {
        value /= 1024.0;
        idx += 1;
    }
    format!("{:.1} {}", value, units[idx])
}

fn format_runtime(seconds: u64) -> String {
    if seconds < 60 {
        return format!("{}s", seconds);
    }
    let minutes = seconds / 60;
    if minutes < 60 {
        let rem = seconds % 60;
        return if rem > 0 {
            format!("{}m {}s", minutes, rem)
        } else {
            format!("{}m", minutes)
        };
    }
    let hours = minutes / 60;
    if hours < 24 {
        let rem = minutes % 60;
        return if rem > 0 {
            format!("{}h {}m", hours, rem)
        } else {
            format!("{}h", hours)
        };
    }
    let days = hours / 24;
    let rem = hours % 24;
    if rem > 0 {
        format!("{}d {}h", days, rem)
    } else {
        format!("{}d", days)
    }
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
        .route("/htmx/logs/chunk", get(htmx_logs_chunk))
        .route("/api/search", get(api_search))
        .route("/api/columns", get(api_columns))
        .route("/api/filters", get(api_filters))
        .route("/api/processes", get(api_processes))
        .route("/htmx/processes/chunk", get(htmx_processes_chunk))
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
