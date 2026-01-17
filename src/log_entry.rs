use chrono::{DateTime, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub fields: HashMap<String, String>,
}

impl LogEntry {
    pub fn new(timestamp: DateTime<Utc>, fields: HashMap<String, String>) -> Self {
        Self { timestamp, fields }
    }

    pub fn get_field(&self, key: &str) -> Option<&String> {
        self.fields.get(key)
    }

    pub fn get_message(&self) -> Option<&String> {
        self.get_field("MESSAGE")
    }

    pub fn get_priority(&self) -> Option<&String> {
        self.get_field("PRIORITY")
    }

    pub fn get_systemd_unit(&self) -> Option<&String> {
        self.get_field("_SYSTEMD_UNIT")
    }

    pub fn get_hostname(&self) -> Option<&String> {
        self.get_field("_HOSTNAME")
    }

    pub fn get_pid(&self) -> Option<&String> {
        self.get_field("_PID")
    }

    pub fn get_exe(&self) -> Option<&String> {
        self.get_field("_EXE")
    }

    pub fn minute_key(&self) -> DateTime<Utc> {
        let mut minute_key = self.timestamp;
        minute_key = minute_key.with_second(0).unwrap_or(minute_key);
        minute_key = minute_key.with_nanosecond(0).unwrap_or(minute_key);
        minute_key
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_log_entry_creation() {
        let mut fields = HashMap::new();
        fields.insert("MESSAGE".to_string(), "Test message".to_string());
        fields.insert("PRIORITY".to_string(), "6".to_string());

        let timestamp = Utc.with_ymd_and_hms(2026, 1, 17, 14, 30, 45).unwrap();
        let entry = LogEntry::new(timestamp, fields.clone());

        assert_eq!(entry.timestamp, timestamp);
        assert_eq!(entry.fields, fields);
    }

    #[test]
    fn test_get_field_methods() {
        let mut fields = HashMap::new();
        fields.insert("MESSAGE".to_string(), "Test message".to_string());
        fields.insert("PRIORITY".to_string(), "6".to_string());
        fields.insert("_SYSTEMD_UNIT".to_string(), "test.service".to_string());
        fields.insert("_HOSTNAME".to_string(), "test-host".to_string());
        fields.insert("_PID".to_string(), "1234".to_string());
        fields.insert("_EXE".to_string(), "/usr/bin/test".to_string());

        let timestamp = Utc.with_ymd_and_hms(2026, 1, 17, 14, 30, 45).unwrap();
        let entry = LogEntry::new(timestamp, fields);

        assert_eq!(entry.get_message(), Some(&"Test message".to_string()));
        assert_eq!(entry.get_priority(), Some(&"6".to_string()));
        assert_eq!(entry.get_systemd_unit(), Some(&"test.service".to_string()));
        assert_eq!(entry.get_hostname(), Some(&"test-host".to_string()));
        assert_eq!(entry.get_pid(), Some(&"1234".to_string()));
        assert_eq!(entry.get_exe(), Some(&"/usr/bin/test".to_string()));
    }

    #[test]
    fn test_minute_key() {
        let timestamp = Utc.with_ymd_and_hms(2026, 1, 17, 14, 30, 45).unwrap();
        let entry = LogEntry::new(timestamp, HashMap::new());

        let minute_key = entry.minute_key();
        let expected = Utc.with_ymd_and_hms(2026, 1, 17, 14, 30, 0).unwrap();

        assert_eq!(minute_key, expected);
    }
}
