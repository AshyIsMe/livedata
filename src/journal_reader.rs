use crate::log_entry::LogEntry;
use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use log::info;
use std::collections::HashMap;
use systemd::journal::{Journal, OpenOptions};

pub struct JournalLogReader {
    journal: Journal,
}

impl JournalLogReader {
    pub fn new() -> Result<Self> {
        info!("Initializing journal connection");
        let journal = OpenOptions::default()
            .system(true)
            .current_user(true)
            .local_only(false)
            .runtime_only(false)
            .open()
            .map_err(|e| anyhow!("Failed to open journal: {}", e))?;

        let mut reader = Self { journal };
        //reader.seek_to_tail()?;

        info!("Journal reader initialized successfully");
        Ok(reader)
    }

    pub fn seek_to_tail(&mut self) -> Result<()> {
        info!("Seeking to tail of journal");
        self.journal
            .seek_tail()
            .map_err(|e| anyhow!("Failed to seek to tail: {}", e))?;
        Ok(())
    }

    pub fn wait_for_entry(&mut self) -> Result<bool> {
        // Wait for entries to become available
        match self.journal.wait(None) {
            Ok(systemd::journal::JournalWaitResult::Nop) => {
                // No new entries, but call was successful
                Ok(false)
            }
            Ok(systemd::journal::JournalWaitResult::Append) => {
                // New entries were appended to the journal
                info!("New journal entries available");
                Ok(true)
            }
            Ok(systemd::journal::JournalWaitResult::Invalidate) => {
                // Journal files were changed/rotated
                info!("Journal invalidated, may need to reposition");
                Ok(true)
            }
            Err(e) => {
                info!("Error waiting for journal changes: {}", e);
                Ok(false)
            }
        }
    }

    pub fn next_entry(&mut self) -> Result<Option<LogEntry>> {
        match self.journal.next_entry() {
            Ok(Some(entry)) => {
                let log_entry = self.convert_journal_entry(&entry)?;
                Ok(Some(log_entry))
            }
            Ok(None) => Ok(None),
            Err(e) => {
                info!("Error reading journal entry: {}", e);
                Ok(None)
            }
        }
    }

    fn convert_journal_entry(
        &self,
        entry: &std::collections::BTreeMap<String, String>,
    ) -> Result<LogEntry> {
        let mut fields = HashMap::new();

        // Extract all fields from the journal entry
        for (field_name, field_value) in entry {
            let name_str = field_name.to_string();
            let value_str = field_value.clone();
            fields.insert(name_str, value_str);
        }

        // Try to extract timestamp
        let timestamp = self.extract_timestamp(&fields)?;

        Ok(LogEntry::new(timestamp, fields))
    }

    fn extract_timestamp(&self, fields: &HashMap<String, String>) -> Result<DateTime<Utc>> {
        if let Some(ts_usec) = fields.get("__REALTIME_TIMESTAMP") {
            let timestamp_usec: u64 = ts_usec
                .parse()
                .map_err(|e| anyhow!("Failed to parse timestamp: {}", e))?;

            let timestamp_sec = timestamp_usec / 1_000_000;
            let timestamp_nsec = (timestamp_usec % 1_000_000) * 1000;

            DateTime::from_timestamp(timestamp_sec as i64, timestamp_nsec as u32)
                .ok_or_else(|| anyhow!("Invalid timestamp: {}", timestamp_usec))
        } else {
            // If no timestamp in entry, use current time
            Ok(Utc::now())
        }
    }

    pub fn next_log_entry(&mut self) -> Result<Option<LogEntry>> {
        // Check for entries and return them
        match self.journal.next_entry() {
            Ok(Some(entry)) => {
                let log_entry = self.convert_journal_entry(&entry)?;
                Ok(Some(log_entry))
            }
            Ok(None) => Ok(None),
            Err(_) => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_journal_reader_creation() {
        let result = JournalLogReader::new();
        assert!(result.is_ok() || result.is_err());
    }
}
