use crate::log_entry::LogEntry;
use anyhow::Result;
use chrono::{DateTime, Utc};
use duckdb::{Connection, params};
use log::{debug, info};
use serde_json::Value;
use std::collections::HashSet;

pub struct DuckDBBuffer {
    pub conn: Connection,
}

impl DuckDBBuffer {
    pub fn new() -> Result<Self> {
        info!("Initializing DuckDB in-memory buffer");
        let conn = Connection::open_in_memory()?;

        // Create the main table for journal logs
        conn.execute(
            "CREATE TABLE journal_logs (
                timestamp TEXT NOT NULL,
                minute_key TEXT NOT NULL,
                fields JSON NOT NULL
            )",
            [],
        )?;

        // Create indexes for efficient querying
        conn.execute(
            "CREATE INDEX idx_minute_key ON journal_logs(minute_key)",
            [],
        )?;
        conn.execute("CREATE INDEX idx_timestamp ON journal_logs(timestamp)", [])?;

        info!("DuckDB buffer initialized successfully");

        Ok(Self { conn })
    }

    pub fn add_entry(&mut self, entry: &LogEntry) -> Result<()> {
        let minute_key = entry.minute_key();
        let fields_json = serde_json::to_string(&entry.fields)?;

        self.conn.execute(
            "INSERT INTO journal_logs (timestamp, minute_key, fields) VALUES (?, ?, ?)",
            params![
                entry.timestamp.to_rfc3339(),
                minute_key.to_rfc3339(),
                fields_json
            ],
        )?;

        Ok(())
    }

    pub fn get_entries_for_minute(
        &mut self,
        minute_key: DateTime<Utc>,
    ) -> Result<Vec<(DateTime<Utc>, Value)>> {
        let mut entries = Vec::new();
        let mut stmt = self.conn.prepare(
            "SELECT timestamp, fields FROM journal_logs WHERE minute_key = ? ORDER BY timestamp",
        )?;
        let mut rows = stmt.query(params![minute_key.to_rfc3339()])?;

        while let Some(row) = rows.next()? {
            let timestamp_str: String = row.get(0)?;
            let fields_json: String = row.get(1)?;
            let fields: Value = serde_json::from_str(&fields_json)?;

            let timestamp_utc: DateTime<Utc> = timestamp_str.parse()?;
            entries.push((timestamp_utc, fields));
        }

        Ok(entries)
    }

    pub fn delete_minute(&mut self, minute_key: DateTime<Utc>) -> Result<usize> {
        let rows_deleted = self.conn.execute(
            "DELETE FROM journal_logs WHERE minute_key = ?",
            params![minute_key.to_rfc3339()],
        )?;
        Ok(rows_deleted)
    }

    pub fn get_buffered_minutes(&mut self) -> Result<HashSet<DateTime<Utc>>> {
        let mut minutes = HashSet::new();
        let mut stmt = self
            .conn
            .prepare("SELECT DISTINCT minute_key FROM journal_logs")?;
        let mut rows = stmt.query([])?;

        while let Some(row) = rows.next()? {
            let minute_key_str: String = row.get(0)?;
            let minute_key_utc: DateTime<Utc> = minute_key_str.parse()?;
            minutes.insert(minute_key_utc);
        }

        Ok(minutes)
    }

    pub fn count_entries(&mut self) -> Result<i64> {
        let mut stmt = self.conn.prepare("SELECT COUNT(*) FROM journal_logs")?;
        let mut rows = stmt.query([])?;

        if let Some(row) = rows.next()? {
            Ok(row.get(0)?)
        } else {
            Ok(0)
        }
    }

    pub fn count_entries_for_minute(&mut self, minute_key: DateTime<Utc>) -> Result<i64> {
        let mut stmt = self
            .conn
            .prepare("SELECT COUNT(*) FROM journal_logs WHERE minute_key = ?")?;
        let mut rows = stmt.query(params![minute_key.to_rfc3339()])?;

        if let Some(row) = rows.next()? {
            Ok(row.get(0)?)
        } else {
            Ok(0)
        }
    }

    pub fn get_oldest_minute(&mut self) -> Result<Option<DateTime<Utc>>> {
        let mut stmt = self
            .conn
            .prepare("SELECT MIN(minute_key) FROM journal_logs")?;
        let mut rows = stmt.query([])?;

        if let Some(row) = rows.next()? {
            let minute_key_str: Option<String> = row.get(0)?;
            match minute_key_str {
                Some(s) => {
                    let dt: DateTime<Utc> = s.parse()?;
                    Ok(Some(dt))
                }
                None => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    pub fn get_newest_minute(&mut self) -> Result<Option<DateTime<Utc>>> {
        let mut stmt = self
            .conn
            .prepare("SELECT MAX(minute_key) FROM journal_logs")?;
        let mut rows = stmt.query([])?;

        if let Some(row) = rows.next()? {
            let minute_key_str: Option<String> = row.get(0)?;
            match minute_key_str {
                Some(s) => {
                    let dt: DateTime<Utc> = s.parse()?;
                    Ok(Some(dt))
                }
                None => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    pub fn clear_all(&mut self) -> Result<()> {
        debug!("Clearing all buffered entries");
        self.conn.execute("DELETE FROM journal_logs", [])?;
        Ok(())
    }

    pub fn vacuum(&mut self) -> Result<()> {
        debug!("Running VACUUM to optimize database");
        self.conn.execute("VACUUM", [])?;
        Ok(())
    }

    pub fn get_buffer_stats(&mut self) -> Result<BufferStats> {
        let total_entries = self.count_entries()?;
        let buffered_minutes = self.get_buffered_minutes()?;
        let oldest_minute = self.get_oldest_minute()?;
        let newest_minute = self.get_newest_minute()?;

        Ok(BufferStats {
            total_entries,
            buffered_minutes_count: buffered_minutes.len(),
            oldest_minute,
            newest_minute,
        })
    }
}

#[derive(Debug)]
pub struct BufferStats {
    pub total_entries: i64,
    pub buffered_minutes_count: usize,
    pub oldest_minute: Option<DateTime<Utc>>,
    pub newest_minute: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_duckdb_buffer_creation() {
        let result = DuckDBBuffer::new();
        assert!(result.is_ok());
    }

    #[test]
    fn test_add_and_retrieve_entry() {
        let mut buffer = DuckDBBuffer::new().unwrap();

        let mut fields = std::collections::HashMap::new();
        fields.insert("MESSAGE".to_string(), "Test message".to_string());

        let timestamp = Utc.with_ymd_and_hms(2026, 1, 17, 14, 30, 45).unwrap();
        let entry = LogEntry::new(timestamp, fields);

        let result = buffer.add_entry(&entry);
        assert!(result.is_ok());

        let minute_key = entry.minute_key();
        let retrieved = buffer.get_entries_for_minute(minute_key).unwrap();
        assert_eq!(retrieved.len(), 1);
        assert_eq!(retrieved[0].0, timestamp);
    }

    #[test]
    fn test_buffer_stats() {
        let mut buffer = DuckDBBuffer::new().unwrap();

        let mut fields = std::collections::HashMap::new();
        fields.insert("MESSAGE".to_string(), "Test message".to_string());

        let timestamp = Utc.with_ymd_and_hms(2026, 1, 17, 14, 30, 45).unwrap();
        let entry = LogEntry::new(timestamp, fields);

        buffer.add_entry(&entry).unwrap();

        let stats = buffer.get_buffer_stats().unwrap();
        assert_eq!(stats.total_entries, 1);
        assert_eq!(stats.buffered_minutes_count, 1);
    }

    #[test]
    fn test_delete_minute() {
        let mut buffer = DuckDBBuffer::new().unwrap();

        let mut fields = std::collections::HashMap::new();
        fields.insert("MESSAGE".to_string(), "Test message".to_string());

        let timestamp = Utc.with_ymd_and_hms(2026, 1, 17, 14, 30, 45).unwrap();
        let entry = LogEntry::new(timestamp, fields);

        buffer.add_entry(&entry).unwrap();

        let minute_key = entry.minute_key();
        let rows_deleted = buffer.delete_minute(minute_key).unwrap();
        assert_eq!(rows_deleted, 1);

        let retrieved = buffer.get_entries_for_minute(minute_key).unwrap();
        assert_eq!(retrieved.len(), 0);
    }
}
