use crate::duckdb_buffer::DuckDBBuffer;
use crate::journal_reader::JournalLogReader;
use crate::log_entry::LogEntry;
use anyhow::Result;
use chrono::{TimeDelta, Utc};
use gethostname::gethostname;
use log::{error, info, warn};
use signal_hook::consts::SIGINT;
use signal_hook::iterator::Signals;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

pub struct ApplicationController {
    journal_reader: JournalLogReader,
    buffer: DuckDBBuffer,
    hostname: String,
    shutdown_signal: Arc<AtomicBool>,
}

impl ApplicationController {
    pub fn new<P: AsRef<std::path::Path>>(data_dir: P) -> Result<Self> {
        info!("Initializing Application Controller");

        let journal_reader = JournalLogReader::new()?;
        let buffer = DuckDBBuffer::new(data_dir)?;
        let hostname = gethostname().to_str().unwrap_or("unknown").to_string();
        let shutdown_signal = Arc::new(AtomicBool::new(false));

        info!("Application Controller initialized successfully");
        info!("Using on-disk DuckDB at: {}", buffer.db_path().display());

        Ok(Self {
            journal_reader,
            buffer,
            hostname,
            shutdown_signal,
        })
    }

    pub fn get_shutdown_signal(&self) -> Arc<AtomicBool> {
        self.shutdown_signal.clone()
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
            .process_historical_entries(cutoff_time, |entry| {
                self.buffer.add_entry(entry)
            });
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

    #[test]
    fn test_application_controller_creation() {
        let temp_dir = TempDir::new().unwrap();
        let result = ApplicationController::new(temp_dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_status_retrieval() {
        let temp_dir = TempDir::new().unwrap();
        let mut controller = ApplicationController::new(temp_dir.path()).unwrap();

        let status = controller.get_status().unwrap();
        assert_eq!(status.total_entries, 0);
        assert_eq!(status.distinct_minutes_count, 0);
        assert!(!status.hostname.is_empty());
    }
}
