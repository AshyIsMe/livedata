use crate::duckdb_buffer::DuckDBBuffer;
use crate::journal_reader::JournalLogReader;
use crate::log_entry::LogEntry;
use crate::parquet_writer::ParquetWriter;
use anyhow::Result;
use chrono::{TimeDelta, Utc};
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
    parquet_writer: ParquetWriter,
    shutdown_signal: Arc<AtomicBool>,
}

impl ApplicationController {
    pub fn new<P: AsRef<std::path::Path>>(data_dir: P) -> Result<Self> {
        info!("Initializing Application Controller");

        let journal_reader = JournalLogReader::new()?;
        let buffer = DuckDBBuffer::new()?;
        let parquet_writer = ParquetWriter::new(data_dir)?;
        let shutdown_signal = Arc::new(AtomicBool::new(false));

        info!("Application Controller initialized successfully");

        Ok(Self {
            journal_reader,
            buffer,
            parquet_writer,
            shutdown_signal,
        })
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

    pub fn run(&mut self) -> Result<()> {
        info!("Starting journald to parquet log collection");

        self.setup_signal_handler()?;

        let mut last_flush_time = Utc::now();
        let flush_interval = TimeDelta::seconds(30); // Check for completed minutes every 30 seconds

        info!("Starting main loop");

        loop {
            // Check for shutdown signal
            if self.shutdown_signal.load(Ordering::Relaxed) {
                info!("Shutdown signal received, initiating graceful shutdown");
                break;
            }

            // Process any new journal entries
            match self.journal_reader.next_log_entry() {
                Ok(Some(entry)) => {
                    if let Err(e) = self.process_log_entry(entry) {
                        error!("Failed to process log entry: {}", e);
                    }
                }
                Ok(None) => {
                    // No new entries, just continue
                }
                Err(e) => {
                    error!("Error reading from journal: {}", e);
                    // Wait a bit before retrying
                    thread::sleep(Duration::from_secs(1));
                }
            }

            // Check if it's time to flush completed minutes
            let current_time = Utc::now();
            if current_time - last_flush_time >= flush_interval {
                if let Err(e) = self.flush_completed_minutes(current_time) {
                    error!("Failed to flush completed minutes: {}", e);
                }
                last_flush_time = current_time;

                // Log status periodically
                self.log_status();
            }

            // Small sleep to prevent busy waiting
            thread::sleep(Duration::from_millis(100));
        }

        // Graceful shutdown - flush all remaining data
        self.graceful_shutdown()
    }

    fn process_log_entry(&mut self, entry: LogEntry) -> Result<()> {
        // Add entry to buffer
        self.buffer.add_entry(&entry)?;

        // Log processing stats (using a field counter for now)
        // In a real implementation, we'd track this more carefully
        info!("Processing log entry (timestamp: {})", entry.timestamp);

        Ok(())
    }

    fn flush_completed_minutes(&mut self, current_time: chrono::DateTime<Utc>) -> Result<()> {
        let results = self
            .parquet_writer
            .write_completed_minutes(&mut self.buffer, current_time)?;

        if !results.is_empty() {
            let total_entries: i64 = results.iter().map(|r| r.entries_written).sum();
            let total_bytes: u64 = results.iter().map(|r| r.bytes_written).sum();

            info!(
                "Flushed {} completed minutes: {} entries ({} bytes)",
                results.len(),
                total_entries,
                total_bytes
            );
        }

        Ok(())
    }

    fn graceful_shutdown(&mut self) -> Result<()> {
        info!("Starting graceful shutdown");

        // Flush all remaining minutes
        match self.parquet_writer.flush_all_minutes(&mut self.buffer) {
            Ok(results) => {
                if !results.is_empty() {
                    let total_entries: i64 = results.iter().map(|r| r.entries_written).sum();
                    let total_bytes: u64 = results.iter().map(|r| r.bytes_written).sum();

                    info!(
                        "Final flush: {} minutes, {} entries ({} bytes)",
                        results.len(),
                        total_entries,
                        total_bytes
                    );
                } else {
                    info!("No remaining data to flush");
                }
            }
            Err(e) => {
                error!("Failed to flush remaining data: {}", e);
            }
        }

        // Log final statistics
        self.log_final_statistics();

        info!("Graceful shutdown completed");
        Ok(())
    }

    fn log_status(&mut self) {
        match self.buffer.get_buffer_stats() {
            Ok(stats) => {
                if stats.total_entries > 0 {
                    info!(
                        "Status: {} entries in buffer, {} minutes buffered",
                        stats.total_entries, stats.buffered_minutes_count
                    );

                    if let (Some(oldest), Some(newest)) =
                        (&stats.oldest_minute, &stats.newest_minute)
                    {
                        info!("Buffer range: {} to {}", oldest, newest);
                    }
                }
            }
            Err(e) => {
                warn!("Failed to get buffer status: {}", e);
            }
        }

        // Log parquet writer statistics
        match self.parquet_writer.get_file_count() {
            Ok(file_count) => {
                if file_count > 0 {
                    match self.parquet_writer.get_disk_usage() {
                        Ok(bytes) => {
                            info!(
                                "Parquet files: {} files, {} MB",
                                file_count,
                                bytes / (1024 * 1024)
                            );
                        }
                        Err(e) => {
                            warn!("Failed to get disk usage: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                warn!("Failed to get file count: {}", e);
            }
        }
    }

    fn log_final_statistics(&mut self) {
        info!("=== Final Statistics ===");

        // Buffer statistics
        match self.buffer.get_buffer_stats() {
            Ok(stats) => {
                info!(
                    "Final buffer: {} entries, {} minutes buffered",
                    stats.total_entries, stats.buffered_minutes_count
                );
            }
            Err(e) => {
                warn!("Failed to get final buffer stats: {}", e);
            }
        }

        // Parquet writer statistics
        match self.parquet_writer.get_file_count() {
            Ok(file_count) => match self.parquet_writer.get_disk_usage() {
                Ok(bytes) => {
                    info!(
                        "Final parquet output: {} files, {} MB",
                        file_count,
                        bytes / (1024 * 1024)
                    );
                }
                Err(e) => {
                    warn!("Failed to get final disk usage: {}", e);
                }
            },
            Err(e) => {
                warn!("Failed to get final file count: {}", e);
            }
        }

        info!("=== End Statistics ===");
    }

    pub fn get_status(&mut self) -> Result<ApplicationStatus> {
        let buffer_stats = self.buffer.get_buffer_stats()?;
        let file_count = self.parquet_writer.get_file_count()?;
        let disk_usage = self.parquet_writer.get_disk_usage()?;

        Ok(ApplicationStatus {
            hostname: self.parquet_writer.get_hostname().to_string(),
            total_buffered_entries: buffer_stats.total_entries,
            buffered_minutes_count: buffer_stats.buffered_minutes_count,
            oldest_buffered_minute: buffer_stats.oldest_minute,
            newest_buffered_minute: buffer_stats.newest_minute,
            parquet_file_count: file_count,
            total_disk_usage_bytes: disk_usage,
        })
    }
}

#[derive(Debug)]
pub struct ApplicationStatus {
    pub hostname: String,
    pub total_buffered_entries: i64,
    pub buffered_minutes_count: usize,
    pub oldest_buffered_minute: Option<chrono::DateTime<Utc>>,
    pub newest_buffered_minute: Option<chrono::DateTime<Utc>>,
    pub parquet_file_count: usize,
    pub total_disk_usage_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_application_controller_creation() {
        let temp_dir = TempDir::new().unwrap();
        let result = ApplicationController::new(&temp_dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_status_retrieval() {
        let temp_dir = TempDir::new().unwrap();
        let mut controller = ApplicationController::new(&temp_dir.path()).unwrap();

        let status = controller.get_status().unwrap();
        assert_eq!(status.total_buffered_entries, 0);
        assert_eq!(status.buffered_minutes_count, 0);
        assert_eq!(status.parquet_file_count, 0);
        assert!(!status.hostname.is_empty());
    }
}
