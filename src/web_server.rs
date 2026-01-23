use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::get,
};
use chrono::{DateTime, Duration, Utc};
use duckdb::Connection;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Application state shared across handlers
pub struct AppState {
    pub data_dir: String,
    pub conn: Mutex<Connection>,
}

impl AppState {
    pub fn new(data_dir: &str) -> Result<Self, duckdb::Error> {
        // Connect to the on-disk DuckDB database
        let db_path = std::path::Path::new(data_dir).join("livedata.duckdb");
        let conn = Connection::open(&db_path)?;
        Ok(Self {
            data_dir: data_dir.to_string(),
            conn: Mutex::new(conn),
        })
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
    /// Results per page (default: 100, max: 10000)
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Pagination offset
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
    100
}

/// Search result entry
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

/// Search response
#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
    pub query_time_ms: u128,
}

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

pub async fn run_web_server(data_dir: &str, shutdown_signal: Arc<AtomicBool>) {
    let state = Arc::new(AppState::new(data_dir).expect("Failed to create application state"));

    let app = Router::new()
        .route("/", get(search_ui))
        .route("/api/search", get(api_search))
        .route("/api/filters", get(api_filters))
        .route("/health", get(health))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    println!("Web server listening on {}", listener.local_addr().unwrap());

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
    let limit = params.limit.min(10000);

    // Build SQL query against the journal_logs table
    let mut sql = format!(
        "SELECT CAST(timestamp AS VARCHAR), _hostname, _systemd_unit, priority, CAST(_pid AS VARCHAR), _comm, message
         FROM journal_logs
         WHERE timestamp >= '{}' AND timestamp < '{}'",
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

    // Add ordering and pagination
    sql.push_str(&format!(
        " ORDER BY timestamp DESC LIMIT {} OFFSET {}",
        limit, params.offset
    ));

    // Execute query
    let conn = state.conn.lock().unwrap();

    // Handle case where no parquet files exist (returns empty results)
    let results: Vec<SearchResult> = match conn.prepare(&sql) {
        Ok(mut stmt) => stmt
            .query_map([], |row| {
                Ok(SearchResult {
                    timestamp: row.get::<_, String>(0)?,
                    hostname: row.get::<_, Option<String>>(1)?,
                    unit: row.get::<_, Option<String>>(2)?,
                    priority: row.get::<_, Option<i32>>(3)?,
                    pid: row.get::<_, Option<String>>(4)?,
                    comm: row.get::<_, Option<String>>(5)?,
                    message: row.get::<_, Option<String>>(6)?,
                })
            })
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Query error: {}", e),
                )
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Query error: {}", e),
                )
            })?,
        Err(e) => {
            // If table doesn't exist yet (no data collected), return empty results
            let err_str = e.to_string();
            if err_str.contains("does not exist") || err_str.contains("journal_logs") {
                Vec::new()
            } else {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Query error: {}", e),
                ));
            }
        }
    };

    let total = results.len();
    let query_time_ms = start_time.elapsed().as_millis();

    Ok(Json(SearchResponse {
        results,
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
    let conn = state.conn.lock().unwrap();

    // Get distinct hostnames
    let hostnames: Vec<String> = conn
        .prepare("SELECT DISTINCT _hostname FROM journal_logs WHERE _hostname IS NOT NULL ORDER BY _hostname")
        .and_then(|mut stmt| {
            stmt.query_map([], |row| row.get(0))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();

    // Get distinct units
    let units: Vec<String> = conn
        .prepare("SELECT DISTINCT _systemd_unit FROM journal_logs WHERE _systemd_unit IS NOT NULL ORDER BY _systemd_unit")
        .and_then(|mut stmt| {
            stmt.query_map([], |row| row.get(0))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();

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

    // Execute search query
    let limit = params.limit.min(10000);

    let mut sql = format!(
        "SELECT CAST(timestamp AS VARCHAR), _hostname, _systemd_unit, priority, CAST(_pid AS VARCHAR), _comm, message
         FROM journal_logs
         WHERE timestamp >= '{}' AND timestamp < '{}'",
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

    // Count total results (for pagination info)
    let count_sql = sql.replace(
        "SELECT CAST(timestamp AS VARCHAR), _hostname, _systemd_unit, priority, CAST(_pid AS VARCHAR), _comm, message",
        "SELECT COUNT(*)",
    );

    // Add ordering and pagination
    sql.push_str(&format!(
        " ORDER BY timestamp DESC LIMIT {} OFFSET {}",
        limit, params.offset
    ));

    let conn = state.conn.lock().unwrap();

    // Get total count
    let total_count: usize = conn
        .prepare(&count_sql)
        .and_then(|mut stmt| stmt.query_row([], |row| row.get(0)))
        .unwrap_or(0);

    // Execute main query
    let results: Vec<SearchResult> = conn
        .prepare(&sql)
        .and_then(|mut stmt| {
            stmt.query_map([], |row| {
                Ok(SearchResult {
                    timestamp: row.get::<_, String>(0)?,
                    hostname: row.get::<_, Option<String>>(1)?,
                    unit: row.get::<_, Option<String>>(2)?,
                    priority: row.get::<_, Option<i32>>(3)?,
                    pid: row.get::<_, Option<String>>(4)?,
                    comm: row.get::<_, Option<String>>(5)?,
                    message: row.get::<_, Option<String>>(6)?,
                })
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();

    // Get filter options
    let hostnames: Vec<String> = conn
        .prepare("SELECT DISTINCT _hostname FROM journal_logs WHERE _hostname IS NOT NULL ORDER BY _hostname")
        .and_then(|mut stmt| {
            stmt.query_map([], |row| row.get(0))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();

    let units: Vec<String> = conn
        .prepare("SELECT DISTINCT _systemd_unit FROM journal_logs WHERE _systemd_unit IS NOT NULL ORDER BY _systemd_unit")
        .and_then(|mut stmt| {
            stmt.query_map([], |row| row.get(0))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();

    // Build HTML
    let html = build_search_html(&params, &results, total_count, &hostnames, &units);

    Html(html)
}

fn build_search_html(
    params: &SearchParams,
    results: &[SearchResult],
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

    // Build results table rows
    let result_rows: String = results
        .iter()
        .map(|r| {
            let priority_class = match r.priority {
                Some(0..=2) => "priority-critical",
                Some(3) => "priority-error",
                Some(4) => "priority-warning",
                _ => "",
            };
            format!(
                r#"<tr class="{}">
                    <td class="timestamp">{}</td>
                    <td>{}</td>
                    <td>{}</td>
                    <td class="priority">{}</td>
                    <td>{}</td>
                    <td class="message">{}</td>
                </tr>"#,
                priority_class,
                html_escape(&r.timestamp),
                html_escape(r.hostname.as_deref().unwrap_or("-")),
                html_escape(r.unit.as_deref().unwrap_or("-")),
                r.priority
                    .map(|p| format!("{}", p))
                    .unwrap_or("-".to_string()),
                html_escape(r.comm.as_deref().unwrap_or("-")),
                html_escape(r.message.as_deref().unwrap_or("-")),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Calculate pagination
    let limit = params.limit.min(10000);
    let current_page = params.offset / limit + 1;
    let total_pages = total_count.div_ceil(limit);

    // Build pagination links
    let pagination = if total_pages > 1 {
        let mut links = Vec::new();

        // Previous link
        if current_page > 1 {
            let prev_offset = (current_page - 2) * limit;
            links.push(format!(
                r#"<a href="?q={}&start={}&end={}&hostname={}&unit={}{}&limit={}&offset={}" class="page-link">&laquo; Prev</a>"#,
                url_encode(query_value),
                url_encode(&params.start),
                url_encode(&params.end),
                url_encode(hostname_value),
                url_encode(unit_value),
                priority_value.map(|p| format!("&priority={}", p)).unwrap_or_default(),
                limit,
                prev_offset
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
                r#"<a href="?q={}&start={}&end={}&hostname={}&unit={}{}&limit={}&offset={}" class="page-link">Next &raquo;</a>"#,
                url_encode(query_value),
                url_encode(&params.start),
                url_encode(&params.end),
                url_encode(hostname_value),
                url_encode(unit_value),
                priority_value.map(|p| format!("&priority={}", p)).unwrap_or_default(),
                limit,
                next_offset
            ));
        }

        format!("<div class=\"pagination\">{}</div>", links.join(" "))
    } else {
        format!(
            "<div class=\"pagination\"><span class=\"page-info\">{} results</span></div>",
            total_count
        )
    };

    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Livedata - Log Search</title>
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
        .container {{
            max-width: 100%;
            padding: 20px;
        }}
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
    </style>
</head>
<body>
    <header>
        <h1>Livedata Log Search</h1>
    </header>
    <div class="container">
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
        </form>

        {}

        {}

        <table class="results-table">
            <thead>
                <tr>
                    <th>Timestamp</th>
                    <th>Host</th>
                    <th>Unit</th>
                    <th>Pri</th>
                    <th>Comm</th>
                    <th>Message</th>
                </tr>
            </thead>
            <tbody>
                {}
            </tbody>
        </table>

        {}
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
    </script>
</body>
</html>"##,
        html_escape(query_value),
        html_escape(&params.start),
        html_escape(&params.end),
        hostname_options,
        unit_options,
        priority_options,
        params.limit.min(10000),
        pagination.clone(),
        if results.is_empty() {
            "<div class=\"no-results\">No results found. Try adjusting your search or time range.</div>".to_string()
        } else {
            "".to_string()
        },
        result_rows,
        pagination,
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
    let state = Arc::new(AppState::new(data_dir).expect("Failed to create test app state"));
    Router::new()
        .route("/", get(search_ui))
        .route("/api/search", get(api_search))
        .route("/api/filters", get(api_filters))
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
