# Journald to Parquet Log Collection Plan

## Overview
Implement a Rust application to continuously listen to journald logs and write them to parquet files organized by hostname, date, and minute-level aggregation.

## Requirements Summary
- **Source**: All journald logs available to current user
- **Destination**: `./data/hostname/YYYY/MM/DD/YYYYMMDD-HHmm.parquet`
- **Aggregation**: Minute-level data grouping
- **Fields**: All available journald fields
- **Mode**: Follow new logs only (tail mode)
- **Flush Frequency**: End of each minute

## Architecture

### 1. Dependencies
```toml
[dependencies]
systemd = "0.10"           # journald interface
duckdb = "1.2"              # in-memory database for buffering
parquet = "52"             # parquet file writing (maintained for compatibility)
arrow = "52"               # columnar data structures
chrono = { version = "0.4", features = ["serde"] }
gethostname = "0.4"        # system hostname
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"         # JSON serialization
tokio = { version = "1.0", features = ["full"] }
anyhow = "1.0"             # error handling
log = "0.4"                # logging
env_logger = "0.10"        # logger implementation
```

### 2. Core Components

#### A. JournalLogReader
- **Purpose**: Interface with systemd journal
- **Key Functions**:
  - Initialize journal connection with proper permissions
  - Seek to tail position for new log collection
  - Continuously monitor for new entries using `journal.wait()`
  - Extract all fields from each log entry
  - Convert journald timestamps to UTC datetime

#### B. LogEntry Structure
```rust
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub fields: HashMap<String, String>,
}
```
- **Design**: Dynamic field storage for journald's variable fields
- **Key Fields**: Always include timestamp, MESSAGE, PRIORITY, _SYSTEMD_UNIT, _HOSTNAME, _PID, _EXE
- **Serialization**: Support for Arrow/Parquet conversion

#### C. DuckDBBuffer
- **Purpose**: Use DuckDB in-memory database for intelligent buffering
- **Database Structure**: 
  - Dynamic table creation based on journald fields
  - Primary table: `CREATE TABLE journal_logs (timestamp TIMESTAMP, minute_key TIMESTAMP, fields JSON)`
- **Operations**:
  - INSERT each log entry as JSON in fields column
  - minute_key truncated to minute for easy aggregation
  - Query by minute: `SELECT * FROM journal_logs WHERE minute_key = ?`
- **Benefits**: SQL-based aggregation, automatic indexing, flexible querying

#### D. ParquetWriter (DuckDB-based)
- **Directory Management**: Create `./data/hostname/YYYY/MM/DD/` structure
- **File Naming**: `YYYYMMDD-HHmm.parquet` format
- **Data Export**: Use DuckDB's native parquet export
  ```sql
  COPY (SELECT * FROM journal_logs WHERE minute_key = '2026-01-17 14:30:00') 
  TO 'output.parquet' (FORMAT parquet, COMPRESSION snappy)
  ```
- **Schema Handling**: DuckDB automatically infers optimal parquet schema
- **Atomic Writes**: DuckDB handles atomic file operations

#### E. Application Controller
- **Lifecycle Management**: Startup, main loop, graceful shutdown
- **Signal Handling**: SIGINT/SIGTERM for clean exit
- **Error Recovery**: Retry logic for transient failures
- **Resource Management**: Memory limits, buffer cleanup

### 3. Data Flow

```
[systemd journal] -> [JournalLogReader] -> [DuckDBBuffer] -> [ParquetWriter] -> [File System]
```

#### Detailed Flow:
1. **Startup**:
   - Get system hostname using `gethostname()`
   - Initialize journal connection
   - Seek to tail position
   - Create base directory structure

2. **Main Loop**:
   - Wait for new journal entries
   - Parse entry fields and timestamp
   - Add to appropriate minute buffer
   - Check if previous minute completed
   - Trigger parquet write for completed minutes
   - Repeat

3. **Minute Completion Logic**:
   - When current time > buffered minute + 60 seconds
   - Execute SQL query: `SELECT * FROM journal_logs WHERE minute_key = ?`
   - Use DuckDB's `COPY` command to export to parquet
   - Delete exported rows: `DELETE FROM journal_logs WHERE minute_key = ?`
   - Repeat for each completed minute

4. **Shutdown**:
   - Catch interrupt signals
   - Flush all pending buffers
   - Close all file handles
   - Exit cleanly

### 4. File Organization

