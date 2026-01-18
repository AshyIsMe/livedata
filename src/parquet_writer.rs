use crate::duckdb_buffer::DuckDBBuffer;
use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use gethostname::gethostname;
use log::{debug, error, info, warn};
use std::fs;
use std::path::{Path, PathBuf};

pub struct ParquetWriter {
    base_dir: PathBuf,
    hostname: String,
}

impl ParquetWriter {
    pub fn new<P: AsRef<Path>>(base_dir: P) -> Result<Self> {
        let hostname = gethostname()
            .to_str()
            .ok_or_else(|| anyhow!("Invalid hostname"))?
            .to_string();

        let base_dir = base_dir.as_ref().to_path_buf();

        info!(
            "Initializing ParquetWriter for hostname: {} in directory: {}",
            hostname,
            base_dir.display()
        );

        // Create base directory if it doesn't exist
        fs::create_dir_all(&base_dir)?;

        Ok(Self { base_dir, hostname })
    }

    pub fn get_hostname(&self) -> &str {
        &self.hostname
    }

    pub fn create_directory_structure(&self, timestamp: DateTime<Utc>) -> Result<PathBuf> {
        let year = timestamp.format("%Y").to_string();
        let month = timestamp.format("%m").to_string();
        let day = timestamp.format("%d").to_string();

        let dir_path = self
            .base_dir
            .join(&self.hostname)
            .join(year)
            .join(month)
            .join(day);

        fs::create_dir_all(&dir_path)?;

        debug!("Created directory structure: {}", dir_path.display());
        Ok(dir_path)
    }

    pub fn generate_filename(timestamp: DateTime<Utc>, data_source: &str) -> String {
        timestamp
            .format(&format!("%Y%m%d-%H%M-{}.parquet", data_source))
            .to_string()
    }

    pub fn get_filepath_for_minute(
        &self,
        minute_key: DateTime<Utc>,
        data_source: &str,
    ) -> Result<PathBuf> {
        let dir_path = self.create_directory_structure(minute_key)?;
        let filename = Self::generate_filename(minute_key, data_source);
        Ok(dir_path.join(filename))
    }

    pub fn write_minute_to_parquet(
        &mut self,
        buffer: &mut DuckDBBuffer,
        minute_key: DateTime<Utc>,
        data_source: &str,
    ) -> Result<WriteResult> {
        let filepath = self.get_filepath_for_minute(minute_key, data_source)?;

        // Check if file already exists
        if filepath.exists() {
            warn!(
                "Parquet file already exists: {}, skipping",
                filepath.display()
            );
            return Ok(WriteResult {
                filepath,
                entries_written: 0,
                bytes_written: 0,
                skipped: true,
            });
        }

        info!(
            "Writing parquet file for minute: {} -> {}",
            minute_key,
            filepath.display()
        );

        // Use buffer's connection for export
        let export_conn = &buffer.conn;

        // Use DuckDB's COPY command to export to parquet with new schema
        let copy_sql = format!(
            "COPY (SELECT timestamp, minute_key, message, priority, systemd_unit, hostname, 
                    pid, exe, syslog_identifier, syslog_facility, _uid, _gid, _comm, extra_fields 
             FROM journal_logs WHERE minute_key = '{}' ORDER BY timestamp) 
             TO '{}' (FORMAT PARQUET, COMPRESSION SNAPPY)",
            minute_key.to_rfc3339(),
            filepath.display()
        );

        debug!("Executing COPY SQL: {}", copy_sql);

