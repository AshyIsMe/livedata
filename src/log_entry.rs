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

    // User journal field getters
    pub fn get_message_id(&self) -> Option<&String> {
        self.get_field("MESSAGE_ID")
    }

    pub fn get_code_file(&self) -> Option<&String> {
        self.get_field("CODE_FILE")
    }

    pub fn get_code_line(&self) -> Option<&String> {
        self.get_field("CODE_LINE")
    }

    pub fn get_code_func(&self) -> Option<&String> {
        self.get_field("CODE_FUNC")
    }

    pub fn get_errno(&self) -> Option<&String> {
        self.get_field("ERRNO")
    }

    pub fn get_invocation_id(&self) -> Option<&String> {
        self.get_field("INVOCATION_ID")
    }

    pub fn get_user_invocation_id(&self) -> Option<&String> {
        self.get_field("USER_INVOCATION_ID")
    }

    pub fn get_syslog_facility(&self) -> Option<&String> {
        self.get_field("SYSLOG_FACILITY")
    }

    pub fn get_syslog_identifier(&self) -> Option<&String> {
        self.get_field("SYSLOG_IDENTIFIER")
    }

    pub fn get_syslog_pid(&self) -> Option<&String> {
        self.get_field("SYSLOG_PID")
    }

    pub fn get_syslog_timestamp(&self) -> Option<&String> {
        self.get_field("SYSLOG_TIMESTAMP")
    }

    pub fn get_syslog_raw(&self) -> Option<&String> {
        self.get_field("SYSLOG_RAW")
    }

    pub fn get_documentation(&self) -> Option<&String> {
        self.get_field("DOCUMENTATION")
    }

    pub fn get_tid(&self) -> Option<&String> {
        self.get_field("TID")
    }

    pub fn get_unit(&self) -> Option<&String> {
        self.get_field("UNIT")
    }

    pub fn get_user_unit(&self) -> Option<&String> {
        self.get_field("USER_UNIT")
    }

    // Trusted journal field getters
    pub fn get_uid(&self) -> Option<&String> {
        self.get_field("_UID")
    }

    pub fn get_gid(&self) -> Option<&String> {
        self.get_field("_GID")
    }

    pub fn get_comm(&self) -> Option<&String> {
        self.get_field("_COMM")
    }

    pub fn get_cmdline(&self) -> Option<&String> {
        self.get_field("_CMDLINE")
    }

    pub fn get_cap_effective(&self) -> Option<&String> {
        self.get_field("_CAP_EFFECTIVE")
    }

    pub fn get_audit_session(&self) -> Option<&String> {
        self.get_field("_AUDIT_SESSION")
    }

    pub fn get_audit_loginuid(&self) -> Option<&String> {
        self.get_field("_AUDIT_LOGINUID")
    }

    pub fn get_systemd_cgroup(&self) -> Option<&String> {
        self.get_field("_SYSTEMD_CGROUP")
    }

    pub fn get_systemd_slice(&self) -> Option<&String> {
        self.get_field("_SYSTEMD_SLICE")
    }

    pub fn get_systemd_user_unit(&self) -> Option<&String> {
        self.get_field("_SYSTEMD_USER_UNIT")
    }

    pub fn get_systemd_user_slice(&self) -> Option<&String> {
        self.get_field("_SYSTEMD_USER_SLICE")
    }

    pub fn get_systemd_session(&self) -> Option<&String> {
        self.get_field("_SYSTEMD_SESSION")
    }

    pub fn get_systemd_owner_uid(&self) -> Option<&String> {
        self.get_field("_SYSTEMD_OWNER_UID")
    }

    pub fn get_selinux_context(&self) -> Option<&String> {
        self.get_field("_SELINUX_CONTEXT")
    }

    pub fn get_source_realtime_timestamp(&self) -> Option<&String> {
        self.get_field("_SOURCE_REALTIME_TIMESTAMP")
    }

    pub fn get_source_boottime_timestamp(&self) -> Option<&String> {
        self.get_field("_SOURCE_BOOTTIME_TIMESTAMP")
    }

    pub fn get_boot_id(&self) -> Option<&String> {
        self.get_field("_BOOT_ID")
    }

    pub fn get_machine_id(&self) -> Option<&String> {
        self.get_field("_MACHINE_ID")
    }

    pub fn get_systemd_invocation_id(&self) -> Option<&String> {
        self.get_field("_SYSTEMD_INVOCATION_ID")
    }

    pub fn get_transport(&self) -> Option<&String> {
        self.get_field("_TRANSPORT")
    }

    pub fn get_stream_id(&self) -> Option<&String> {
        self.get_field("_STREAM_ID")
    }

    pub fn get_line_break(&self) -> Option<&String> {
        self.get_field("_LINE_BREAK")
    }

    pub fn get_namespace(&self) -> Option<&String> {
        self.get_field("_NAMESPACE")
    }

    pub fn get_runtime_scope(&self) -> Option<&String> {
        self.get_field("_RUNTIME_SCOPE")
    }

    // Kernel journal field getters
    pub fn get_kernel_device(&self) -> Option<&String> {
        self.get_field("_KERNEL_DEVICE")
    }

    pub fn get_kernel_subsystem(&self) -> Option<&String> {
        self.get_field("_KERNEL_SUBSYSTEM")
    }

    pub fn get_udev_sysname(&self) -> Option<&String> {
        self.get_field("_UDEV_SYSNAME")
    }

    pub fn get_udev_devnode(&self) -> Option<&String> {
        self.get_field("_UDEV_DEVNODE")
    }

    pub fn get_udev_devlink(&self) -> Option<&String> {
        self.get_field("_UDEV_DEVLINK")
    }

    // Object field getters
    pub fn get_coredump_unit(&self) -> Option<&String> {
        self.get_field("COREDUMP_UNIT")
    }

    pub fn get_coredump_user_unit(&self) -> Option<&String> {
        self.get_field("COREDUMP_USER_UNIT")
    }

    pub fn get_object_pid(&self) -> Option<&String> {
        self.get_field("OBJECT_PID")
    }

    pub fn get_object_uid(&self) -> Option<&String> {
        self.get_field("OBJECT_UID")
    }

    pub fn get_object_gid(&self) -> Option<&String> {
        self.get_field("OBJECT_GID")
    }

    pub fn get_object_comm(&self) -> Option<&String> {
        self.get_field("OBJECT_COMM")
    }

    pub fn get_object_exe(&self) -> Option<&String> {
        self.get_field("OBJECT_EXE")
    }

    pub fn get_object_cmdline(&self) -> Option<&String> {
        self.get_field("OBJECT_CMDLINE")
    }

    pub fn get_object_audit_session(&self) -> Option<&String> {
        self.get_field("OBJECT_AUDIT_SESSION")
    }

    pub fn get_object_audit_loginuid(&self) -> Option<&String> {
        self.get_field("OBJECT_AUDIT_LOGINUID")
    }

    pub fn get_object_systemd_cgroup(&self) -> Option<&String> {
        self.get_field("OBJECT_SYSTEMD_CGROUP")
    }

    pub fn get_object_systemd_session(&self) -> Option<&String> {
        self.get_field("OBJECT_SYSTEMD_SESSION")
    }

    pub fn get_object_systemd_owner_uid(&self) -> Option<&String> {
        self.get_field("OBJECT_SYSTEMD_OWNER_UID")
    }

    pub fn get_object_systemd_unit(&self) -> Option<&String> {
        self.get_field("OBJECT_SYSTEMD_UNIT")
    }

    pub fn get_object_systemd_user_unit(&self) -> Option<&String> {
        self.get_field("OBJECT_SYSTEMD_USER_UNIT")
    }

    pub fn get_object_systemd_invocation_id(&self) -> Option<&String> {
        self.get_field("OBJECT_SYSTEMD_INVOCATION_ID")
    }

    // Address field getters
    pub fn get_cursor(&self) -> Option<&String> {
        self.get_field("__CURSOR")
    }

    pub fn get_realtime_timestamp(&self) -> Option<&String> {
        self.get_field("__REALTIME_TIMESTAMP")
    }

    pub fn get_monotonic_timestamp(&self) -> Option<&String> {
        self.get_field("__MONOTONIC_TIMESTAMP")
    }

    pub fn get_seqnum(&self) -> Option<&String> {
        self.get_field("__SEQNUM")
    }

    pub fn get_seqnum_id(&self) -> Option<&String> {
        self.get_field("__SEQNUM_ID")
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
