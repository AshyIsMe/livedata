use crate::config::Settings;
use crate::duckdb_buffer::DuckDBBuffer;
use crate::journal_reader::JournalLogReader;
use crate::log_entry::LogEntry;
use crate::process_monitor::{ProcessMetricsBatch, ProcessMonitor};
use anyhow::Result;
use chrono::{TimeDelta, Utc};
use gethostname::gethostname;
use log::{error, info, warn};
use signal_hook::consts::SIGINT;
use signal_hook::iterator::Signals;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use tokio::sync::mpsc;

pub struct ApplicationController {
    journal_reader: JournalLogReader,
    buffer: DuckDBBuffer,
    hostname: String,
    shutdown_signal: Arc<AtomicBool>,
    process_monitor: Arc<ProcessMonitor>,
    process_monitor_handle: Option<thread::JoinHandle<()>>,
    metrics_receiver_handle: Option<thread::JoinHandle<()>>,
    cleanup_handle: Option<thread::JoinHandle<()>>,
}

impl ApplicationController {
    pub fn new<P: AsRef<std::path::Path>>(
        data_dir: P,
        process_interval: u64,
        settings: Settings,
    ) -> Result<Self> {
        info!("Initializing Application Controller");

        let shutdown_signal = Arc::new(AtomicBool::new(false));

        // Backup database before any migrations
        Self::backup_database(&data_dir)?;

        let journal_reader = JournalLogReader::new()?;
        let buffer = DuckDBBuffer::new(&data_dir)?;
        let hostname = gethostname().to_str().unwrap_or("unknown").to_string();

        // Create mpsc channel for process metrics
        let (metrics_tx, mut metrics_rx) = mpsc::channel::<ProcessMetricsBatch>(32);

        // Create process monitor with metrics sender
        let process_monitor = Arc::new(ProcessMonitor::with_metrics_sender(
            metrics_tx,
            shutdown_signal.clone(),
        ));
        let process_monitor_handle = process_monitor.start_collection(process_interval);
        info!(
            "Started process monitoring with {}s interval",
            process_interval
        );

        // Get database path for the receiver task
        let db_path = buffer.db_path().to_path_buf();

        // Spawn dedicated receiver task in a thread to persist process metrics
        let metrics_receiver_handle = thread::spawn(move || {
            // Create tokio runtime for this thread
            let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

            rt.block_on(async move {
                // Open a separate connection for process metrics
                let conn = match duckdb::Connection::open(&db_path) {
                    Ok(c) => c,
                    Err(e) => {
                        error!("Failed to open database connection for process metrics: {}", e);
                        return;
                    }
                };

                info!("Process metrics receiver task started");

                while let Some(batch) = metrics_rx.recv().await {
                    let process_count = batch.processes.len();
                    info!("Received process metrics batch with {} processes", process_count);

                    if batch.processes.is_empty() {
                        continue;
                    }

                    // Batch insert all process metrics in a transaction
                    let result = conn.execute("BEGIN TRANSACTION", [])
                        .and_then(|_| {
                            let mut stmt = conn.prepare(
                                "INSERT INTO process_metrics (timestamp, pid, name, cpu_usage, mem_usage, user, runtime)
                                 VALUES (?, ?, ?, ?, ?, ?, ?)"
                            )?;

                            for process in batch.processes {
                                // Extract numeric UID from "Uid(1234)" format
                                let user = process.user_id.as_ref().and_then(|uid_str| {
                                    uid_str.strip_prefix("Uid(")
                                        .and_then(|s| s.strip_suffix(')'))
                                        .map(|s| s.to_string())
                                });

                                stmt.execute(duckdb::params![
                                    batch.timestamp.to_rfc3339(),
                                    process.pid as i32,
                                    process.name,
                                    process.cpu_percent as f64,
                                    process.memory_bytes as f64,
                                    user,
                                    process.runtime_secs as i64,
                                ])?;
                            }

                            conn.execute("COMMIT", [])?;
                            conn.execute("CHECKPOINT", [])
                        });

                    if let Err(e) = result {
                        error!("Failed to persist process metrics: {}", e);
                    } else {
                        info!("Successfully persisted {} process metrics", process_count);
                    }
                }

                if let Err(e) = conn.execute("CHECKPOINT", []) {
                    error!("Failed to checkpoint process metrics connection: {}", e);
                }

                info!("Process metrics receiver task shutting down");
            });
        });

        info!("Application Controller initialized successfully");
        info!("Using on-disk DuckDB at: {}", buffer.db_path().display());

        // Spawn background cleanup task
        let cleanup_handle = Self::spawn_cleanup_task(
            buffer.db_path().to_path_buf(),
            settings.clone(),
            shutdown_signal.clone(),
        );

        Ok(Self {
            journal_reader,
            buffer,
            hostname,
            shutdown_signal,
            process_monitor,
            process_monitor_handle: Some(process_monitor_handle),
            metrics_receiver_handle: Some(metrics_receiver_handle),
            cleanup_handle: Some(cleanup_handle),
        })
    }