        match export_conn.execute(&copy_sql, []) {
            Ok(_) => {
                // Get file size
                let bytes_written = fs::metadata(&filepath).map(|m| m.len()).unwrap_or(0);

                // Count entries written
                let entries_written = buffer.count_entries_for_minute(minute_key)?;

                info!(
                    "Successfully wrote {} entries ({} bytes) to {}",
                    entries_written,
                    bytes_written,
                    filepath.display()
                );

                Ok(WriteResult {
                    filepath,
                    entries_written,
                    bytes_written,
                    skipped: false,
                })
            }
            Err(e) => {
                error!("Failed to write parquet file: {}", e);
                Err(anyhow!("Parquet write failed: {}", e))
            }
        }
    }

    pub fn write_and_cleanup_minute(
        &mut self,
        buffer: &mut DuckDBBuffer,
        minute_key: DateTime<Utc>,
        data_source: &str,
    ) -> Result<WriteResult> {
        let write_result = self.write_minute_to_parquet(buffer, minute_key, data_source)?;

        if !write_result.skipped {
            // Delete the exported entries from buffer
            let rows_deleted = buffer.delete_minute(minute_key)?;

            if rows_deleted as i64 != write_result.entries_written {
                warn!(
                    "Mismatch: wrote {} entries but deleted {} rows",
                    write_result.entries_written, rows_deleted
                );
            }

            debug!(
                "Deleted {} entries from buffer for minute {}",
                rows_deleted, minute_key
            );
        }

        Ok(write_result)
    }

    pub fn write_completed_minutes(
        &mut self,
        buffer: &mut DuckDBBuffer,
        current_time: DateTime<Utc>,
    ) -> Result<Vec<WriteResult>> {
        let mut results = Vec::new();
        let buffered_minutes = buffer.get_buffered_minutes()?;

        for minute_key in buffered_minutes {
            // Check if this minute is completed (current time > minute + 60 seconds)
            let completion_time = minute_key + chrono::Duration::seconds(60);

            if current_time >= completion_time {
                debug!("Processing completed minute: {}", minute_key);

                match self.write_and_cleanup_minute(buffer, minute_key, "journald") {
                    Ok(result) => {
                        if !result.skipped {
                            info!(
                                "Successfully processed minute {}: {} entries",
                                minute_key, result.entries_written
                            );
                        }
                        results.push(result);
                    }
                    Err(e) => {
                        error!("Failed to process minute {}: {}", minute_key, e);
                        // Continue with other minutes
                    }
                }
            }
        }

        Ok(results)
    }

    pub fn flush_all_minutes(&mut self, buffer: &mut DuckDBBuffer) -> Result<Vec<WriteResult>> {
        let mut results = Vec::new();
        let buffered_minutes = buffer.get_buffered_minutes()?;

        info!("Flushing all {} buffered minutes", buffered_minutes.len());

        for minute_key in buffered_minutes {
            debug!("Flushing minute: {}", minute_key);

            match self.write_and_cleanup_minute(buffer, minute_key, "journald") {
                Ok(result) => {
                    if !result.skipped {
                        info!(
                            "Successfully flushed minute {}: {} entries",
                            minute_key, result.entries_written
                        );
                    }
                    results.push(result);
                }
                Err(e) => {
                    error!("Failed to flush minute {}: {}", minute_key, e);
                    // Continue with other minutes
                }
            }
        }

        Ok(results)
    }

    pub fn get_disk_usage(&self) -> Result<u64> {
        let total_size = self.calculate_directory_size(&self.base_dir)?;
        Ok(total_size)
    }

    pub fn get_file_count(&self) -> Result<usize> {
        let count = self.count_parquet_files(&self.base_dir)?;
        Ok(count)
    }

    fn calculate_directory_size(&self, path: &Path) -> Result<u64> {
        let mut total_size = 0u64;

        if path.is_dir() {
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                let entry_path = entry.path();

                if entry_path.is_dir() {
                    total_size += self.calculate_directory_size(&entry_path)?;
                } else if entry_path.extension().is_some_and(|ext| ext == "parquet") {
                    total_size += entry.metadata()?.len();
                }
            }
        }

        Ok(total_size)
    }

    fn count_parquet_files(&self, path: &Path) -> Result<usize> {
        let mut count = 0;

        if path.is_dir() {
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                let entry_path = entry.path();

                if entry_path.is_dir() {
                    count += self.count_parquet_files(&entry_path)?;
                } else if entry_path.extension().is_some_and(|ext| ext == "parquet") {
                    count += 1;
                }
            }
        }

        Ok(count)
    }
}

#[derive(Debug)]
pub struct WriteResult {
    pub filepath: PathBuf,
    pub entries_written: i64,
    pub bytes_written: u64,
    pub skipped: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use tempfile::TempDir;

    #[test]
    fn test_parquet_writer_creation() {
        let temp_dir = TempDir::new().unwrap();
        let result = ParquetWriter::new(&temp_dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_directory_structure() {
        let temp_dir = TempDir::new().unwrap();
        let writer = ParquetWriter::new(&temp_dir.path()).unwrap();

        let timestamp = Utc.with_ymd_and_hms(2026, 1, 17, 14, 30, 0).unwrap();
        let dir_path = writer.create_directory_structure(timestamp).unwrap();

        let expected_path = temp_dir
            .path()
            .join(writer.get_hostname())
            .join("2026")
            .join("01")
            .join("17");

        assert_eq!(dir_path, expected_path);
        assert!(dir_path.exists());
        assert!(dir_path.is_dir());
    }

    #[test]
    fn test_filename_generation() {
        let timestamp = Utc.with_ymd_and_hms(2026, 1, 17, 14, 30, 0).unwrap();
        let filename = ParquetWriter::generate_filename(timestamp, "journald");
        assert_eq!(filename, "20260117-1430-journald.parquet");
    }

    #[test]
    fn test_filepath_generation() {
        let temp_dir = TempDir::new().unwrap();
        let writer = ParquetWriter::new(&temp_dir.path()).unwrap();

        let timestamp = Utc.with_ymd_and_hms(2026, 1, 17, 14, 30, 0).unwrap();
        let filepath = writer
            .get_filepath_for_minute(timestamp, "journald")
            .unwrap();

        let expected_path = temp_dir
            .path()
            .join(writer.get_hostname())
            .join("2026")
            .join("01")
            .join("17")
            .join("20260117-1430-journald.parquet");

        assert_eq!(filepath, expected_path);
    }
}
