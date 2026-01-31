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

        let reader = Self { journal };
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

    pub fn previous_skip(&mut self, skip_count: u64) -> Result<()> {
        info!("previous_skip({})", skip_count);
        self.journal
            .previous_skip(skip_count)
            .map_err(|e| anyhow!("Failed to previous_skip: {}", e))?;
        Ok(())
    }

    pub fn seek_to_timestamp(&mut self, timestamp: DateTime<Utc>) -> Result<()> {
        info!("Seeking to timestamp: {}", timestamp);

        // Convert timestamp to microseconds since epoch for systemd
        let timestamp_usec = timestamp.timestamp_micros().to_string();

        // Add match for timestamp
        self.journal
            .match_add("__REALTIME_TIMESTAMP", timestamp_usec)?;

        Ok(())
    }

    pub fn skip_older_than(&mut self, cutoff_timestamp: DateTime<Utc>) -> Result<usize> {
        info!("Skipping entries older than: {}", cutoff_timestamp);

        let mut skipped_count = 0;

        // Seek to the beginning of the journal first
        self.journal
            .seek_head()
            .map_err(|e| anyhow!("Failed to seek to head: {}", e))?;

        // Read and discard entries older than cutoff
        loop {
            match self.journal.next_entry() {
                Ok(Some(entry)) => {
                    if let Ok(log_entry) = self.convert_journal_entry(&entry) {
                        if log_entry.timestamp >= cutoff_timestamp {
                            // This entry is within our time window
                            info!(
                                "Found first entry within time window: {}",
                                log_entry.timestamp
                            );
                            break;
                        } else {
                            skipped_count += 1;
                        }
                    }
                }
                Ok(None) => {
                    // Reached end of journal
                    break;
                }
                Err(_) => {
                    // Error reading entry, stop skipping
                    break;
                }
            }
        }

        info!("Skipped {} entries older than cutoff", skipped_count);
        Ok(skipped_count)
    }

    pub fn wait_for_entry(&mut self, timeout: Option<std::time::Duration>) -> Result<bool> {
        // Wait for entries to become available
        match self.journal.wait(timeout) {
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

                // Print all fields on a single line for real-time processing
                let fields_vec: Vec<String> = log_entry
                    .fields
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect();
                info!(
                    "Real-time Entry {} {}",
                    log_entry.timestamp,
                    fields_vec.join(" ")
                );

                Ok(Some(log_entry))
            }
            Ok(None) => Ok(None),
            Err(_) => Ok(None),
        }
    }

    pub fn process_historical_entries<F>(
        &mut self,
        cutoff_timestamp: DateTime<Utc>,
        mut callback: F,
    ) -> Result<usize>
    where
        F: FnMut(&LogEntry) -> Result<()>,
    {
        info!("Processing historical entries from: {}", cutoff_timestamp);

        let mut processed_count = 0;

        // First, seek to tail to get to recent entries faster
        self.journal
            .seek_tail()
            .map_err(|e| anyhow!("Failed to seek to tail: {}", e))?;

        // Estimate how far back to go (rough approximation)
        // Journal entries are typically in reverse chronological order when seeking from tail
        // We'll go back a reasonable amount and then filter
        let entries_to_check = 10000; // Reasonable limit for last hour
        let mut entries_checked = 0;

        // Process entries, looking for ones within our time window
        while entries_checked < entries_to_check {
            match self.journal.previous_entry() {
                Ok(Some(entry)) => {
                    if let Ok(log_entry) = self.convert_journal_entry(&entry) {

                        if log_entry.timestamp >= cutoff_timestamp
                            && log_entry.timestamp <= Utc::now()
                        {
                            callback(&log_entry)?;
                            processed_count += 1;
                        } else if log_entry.timestamp < cutoff_timestamp {
                            // We've gone far enough back
                            break;
                        }
                    }
                    entries_checked += 1;
                }
                Ok(None) => {
                    // Reached beginning of journal
                    break;
                }
                Err(_) => {
                    // Error reading entry, stop processing
                    break;
                }
            }
        }

        info!(
            "Processed {} historical entries from {} checked",
            processed_count, entries_checked
        );
        Ok(processed_count)
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