    /// Backup database file before migrations
    fn backup_database<P: AsRef<std::path::Path>>(data_dir: P) -> Result<()> {
        let db_path = data_dir.as_ref().join("livedata.duckdb");

        if !db_path.exists() {
            // No database to backup yet
            return Ok(());
        }

        let backup_path = data_dir.as_ref().join("livedata.duckdb.bak");
        info!("Backing up database to: {}", backup_path.display());

        std::fs::copy(&db_path, &backup_path)?;
        info!("Database backup complete");

        Ok(())
    }

    /// Spawn background cleanup task that runs periodically
    fn spawn_cleanup_task(
        db_path: PathBuf,
        settings: Settings,
        shutdown_signal: Arc<AtomicBool>,
    ) -> thread::JoinHandle<()> {
        let interval_secs = settings.cleanup_interval_minutes * 60;

        info!(
            "Starting periodic storage cleanup (interval: {}m, priority: high)",
            settings.cleanup_interval_minutes
        );

        thread::spawn(move || {
            // Create tokio runtime for this thread
            let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

            rt.block_on(async move {
                let interval = Duration::from_secs(interval_secs as u64);

                // Run first cleanup immediately.
                if !shutdown_signal.load(Ordering::Relaxed) {
                    if let Err(e) = Self::run_cleanup_cycle(&db_path, &settings) {
                        error!("Cleanup cycle failed: {}", e);
                    }
                }

                let mut next_run = tokio::time::Instant::now() + interval;

                loop {
                    if shutdown_signal.load(Ordering::Relaxed) {
                        info!("Cleanup task shutting down");
                        break;
                    }

                    let now = tokio::time::Instant::now();
                    if now >= next_run {
                        if let Err(e) = Self::run_cleanup_cycle(&db_path, &settings) {
                            error!("Cleanup cycle failed: {}", e);
                        }
                        next_run = tokio::time::Instant::now() + interval;
                    }

                    // Short sleep so shutdown can interrupt promptly.
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
            });
        })
    }

    /// Run a single cleanup cycle (uninterruptible)
    fn run_cleanup_cycle(db_path: &Path, settings: &Settings) -> Result<()> {
        let mut buffer = DuckDBBuffer::new(db_path.parent().unwrap())?;

        let stats = buffer.enforce_retention(
            settings.log_retention_days,
            settings.log_max_size_gb,
            settings.process_retention_days,
            settings.process_max_size_gb,
        )?;

        if stats.total_deleted() > 0 {
            info!(
                "Cleanup cycle complete: {} total records deleted",
                stats.total_deleted()
            );
        }

        Ok(())
    }

    pub fn get_shutdown_signal(&self) -> Arc<AtomicBool> {
        self.shutdown_signal.clone()
    }

    pub fn get_process_monitor(&self) -> Arc<ProcessMonitor> {
        self.process_monitor.clone()
    }

    pub fn setup_signal_handler(&self) -> Result<()> {
        let shutdown_signal = self.shutdown_signal.clone();
        thread::spawn(move || {
            let mut signals = Signals::new([SIGINT, signal_hook::consts::SIGTERM]).unwrap();
            for signal in &mut signals {
                match signal {
                    SIGINT | signal_hook::consts::SIGTERM => {
                        info!("Received shutdown signal: {}", signal);
                        shutdown_signal.store(true, Ordering::Relaxed);
                        break;
                    }
                    _ => {}
                }
            }
        });

        Ok(())
    }

    pub fn run(&mut self, follow: bool) -> Result<()> {
        info!("Starting journald log collection to DuckDB");

        self.setup_signal_handler()?;

        // Process historical data from the last hour on startup (unless in follow mode)
        if !follow {
            self.process_startup_historical_data()?;
        } else {
            // In follow mode, just seek to tail for real-time monitoring
            self.journal_reader.seek_to_tail()?;
            // Position cursor at the last entry so next_entry() can read new entries
            self.journal_reader.previous_skip(1)?;
            info!("Follow mode: starting real-time monitoring from now");
        }

        let mut last_status_time = Utc::now();
        let status_interval = TimeDelta::seconds(30); // Log status every 30 seconds

        info!("Starting main loop");

        loop {
            // Check for shutdown signal
            if self.shutdown_signal.load(Ordering::Relaxed) {
                info!("Shutdown signal received, initiating graceful shutdown");
                break;
            }

            // Wait for new journal entries with a short timeout
            if self
                .journal_reader
                .wait_for_entry(Some(Duration::from_millis(100)))?
            {
                // Process any new journal entries
                while let Ok(Some(entry)) = self.journal_reader.next_log_entry() {
                    if let Err(e) = self.process_log_entry(entry) {
                        error!("Failed to process log entry: {}", e);
                    }
                }
            }

            // Log status periodically
            let current_time = Utc::now();
            if current_time - last_status_time >= status_interval {
                self.log_status();
                last_status_time = current_time;
            }

            // Small sleep to prevent busy waiting
            thread::sleep(Duration::from_millis(100));
        }

        // Graceful shutdown
        self.graceful_shutdown()
    }

    fn process_startup_historical_data(&mut self) -> Result<()> {
        info!("Processing startup historical data - last hour of logs");

        // Calculate cutoff time (1 hour ago)
        let cutoff_time = Utc::now() - TimeDelta::hours(1);

        // Process historical entries from the last hour in a single transaction
        // to avoid per-row auto-commit overhead
        self.buffer.conn.execute("BEGIN TRANSACTION", [])?;
        let result = self
            .journal_reader
            .process_historical_entries(cutoff_time, |entry| self.buffer.add_entry(entry));
        match result {
            Ok(count) => {
                self.buffer.conn.execute("COMMIT", [])?;
                let processed_count = count;

                info!(
                    "Processed {} historical entries from the last hour (stored in DuckDB)",
                    processed_count
                );
            }
            Err(e) => {
                let _ = self.buffer.conn.execute("ROLLBACK", []);
                return Err(e);
            }
        }

        // Now seek to tail for real-time monitoring
        self.journal_reader.seek_to_tail()?;
        // Position cursor at the last entry so next_entry() can read new entries
        // Without this, the cursor is "past the end" and next_entry() won't work
        self.journal_reader.previous_skip(1)?;
        info!("Completed historical data processing, starting real-time monitoring");

        Ok(())
    }

    fn process_log_entry(&mut self, entry: LogEntry) -> Result<()> {
        // Add entry to on-disk DuckDB
        self.buffer.add_entry(&entry)?;
        Ok(())
    }

    fn graceful_shutdown(&mut self) -> Result<()> {
        info!("Starting graceful shutdown");

        self.shutdown_signal.store(true, Ordering::Relaxed);

        self.process_monitor.shutdown_metrics_channel();

        if let Some(handle) = self.process_monitor_handle.take() {
            if let Err(e) = handle.join() {
                warn!("Failed to join process monitor thread: {:?}", e);
            }
        }

        if let Some(handle) = self.metrics_receiver_handle.take() {
            if let Err(e) = handle.join() {
                warn!("Failed to join metrics receiver thread: {:?}", e);
            }
        }

        if let Some(handle) = self.cleanup_handle.take() {
            if let Err(e) = handle.join() {
                warn!("Failed to join cleanup thread: {:?}", e);
            }
        }

        if let Err(e) = self.buffer.conn.execute("CHECKPOINT", []) {
            warn!("Failed to checkpoint database during shutdown: {}", e);
        }

        // Log final statistics
        self.log_final_statistics();

        info!("Graceful shutdown completed");
        Ok(())
    }

    fn log_status(&mut self) {
        match self.buffer.get_buffer_stats() {
            Ok(stats) => {
                info!(
                    "Status: {} total entries in DuckDB, {} distinct minutes",
                    stats.total_entries, stats.buffered_minutes_count
                );

                if let (Some(oldest), Some(newest)) = (&stats.oldest_minute, &stats.newest_minute) {
                    info!("Data range: {} to {}", oldest, newest);
                }
            }
            Err(e) => {
                warn!("Failed to get database status: {}", e);
            }
        }

        // Log database file size
        if let Ok(metadata) = std::fs::metadata(self.buffer.db_path()) {
            let size_mb = metadata.len() / (1024 * 1024);
            info!("Database size: {} MB", size_mb);
        }
    }

    fn log_final_statistics(&mut self) {
        info!("=== Final Statistics ===");

        // Database statistics
        match self.buffer.get_buffer_stats() {
            Ok(stats) => {
                info!(
                    "Total entries in DuckDB: {}, distinct minutes: {}",
                    stats.total_entries, stats.buffered_minutes_count
                );
            }
            Err(e) => {
                warn!("Failed to get final database stats: {}", e);
            }
        }

        // Database file size
        if let Ok(metadata) = std::fs::metadata(self.buffer.db_path()) {
            let size_mb = metadata.len() / (1024 * 1024);
            info!("Final database size: {} MB", size_mb);
        }

        info!("Database path: {}", self.buffer.db_path().display());
        info!("=== End Statistics ===");
    }

    pub fn get_status(&mut self) -> Result<ApplicationStatus> {
        let buffer_stats = self.buffer.get_buffer_stats()?;
        let db_size = std::fs::metadata(self.buffer.db_path())
            .map(|m| m.len())
            .unwrap_or(0);

        Ok(ApplicationStatus {
            hostname: self.hostname.clone(),
            total_entries: buffer_stats.total_entries,
            distinct_minutes_count: buffer_stats.buffered_minutes_count,
            oldest_entry_minute: buffer_stats.oldest_minute,
            newest_entry_minute: buffer_stats.newest_minute,
            database_size_bytes: db_size,
        })
    }
}

#[derive(Debug)]
pub struct ApplicationStatus {
    pub hostname: String,
    pub total_entries: i64,
    pub distinct_minutes_count: usize,
    pub oldest_entry_minute: Option<chrono::DateTime<Utc>>,
    pub newest_entry_minute: Option<chrono::DateTime<Utc>>,
    pub database_size_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_application_controller_creation() {
        let temp_dir = TempDir::new().unwrap();
        let settings = Settings::default();
        let result = ApplicationController::new(temp_dir.path(), 5, settings);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_status_retrieval() {
        let temp_dir = TempDir::new().unwrap();
        let settings = Settings::default();
        let mut controller = ApplicationController::new(temp_dir.path(), 5, settings).unwrap();

        let status = controller.get_status().unwrap();
        assert_eq!(status.total_entries, 0);
        assert_eq!(status.distinct_minutes_count, 0);
        assert!(!status.hostname.is_empty());
    }

    #[test]
    fn test_graceful_shutdown_allows_reopen() {
        let temp_dir = TempDir::new().unwrap();
        let settings = Settings::default();
        let mut controller = ApplicationController::new(temp_dir.path(), 1, settings).unwrap();

        std::thread::sleep(Duration::from_millis(100));

        controller.graceful_shutdown().unwrap();

        let db_path = temp_dir.path().join("livedata.duckdb");
        let conn = duckdb::Connection::open(&db_path);
        assert!(conn.is_ok());

        let conn = conn.unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM _schema_version", [], |row| row.get(0))
            .unwrap();
        assert!(count >= 0);
    }
}
