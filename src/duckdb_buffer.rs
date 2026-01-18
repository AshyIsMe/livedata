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

        // Create the main table for journal logs with proper data types
        conn.execute(
            "CREATE TABLE journal_logs (
                timestamp TIMESTAMP NOT NULL,
                minute_key TIMESTAMP NOT NULL,
                message TEXT,
                priority INTEGER,
                systemd_unit TEXT,
                hostname TEXT,
                pid INTEGER,
                exe TEXT,
                syslog_identifier TEXT,
                syslog_facility TEXT,
                _uid INTEGER,
                _gid INTEGER,
                _comm TEXT,
                extra_fields JSON
            )",
            [],
        )?;

        // Create indexes for efficient querying
        conn.execute(
            "CREATE INDEX idx_minute_key ON journal_logs(minute_key)",
            [],
        )?;
        conn.execute("CREATE INDEX idx_timestamp ON journal_logs(timestamp)", [])?;
        conn.execute("CREATE INDEX idx_priority ON journal_logs(priority)", [])?;
        conn.execute("CREATE INDEX idx_hostname ON journal_logs(hostname)", [])?;
        conn.execute(
            "CREATE INDEX idx_systemd_unit ON journal_logs(systemd_unit)",
            [],
        )?;

        info!("DuckDB buffer initialized successfully");

        Ok(Self { conn })
    }

    pub fn add_entry(&mut self, entry: &LogEntry) -> Result<()> {
        let minute_key = entry.minute_key();

        // Extract common fields with proper type conversions
        let message = entry.get_message().cloned();
        let priority = entry.get_priority().and_then(|p| p.parse::<i32>().ok());
        let systemd_unit = entry.get_systemd_unit().cloned();
        let hostname = entry.get_hostname().cloned();
        let pid = entry.get_pid().and_then(|p| p.parse::<i32>().ok());
        let exe = entry.get_exe().cloned();
        let syslog_identifier = entry.get_field("SYSLOG_IDENTIFIER").cloned();
        let syslog_facility = entry.get_field("SYSLOG_FACILITY").cloned();
        let _uid = entry.get_field("_UID").and_then(|u| u.parse::<i32>().ok());
        let _gid = entry.get_field("_GID").and_then(|g| g.parse::<i32>().ok());
        let _comm = entry.get_field("_COMM").cloned();

        // Create extra_fields JSON with uncommon fields
        let common_fields = std::collections::HashSet::from([
            "MESSAGE".to_string(),
            "PRIORITY".to_string(),
            "_SYSTEMD_UNIT".to_string(),
            "_HOSTNAME".to_string(),
            "_PID".to_string(),
            "_EXE".to_string(),
            "SYSLOG_IDENTIFIER".to_string(),
            "SYSLOG_FACILITY".to_string(),
            "_UID".to_string(),
            "_GID".to_string(),
            "_COMM".to_string(),
        ]);

        let extra_fields: std::collections::HashMap<String, String> = entry
            .fields
            .iter()
            .filter(|(k, _)| !common_fields.contains(*k))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        let extra_fields_json = if extra_fields.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&extra_fields)?)
        };

        self.conn.execute(
            "INSERT INTO journal_logs (
                timestamp, minute_key, message, priority, systemd_unit, hostname, 
                pid, exe, syslog_identifier, syslog_facility, _uid, _gid, _comm, extra_fields
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                entry.timestamp.to_rfc3339(),
                minute_key.to_rfc3339(),
                message,
                priority,
                systemd_unit,
                hostname,
                pid,
                exe,
                syslog_identifier,
                syslog_facility,
                _uid,
                _gid,
                _comm,
                extra_fields_json
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
            "SELECT timestamp, message, priority, systemd_unit, hostname, pid, exe, 
                    syslog_identifier, syslog_facility, _uid, _gid, _comm, extra_fields 
             FROM journal_logs WHERE minute_key = ? ORDER BY timestamp",
        )?;
        let mut rows = stmt.query(params![minute_key.to_rfc3339()])?;

        while let Some(row) = rows.next()? {
            let timestamp_str: String = row.get(0)?;
            let timestamp: DateTime<Utc> = timestamp_str.parse()?;

            // Reconstruct the original fields structure
            let mut fields = serde_json::Map::new();

            // Add common fields if they exist
            if let Some(message) = row.get::<_, Option<String>>(1)? {
                fields.insert("MESSAGE".to_string(), serde_json::Value::String(message));
            }
            if let Some(priority) = row.get::<_, Option<i32>>(2)? {
                fields.insert(
                    "PRIORITY".to_string(),
                    serde_json::Value::String(priority.to_string()),
                );
            }
            if let Some(systemd_unit) = row.get::<_, Option<String>>(3)? {
                fields.insert(
                    "_SYSTEMD_UNIT".to_string(),
                    serde_json::Value::String(systemd_unit),
                );
            }
            if let Some(hostname) = row.get::<_, Option<String>>(4)? {
                fields.insert("_HOSTNAME".to_string(), serde_json::Value::String(hostname));
            }
            if let Some(pid) = row.get::<_, Option<i32>>(5)? {
                fields.insert(
                    "_PID".to_string(),
                    serde_json::Value::String(pid.to_string()),
                );
            }
            if let Some(exe) = row.get::<_, Option<String>>(6)? {
                fields.insert("_EXE".to_string(), serde_json::Value::String(exe));
            }
            if let Some(syslog_identifier) = row.get::<_, Option<String>>(7)? {
                fields.insert(
                    "SYSLOG_IDENTIFIER".to_string(),
                    serde_json::Value::String(syslog_identifier),
                );
            }
            if let Some(syslog_facility) = row.get::<_, Option<String>>(8)? {
                fields.insert(
                    "SYSLOG_FACILITY".to_string(),
                    serde_json::Value::String(syslog_facility),
                );
            }
            if let Some(_uid) = row.get::<_, Option<i32>>(9)? {
                fields.insert(
                    "_UID".to_string(),
                    serde_json::Value::String(_uid.to_string()),
                );
            }
            if let Some(_gid) = row.get::<_, Option<i32>>(10)? {
                fields.insert(
                    "_GID".to_string(),
                    serde_json::Value::String(_gid.to_string()),
                );
            }
            if let Some(_comm) = row.get::<_, Option<String>>(11)? {
                fields.insert("_COMM".to_string(), serde_json::Value::String(_comm));
            }

            // Add extra fields if they exist
            if let Some(extra_fields_json) = row.get::<_, Option<String>>(12)? {
                if let Ok(extra_fields) =
                    serde_json::from_str::<serde_json::Value>(&extra_fields_json)
                {
                    if let serde_json::Value::Object(extra_map) = extra_fields {
                        for (key, value) in extra_map {
                            fields.insert(key, value);
                        }
                    }
                }
            }

            entries.push((timestamp, serde_json::Value::Object(fields)));
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
            let minute_key: DateTime<Utc> = minute_key_str.parse()?;
            minutes.insert(minute_key);
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
        fields.insert("PRIORITY".to_string(), "6".to_string());
        fields.insert("_SYSTEMD_UNIT".to_string(), "test.service".to_string());
        fields.insert("_HOSTNAME".to_string(), "test-host".to_string());
        fields.insert("_PID".to_string(), "1234".to_string());
        fields.insert("CUSTOM_FIELD".to_string(), "custom value".to_string());

        let timestamp = Utc.with_ymd_and_hms(2026, 1, 17, 14, 30, 45).unwrap();
        let entry = LogEntry::new(timestamp, fields);

        let result = buffer.add_entry(&entry);
        assert!(result.is_ok());

        let minute_key = entry.minute_key();
        let retrieved = buffer.get_entries_for_minute(minute_key).unwrap();
        assert_eq!(retrieved.len(), 1);
        assert_eq!(retrieved[0].0, timestamp);

        // Verify field extraction worked correctly
        let fields = retrieved[0].1.as_object().unwrap();
        assert_eq!(
            fields.get("MESSAGE").unwrap().as_str().unwrap(),
            "Test message"
        );
        assert_eq!(fields.get("PRIORITY").unwrap().as_str().unwrap(), "6");
        assert_eq!(
            fields.get("_SYSTEMD_UNIT").unwrap().as_str().unwrap(),
            "test.service"
        );
        assert_eq!(
            fields.get("_HOSTNAME").unwrap().as_str().unwrap(),
            "test-host"
        );
        assert_eq!(fields.get("_PID").unwrap().as_str().unwrap(), "1234");
        assert_eq!(
            fields.get("CUSTOM_FIELD").unwrap().as_str().unwrap(),
            "custom value"
        );
    }

    #[test]
    fn test_buffer_stats() {
        let mut buffer = DuckDBBuffer::new().unwrap();

        let mut fields = std::collections::HashMap::new();
        fields.insert("MESSAGE".to_string(), "Test message".to_string());
        fields.insert("PRIORITY".to_string(), "6".to_string());

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
        fields.insert("PRIORITY".to_string(), "6".to_string());

        let timestamp = Utc.with_ymd_and_hms(2026, 1, 17, 14, 30, 45).unwrap();
        let entry = LogEntry::new(timestamp, fields);

        buffer.add_entry(&entry).unwrap();

        let minute_key = entry.minute_key();
        let rows_deleted = buffer.delete_minute(minute_key).unwrap();
        assert_eq!(rows_deleted, 1);

        let retrieved = buffer.get_entries_for_minute(minute_key).unwrap();
        assert_eq!(retrieved.len(), 0);
    }

    #[test]
    fn test_field_extraction_type_conversions() {
        let mut buffer = DuckDBBuffer::new().unwrap();

        let mut fields = std::collections::HashMap::new();
        fields.insert("MESSAGE".to_string(), "Test message".to_string());
        fields.insert("PRIORITY".to_string(), "6".to_string());
        fields.insert("_PID".to_string(), "1234".to_string());
        fields.insert("_UID".to_string(), "1000".to_string());
        fields.insert("_GID".to_string(), "1000".to_string());
        // Invalid integer values should become NULL
        fields.insert("INVALID_PRIORITY".to_string(), "invalid".to_string());

        let timestamp = Utc.with_ymd_and_hms(2026, 1, 17, 14, 30, 45).unwrap();
        let entry = LogEntry::new(timestamp, fields);

        buffer.add_entry(&entry).unwrap();

        let minute_key = entry.minute_key();
        let retrieved = buffer.get_entries_for_minute(minute_key).unwrap();
        assert_eq!(retrieved.len(), 1);

        let fields_obj = retrieved[0].1.as_object().unwrap();
        assert_eq!(
            fields_obj.get("MESSAGE").unwrap().as_str().unwrap(),
            "Test message"
        );
        assert_eq!(fields_obj.get("PRIORITY").unwrap().as_str().unwrap(), "6");
        assert_eq!(fields_obj.get("_PID").unwrap().as_str().unwrap(), "1234");
        assert_eq!(fields_obj.get("_UID").unwrap().as_str().unwrap(), "1000");
        assert_eq!(fields_obj.get("_GID").unwrap().as_str().unwrap(), "1000");
    }

    #[test]
    fn test_extra_fields_preservation() {
        let mut buffer = DuckDBBuffer::new().unwrap();

        let mut fields = std::collections::HashMap::new();
        fields.insert("MESSAGE".to_string(), "Test message".to_string());
        fields.insert("CUSTOM_FIELD_1".to_string(), "custom value 1".to_string());
        fields.insert("CUSTOM_FIELD_2".to_string(), "custom value 2".to_string());

        let timestamp = Utc.with_ymd_and_hms(2026, 1, 17, 14, 30, 45).unwrap();
        let entry = LogEntry::new(timestamp, fields);

        buffer.add_entry(&entry).unwrap();

        let minute_key = entry.minute_key();
        let retrieved = buffer.get_entries_for_minute(minute_key).unwrap();
        assert_eq!(retrieved.len(), 1);

        let fields_obj = retrieved[0].1.as_object().unwrap();
        assert_eq!(
            fields_obj.get("MESSAGE").unwrap().as_str().unwrap(),
            "Test message"
        );
        assert_eq!(
            fields_obj.get("CUSTOM_FIELD_1").unwrap().as_str().unwrap(),
            "custom value 1"
        );
        assert_eq!(
            fields_obj.get("CUSTOM_FIELD_2").unwrap().as_str().unwrap(),
            "custom value 2"
        );
    }

    #[test]
    fn test_nullable_fields_handling() {
        let mut buffer = DuckDBBuffer::new().unwrap();

        let mut fields = std::collections::HashMap::new();
        // Only include MESSAGE, other fields should be NULL
        fields.insert("MESSAGE".to_string(), "Test message".to_string());

        let timestamp = Utc.with_ymd_and_hms(2026, 1, 17, 14, 30, 45).unwrap();
        let entry = LogEntry::new(timestamp, fields);

        buffer.add_entry(&entry).unwrap();

        let minute_key = entry.minute_key();
        let retrieved = buffer.get_entries_for_minute(minute_key).unwrap();
        assert_eq!(retrieved.len(), 1);

        let fields_obj = retrieved[0].1.as_object().unwrap();
        assert_eq!(
            fields_obj.get("MESSAGE").unwrap().as_str().unwrap(),
            "Test message"
        );
        // Other common fields should not be present since they were NULL
        assert!(fields_obj.get("PRIORITY").is_none());
        assert!(fields_obj.get("_PID").is_none());
        assert!(fields_obj.get("_SYSTEMD_UNIT").is_none());
    }
}