#### Directory Structure:
```
./data/
├── hostname1/
│   ├── 2026/
│   │   ├── 01/
│   │   │   ├── 17/
│   │   │   │   ├── 20260117-1430.parquet
│   │   │   │   ├── 20260117-1431.parquet
│   │   │   │   └── ...
│   │   │   └── ...
│   │   └── ...
│   └── ...
└── hostname2/
    └── ...
```

#### Parquet Schema:
- **Dynamic Columns**: DuckDB automatically infers schema from JSON fields
- **Fixed Columns**: timestamp, minute_key, fields (JSON)
- **Data Types**: TIMESTAMP for timestamps, JSONB for dynamic fields
- **Partitioning**: By hostname and date through directory structure

### 5. Error Handling Strategy

#### Error Categories:
1. **Journal Access Errors**:
   - Permission denied
   - Journal unavailable
   - Connection lost

2. **File System Errors**:
   - Disk space exhausted
   - Permission denied
   - Directory creation failures

3. **Data Processing Errors**:
   - Timestamp parsing failures
   - Field conversion errors
   - Parquet write failures

#### Recovery Mechanisms:
- **Retries**: Exponential backoff for transient failures
- **Fallback**: Continue processing when possible
- **Logging**: Comprehensive error logging
- **Validation**: Data integrity checks

### 6. Performance Considerations

#### Memory Management:
- **DuckDB Buffering**: Automatic memory management by DuckDB
- **Row Deletion**: Clean up exported rows to prevent memory growth
- **JSON Storage**: Efficient JSON storage for variable journald fields

#### I/O Optimization:
- **Native Parquet Export**: DuckDB's optimized parquet writer
- **Compression**: Snappy compression via DuckDB
- **Async Operations**: Use tokio for non-blocking I/O
- **SQL Optimization**: DuckDB query optimizer for data retrieval

#### CPU Efficiency:
- **SQL Aggregation**: Leverage DuckDB's query optimization
- **Columnar Storage**: In-memory columnar format for efficiency
- **Zero-Copy Export**: DuckDB writes directly to parquet format
- **Automatic Indexing**: DuckDB creates indexes for efficient queries

### 7. Configuration

#### Runtime Configuration (future):
```toml
[general]
data_directory = "./data"
buffer_size = 10000
flush_interval_seconds = 60

[parquet]
compression = "snappy"
batch_size = 1000

[journal]
follow_only = true
fields = "all"  # or specific field list
```

### 8. Testing Strategy

#### Unit Tests:
- Journal entry parsing
- Timestamp conversion
- File path generation
- Schema creation

#### Integration Tests:
- End-to-end log flow
- Parquet file validation
- Error scenarios
- Performance benchmarks

#### Manual Testing:
- Real journald data
- High log volume scenarios
- Long-running stability

### 9. Security Considerations

#### Permissions:
- Run with minimal required privileges
- Only access logs available to current user
- Secure file permissions on output data

#### Data Privacy:
- No modification of log data
- Preserve all original fields and values
- Consider sensitive data implications in output

### 10. Monitoring and Observability

#### Logging:
- Application startup/shutdown events
- Error rates and types
- Processing statistics (entries/minute)
- File writing status

#### Metrics (future):
- Journal entry rate
- Parquet file sizes
- Buffer utilization
- Error frequencies

## Implementation Steps

1. **Project Setup**: Add dependencies, update Cargo.toml
2. **Basic Structures**: Implement LogEntry and core data types
3. **Journal Reader**: Create journald interface
4. **DuckDB Buffer**: Implement in-memory database buffering
5. **Parquet Writer**: Create DuckDB-based file writing functionality
6. **Application Controller**: Wire everything together
7. **Error Handling**: Add comprehensive error management
8. **Testing**: Write tests and validate functionality
9. **Documentation**: Add inline documentation and examples
10. **Performance Tuning**: Optimize memory and I/O usage

## Success Criteria

- ✅ Continuously collects new journald entries
- ✅ Properly aggregates by minute using SQL
- ✅ Creates correct directory/file structure
- ✅ Writes valid parquet files using DuckDB
- ✅ Handles errors gracefully
- ✅ Provides clean shutdown
- ✅ Maintains data integrity through ACID operations
- ✅ Uses DuckDB for efficient in-memory buffering
- ✅ Leverages SQL for flexible data querying
- ✅ Comprehensive logging

## Future Enhancements

- Configurable field filtering
- Historical log processing mode
- Multiple output formats
- Real-time query interface using DuckDB buffer
- Log rotation and cleanup
- Distributed processing
- Web dashboard for monitoring
- SQL-based log analysis before parquet export
- Custom aggregation functions in DuckDB
- Direct DuckDB database export alongside parquet files