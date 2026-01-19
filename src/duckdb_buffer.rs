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

        // Create the main table for journal logs with all systemd journal fields
        conn.execute(
            "CREATE TABLE journal_logs (
                timestamp TIMESTAMP NOT NULL,
                minute_key VARCHAR NOT NULL,
                -- User journal fields
                message TEXT,
                message_id TEXT,
                priority INTEGER,
                code_file TEXT,
                code_line INTEGER,
                code_func TEXT,
                errno INTEGER,
                invocation_id TEXT,
                user_invocation_id TEXT,
                syslog_facility INTEGER,
                syslog_identifier TEXT,
                syslog_pid INTEGER,
                syslog_timestamp TEXT,
                syslog_raw TEXT,
                documentation TEXT,
                tid INTEGER,
                unit TEXT,
                user_unit TEXT,
                -- Trusted journal fields
                _PID INTEGER,
                _UID INTEGER,
                _GID INTEGER,
                _COMM TEXT,
                _EXE TEXT,
                _CMDLINE TEXT,
                _CAP_EFFECTIVE TEXT,
                _AUDIT_SESSION INTEGER,
                _AUDIT_LOGINUID INTEGER,
                _SYSTEMD_CGROUP TEXT,
                _SYSTEMD_SLICE TEXT,
                _SYSTEMD_UNIT TEXT,
                _SYSTEMD_USER_UNIT TEXT,
                _SYSTEMD_USER_SLICE TEXT,
                _SYSTEMD_SESSION TEXT,
                _SYSTEMD_OWNER_UID INTEGER,
                _SELINUX_CONTEXT TEXT,
                _SOURCE_REALTIME_TIMESTAMP BIGINT,
                _SOURCE_BOOTTIME_TIMESTAMP BIGINT,
                _BOOT_ID TEXT,
                _MACHINE_ID TEXT,
                _SYSTEMD_INVOCATION_ID TEXT,
                _HOSTNAME TEXT,
                _TRANSPORT TEXT,
                _STREAM_ID TEXT,
                _LINE_BREAK TEXT,
                _NAMESPACE TEXT,
                _RUNTIME_SCOPE TEXT,
                -- Kernel journal fields
                _KERNEL_DEVICE TEXT,
                _KERNEL_SUBSYSTEM TEXT,
                _UDEV_SYSNAME TEXT,
                _UDEV_DEVNODE TEXT,
                _UDEV_DEVLINK TEXT,
                -- Fields to log on behalf of another program
                COREDUMP_UNIT TEXT,
                COREDUMP_USER_UNIT TEXT,
                OBJECT_PID INTEGER,
                OBJECT_UID INTEGER,
                OBJECT_GID INTEGER,
                OBJECT_COMM TEXT,
                OBJECT_EXE TEXT,
                OBJECT_CMDLINE TEXT,
                OBJECT_AUDIT_SESSION INTEGER,
                OBJECT_AUDIT_LOGINUID INTEGER,
                OBJECT_SYSTEMD_CGROUP TEXT,
                OBJECT_SYSTEMD_SESSION TEXT,
                OBJECT_SYSTEMD_OWNER_UID INTEGER,
                OBJECT_SYSTEMD_UNIT TEXT,
                OBJECT_SYSTEMD_USER_UNIT TEXT,
                OBJECT_SYSTEMD_USER_SLICE TEXT,
                OBJECT_SYSTEMD_INVOCATION_ID TEXT,
                -- Address fields (for serialization metadata)
                __CURSOR TEXT,
                __REALTIME_TIMESTAMP BIGINT,
                __MONOTONIC_TIMESTAMP BIGINT,
                __SEQNUM BIGINT,
                __SEQNUM_ID BIGINT,
                -- Fallback for any custom fields not in systemd spec
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
        conn.execute("CREATE INDEX idx_hostname ON journal_logs(_HOSTNAME)", [])?;
        conn.execute(
            "CREATE INDEX idx_systemd_unit ON journal_logs(_SYSTEMD_UNIT)",
            [],
        )?;

        info!("DuckDB buffer initialized successfully");

        Ok(Self { conn })
    }

    pub fn add_entry(&mut self, entry: &LogEntry) -> Result<()> {
        let minute_key = entry.minute_key();

        // Extract all systemd journal fields with proper type conversions

        // User journal fields
        let message = entry.get_field("MESSAGE").cloned();
        let message_id = entry.get_field("MESSAGE_ID").cloned();
        let priority = entry
            .get_field("PRIORITY")
            .and_then(|p| p.parse::<i32>().ok());
        let code_file = entry.get_field("CODE_FILE").cloned();
        let code_line = entry
            .get_field("CODE_LINE")
            .and_then(|p| p.parse::<i32>().ok());
        let code_func = entry.get_field("CODE_FUNC").cloned();
        let errno = entry.get_field("ERRNO").and_then(|p| p.parse::<i32>().ok());
        let invocation_id = entry.get_field("INVOCATION_ID").cloned();
        let user_invocation_id = entry.get_field("USER_INVOCATION_ID").cloned();
        let syslog_facility = entry
            .get_field("SYSLOG_FACILITY")
            .and_then(|p| p.parse::<i32>().ok());
        let syslog_identifier = entry.get_field("SYSLOG_IDENTIFIER").cloned();
        let syslog_pid = entry
            .get_field("SYSLOG_PID")
            .and_then(|p| p.parse::<i32>().ok());
        let syslog_timestamp = entry.get_field("SYSLOG_TIMESTAMP").cloned();
        let syslog_raw = entry.get_field("SYSLOG_RAW").cloned();
        let documentation = entry.get_field("DOCUMENTATION").cloned();
        let tid = entry.get_field("TID").and_then(|p| p.parse::<i32>().ok());
        let unit = entry.get_field("UNIT").cloned();
        let user_unit = entry.get_field("USER_UNIT").cloned();

        // Trusted journal fields
        let _pid = entry.get_field("_PID").and_then(|p| p.parse::<i32>().ok());
        let _uid = entry.get_field("_UID").and_then(|p| p.parse::<i32>().ok());
        let _gid = entry.get_field("_GID").and_then(|p| p.parse::<i32>().ok());
        let _comm = entry.get_field("_COMM").cloned();
        let _exe = entry.get_field("_EXE").cloned();
        let _cmdline = entry.get_field("_CMDLINE").cloned();
        let _cap_effective = entry.get_field("_CAP_EFFECTIVE").cloned();
        let _audit_session = entry
            .get_field("_AUDIT_SESSION")
            .and_then(|p| p.parse::<i32>().ok());
        let _audit_loginuid = entry
            .get_field("_AUDIT_LOGINUID")
            .and_then(|p| p.parse::<i32>().ok());
        let _systemd_cgroup = entry.get_field("_SYSTEMD_CGROUP").cloned();
        let _systemd_slice = entry.get_field("_SYSTEMD_SLICE").cloned();
        let _systemd_unit = entry.get_field("_SYSTEMD_UNIT").cloned();
        let _systemd_user_unit = entry.get_field("_SYSTEMD_USER_UNIT").cloned();
        let _systemd_user_slice = entry.get_field("_SYSTEMD_USER_SLICE").cloned();
        let _systemd_session = entry.get_field("_SYSTEMD_SESSION").cloned();
        let _systemd_owner_uid = entry
            .get_field("_SYSTEMD_OWNER_UID")
            .and_then(|p| p.parse::<i32>().ok());
        let _selinux_context = entry.get_field("_SELINUX_CONTEXT").cloned();
        let _source_realtime_timestamp = entry
            .get_field("_SOURCE_REALTIME_TIMESTAMP")
            .and_then(|p| p.parse::<i64>().ok());
        let _source_boottime_timestamp = entry
            .get_field("_SOURCE_BOOTTIME_TIMESTAMP")
            .and_then(|p| p.parse::<i64>().ok());
        let _boot_id = entry.get_field("_BOOT_ID").cloned();
        let _machine_id = entry.get_field("_MACHINE_ID").cloned();
        let _systemd_invocation_id = entry.get_field("_SYSTEMD_INVOCATION_ID").cloned();
        let _hostname = entry.get_field("_HOSTNAME").cloned();
        let _transport = entry.get_field("_TRANSPORT").cloned();
        let _stream_id = entry.get_field("_STREAM_ID").cloned();
        let _line_break = entry.get_field("_LINE_BREAK").cloned();
        let _namespace = entry.get_field("_NAMESPACE").cloned();
        let _runtime_scope = entry.get_field("_RUNTIME_SCOPE").cloned();

        // Kernel journal fields
        let _kernel_device = entry.get_field("_KERNEL_DEVICE").cloned();
        let _kernel_subsystem = entry.get_field("_KERNEL_SUBSYSTEM").cloned();
        let _udev_sysname = entry.get_field("_UDEV_SYSNAME").cloned();
        let _udev_devnode = entry.get_field("_UDEV_DEVNODE").cloned();
        let _udev_devlink = entry.get_field("_UDEV_DEVLINK").cloned();

        // Fields to log on behalf of another program
        let coredump_unit = entry.get_field("COREDUMP_UNIT").cloned();
        let coredump_user_unit = entry.get_field("COREDUMP_USER_UNIT").cloned();
        let object_pid = entry
            .get_field("OBJECT_PID")
            .and_then(|p| p.parse::<i32>().ok());
        let object_uid = entry
            .get_field("OBJECT_UID")
            .and_then(|p| p.parse::<i32>().ok());
        let object_gid = entry
            .get_field("OBJECT_GID")
            .and_then(|p| p.parse::<i32>().ok());
        let object_comm = entry.get_field("OBJECT_COMM").cloned();
        let object_exe = entry.get_field("OBJECT_EXE").cloned();
        let object_cmdline = entry.get_field("OBJECT_CMDLINE").cloned();
        let object_audit_session = entry
            .get_field("OBJECT_AUDIT_SESSION")
            .and_then(|p| p.parse::<i32>().ok());
        let object_audit_loginuid = entry
            .get_field("OBJECT_AUDIT_LOGINUID")
            .and_then(|p| p.parse::<i32>().ok());
        let object_systemd_cgroup = entry.get_field("OBJECT_SYSTEMD_CGROUP").cloned();
        let object_systemd_session = entry.get_field("OBJECT_SYSTEMD_SESSION").cloned();
        let object_systemd_owner_uid = entry
            .get_field("OBJECT_SYSTEMD_OWNER_UID")
            .and_then(|p| p.parse::<i32>().ok());
        let object_systemd_unit = entry.get_field("OBJECT_SYSTEMD_UNIT").cloned();
        let object_systemd_user_unit = entry.get_field("OBJECT_SYSTEMD_USER_UNIT").cloned();
        let object_systemd_user_slice = entry.get_field("OBJECT_SYSTEMD_USER_SLICE").cloned();
        let object_systemd_invocation_id = entry.get_field("OBJECT_SYSTEMD_INVOCATION_ID").cloned();

        // Address fields
        let __cursor = entry.get_field("__CURSOR").cloned();
        let __realtime_timestamp = entry
            .get_field("__REALTIME_TIMESTAMP")
            .and_then(|p| p.parse::<i64>().ok());
        let __monotonic_timestamp = entry
            .get_field("__MONOTONIC_TIMESTAMP")
            .and_then(|p| p.parse::<i64>().ok());
        let __seqnum = entry
            .get_field("__SEQNUM")
            .and_then(|p| p.parse::<i64>().ok());
        let __seqnum_id = entry
            .get_field("__SEQNUM_ID")
            .and_then(|p| p.parse::<i64>().ok());

        // Create extra_fields JSON with any fields not in the systemd spec
        let systemd_fields = std::collections::HashSet::from([
            // User fields
            "MESSAGE",
            "MESSAGE_ID",
            "PRIORITY",
            "CODE_FILE",
            "CODE_LINE",
            "CODE_FUNC",
            "ERRNO",
            "INVOCATION_ID",
            "USER_INVOCATION_ID",
            "SYSLOG_FACILITY",
            "SYSLOG_IDENTIFIER",
            "SYSLOG_PID",
            "SYSLOG_TIMESTAMP",
            "SYSLOG_RAW",
            "DOCUMENTATION",
            "TID",
            "UNIT",
            "USER_UNIT",
            // Trusted fields
            "_PID",
            "_UID",
            "_GID",
            "_COMM",
            "_EXE",
            "_CMDLINE",
            "_CAP_EFFECTIVE",
            "_AUDIT_SESSION",
            "_AUDIT_LOGINUID",
            "_SYSTEMD_CGROUP",
            "_SYSTEMD_SLICE",
            "_SYSTEMD_UNIT",
            "_SYSTEMD_USER_UNIT",
            "_SYSTEMD_USER_SLICE",
            "_SYSTEMD_SESSION",
            "_SYSTEMD_OWNER_UID",
            "_SELINUX_CONTEXT",
            "_SOURCE_REALTIME_TIMESTAMP",
            "_SOURCE_BOOTTIME_TIMESTAMP",
            "_BOOT_ID",
            "_MACHINE_ID",
            "_SYSTEMD_INVOCATION_ID",
            "_HOSTNAME",
            "_TRANSPORT",
            "_STREAM_ID",
            "_LINE_BREAK",
            "_NAMESPACE",
            "_RUNTIME_SCOPE",
            // Kernel fields
            "_KERNEL_DEVICE",
            "_KERNEL_SUBSYSTEM",
            "_UDEV_SYSNAME",
            "_UDEV_DEVNODE",
            "_UDEV_DEVLINK",
            // Object fields
            "COREDUMP_UNIT",
            "COREDUMP_USER_UNIT",
            "OBJECT_PID",
            "OBJECT_UID",
            "OBJECT_GID",
            "OBJECT_COMM",
            "OBJECT_EXE",
            "OBJECT_CMDLINE",
            "OBJECT_AUDIT_SESSION",
            "OBJECT_AUDIT_LOGINUID",
            "OBJECT_SYSTEMD_CGROUP",
            "OBJECT_SYSTEMD_SESSION",
            "OBJECT_SYSTEMD_OWNER_UID",
            "OBJECT_SYSTEMD_UNIT",
            "OBJECT_SYSTEMD_USER_UNIT",
            "OBJECT_SYSTEMD_USER_SLICE",
            "OBJECT_SYSTEMD_INVOCATION_ID",
            // Address fields
            "__CURSOR",
            "__REALTIME_TIMESTAMP",
            "__MONOTONIC_TIMESTAMP",
            "__SEQNUM",
            "__SEQNUM_ID",
        ]);

        let extra_fields: std::collections::HashMap<String, String> = entry
            .fields
            .iter()
            .filter(|(k, _)| !systemd_fields.contains(k.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        let extra_fields_json = if extra_fields.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&extra_fields)?)
        };

        self.conn.execute(
            "INSERT INTO journal_logs (
                timestamp, minute_key,
                -- User journal fields
                message, message_id, priority, code_file, code_line, code_func, errno,
                invocation_id, user_invocation_id, syslog_facility, syslog_identifier,
                syslog_pid, syslog_timestamp, syslog_raw, documentation, tid, unit, user_unit,
                -- Trusted journal fields
                _PID, _UID, _GID, _COMM, _EXE, _CMDLINE, _CAP_EFFECTIVE, _AUDIT_SESSION,
                _AUDIT_LOGINUID, _SYSTEMD_CGROUP, _SYSTEMD_SLICE, _SYSTEMD_UNIT,
                _SYSTEMD_USER_UNIT, _SYSTEMD_USER_SLICE, _SYSTEMD_SESSION, _SYSTEMD_OWNER_UID,
                _SELINUX_CONTEXT, _SOURCE_REALTIME_TIMESTAMP, _SOURCE_BOOTTIME_TIMESTAMP,
                _BOOT_ID, _MACHINE_ID, _SYSTEMD_INVOCATION_ID, _HOSTNAME, _TRANSPORT,
                _STREAM_ID, _LINE_BREAK, _NAMESPACE, _RUNTIME_SCOPE,
                -- Kernel journal fields
                _KERNEL_DEVICE, _KERNEL_SUBSYSTEM, _UDEV_SYSNAME, _UDEV_DEVNODE, _UDEV_DEVLINK,
                 -- Fields to log on behalf of another program
                 COREDUMP_UNIT, COREDUMP_USER_UNIT, OBJECT_PID, OBJECT_UID, OBJECT_GID,
                 OBJECT_COMM, OBJECT_EXE, OBJECT_CMDLINE, OBJECT_AUDIT_SESSION,
                 OBJECT_AUDIT_LOGINUID, OBJECT_SYSTEMD_CGROUP, OBJECT_SYSTEMD_SESSION,
                 OBJECT_SYSTEMD_OWNER_UID, OBJECT_SYSTEMD_UNIT, OBJECT_SYSTEMD_USER_UNIT,
                 OBJECT_SYSTEMD_USER_SLICE, OBJECT_SYSTEMD_INVOCATION_ID,
                    -- Address fields
                    __CURSOR, __REALTIME_TIMESTAMP, __MONOTONIC_TIMESTAMP, __SEQNUM, __SEQNUM_ID,
                -- Extra fields
                extra_fields
            ) VALUES (
                ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
                ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
                ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
                ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
                ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
                ?, ?, ?, ?, ?, ?
            )",
            params![
                entry.timestamp.to_rfc3339(),
                minute_key.to_rfc3339(),
                // User journal fields
                message,
                message_id,
                priority,
                code_file,
                code_line,
                code_func,
                errno,
                invocation_id,
                user_invocation_id,
                syslog_facility,
                syslog_identifier,
                syslog_pid,
                syslog_timestamp,
                syslog_raw,
                documentation,
                tid,
                unit,
                user_unit,
                // Trusted journal fields
                _pid,
                _uid,
                _gid,
                _comm,
                _exe,
                _cmdline,
                _cap_effective,
                _audit_session,
                _audit_loginuid,
                _systemd_cgroup,
                _systemd_slice,
                _systemd_unit,
                _systemd_user_unit,
                _systemd_user_slice,
                _systemd_session,
                _systemd_owner_uid,
                _selinux_context,
                _source_realtime_timestamp,
                _source_boottime_timestamp,
                _boot_id,
                _machine_id,
                _systemd_invocation_id,
                _hostname,
                _transport,
                _stream_id,
                _line_break,
                _namespace,
                _runtime_scope,
                // Kernel journal fields
                _kernel_device,
                _kernel_subsystem,
                _udev_sysname,
                _udev_devnode,
                _udev_devlink,
                // Fields to log on behalf of another program
                coredump_unit,
                coredump_user_unit,
                object_pid,
                object_uid,
                object_gid,
                object_comm,
                object_exe,
                object_cmdline,
                object_audit_session,
                object_audit_loginuid,
                object_systemd_cgroup,
                object_systemd_session,
                object_systemd_owner_uid,
                object_systemd_unit,
                object_systemd_user_unit,
                object_systemd_user_slice,
                object_systemd_invocation_id,
                // Address fields
                __cursor,
                __realtime_timestamp,
                __monotonic_timestamp,
                __seqnum,
                __seqnum_id,
                // Extra fields
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
            "SELECT CAST(timestamp AS VARCHAR),
                    -- User journal fields
                    message, message_id, priority, code_file, code_line, code_func, errno,
                    invocation_id, user_invocation_id, syslog_facility, syslog_identifier,
                    syslog_pid, syslog_timestamp, syslog_raw, documentation, tid, unit, user_unit,
                    -- Trusted journal fields
                    _PID, _UID, _GID, _COMM, _EXE, _CMDLINE, _CAP_EFFECTIVE, _AUDIT_SESSION,
                    _AUDIT_LOGINUID, _SYSTEMD_CGROUP, _SYSTEMD_SLICE, _SYSTEMD_UNIT,
                    _SYSTEMD_USER_UNIT, _SYSTEMD_USER_SLICE, _SYSTEMD_SESSION, _SYSTEMD_OWNER_UID,
                    _SELINUX_CONTEXT, _SOURCE_REALTIME_TIMESTAMP, _SOURCE_BOOTTIME_TIMESTAMP,
                    _BOOT_ID, _MACHINE_ID, _SYSTEMD_INVOCATION_ID, _HOSTNAME, _TRANSPORT,
                    _STREAM_ID, _LINE_BREAK, _NAMESPACE, _RUNTIME_SCOPE,
                    -- Kernel journal fields
                    _KERNEL_DEVICE, _KERNEL_SUBSYSTEM, _UDEV_SYSNAME, _UDEV_DEVNODE, _UDEV_DEVLINK,
                     -- Fields to log on behalf of another program
                     COREDUMP_UNIT, COREDUMP_USER_UNIT, OBJECT_PID, OBJECT_UID, OBJECT_GID,
                     OBJECT_COMM, OBJECT_EXE, OBJECT_CMDLINE, OBJECT_AUDIT_SESSION,
                     OBJECT_AUDIT_LOGINUID, OBJECT_SYSTEMD_CGROUP, OBJECT_SYSTEMD_SESSION,
                     OBJECT_SYSTEMD_OWNER_UID, OBJECT_SYSTEMD_UNIT, OBJECT_SYSTEMD_USER_UNIT,
                     OBJECT_SYSTEMD_USER_SLICE, OBJECT_SYSTEMD_INVOCATION_ID,
                    -- Address fields
                __CURSOR, __REALTIME_TIMESTAMP, __MONOTONIC_TIMESTAMP, __SEQNUM, __SEQNUM_ID,
                    -- Extra fields
                    extra_fields
              FROM journal_logs WHERE minute_key = ? ORDER BY timestamp",
        )?;
        let mut rows = stmt.query(params![minute_key.to_rfc3339()])?;

        while let Some(row) = rows.next()? {
            let timestamp_str: String = row.get(0)?;
            let naive_dt =
                chrono::NaiveDateTime::parse_from_str(&timestamp_str, "%Y-%m-%d %H:%M:%S")
                    .map_err(|e| {
                        anyhow::anyhow!("Failed to parse timestamp '{}': {}", timestamp_str, e)
                    })?;
            let timestamp: DateTime<Utc> = DateTime::from_naive_utc_and_offset(naive_dt, Utc);

            // Reconstruct the original fields structure
            let mut fields = serde_json::Map::new();

            // Helper macro to add optional fields
            macro_rules! add_field {
                ($field_name:expr, $row_idx:expr, String) => {
                    if let Some(value) = row.get::<_, Option<String>>($row_idx)? {
                        fields.insert($field_name.to_string(), serde_json::Value::String(value));
                    }
                };
                ($field_name:expr, $row_idx:expr, i32) => {
                    if let Some(value) = row.get::<_, Option<i32>>($row_idx)? {
                        fields.insert(
                            $field_name.to_string(),
                            serde_json::Value::String(value.to_string()),
                        );
                    }
                };
                ($field_name:expr, $row_idx:expr, i64) => {
                    if let Some(value) = row.get::<_, Option<i64>>($row_idx)? {
                        fields.insert(
                            $field_name.to_string(),
                            serde_json::Value::String(value.to_string()),
                        );
                    }
                };
            }

            // User journal fields
            add_field!("MESSAGE", 1, String);
            add_field!("MESSAGE_ID", 2, String);
            add_field!("PRIORITY", 3, i32);
            add_field!("CODE_FILE", 4, String);
            add_field!("CODE_LINE", 5, i32);
            add_field!("CODE_FUNC", 6, String);
            add_field!("ERRNO", 7, i32);
            add_field!("INVOCATION_ID", 8, String);
            add_field!("USER_INVOCATION_ID", 9, String);
            add_field!("SYSLOG_FACILITY", 10, i32);
            add_field!("SYSLOG_IDENTIFIER", 11, String);
            add_field!("SYSLOG_PID", 12, i32);
            add_field!("SYSLOG_TIMESTAMP", 13, String);
            add_field!("SYSLOG_RAW", 14, String);
            add_field!("DOCUMENTATION", 15, String);
            add_field!("TID", 16, i32);
            add_field!("UNIT", 17, String);
            add_field!("USER_UNIT", 18, String);

            // Trusted journal fields
            add_field!("_PID", 19, i32);
            add_field!("_UID", 20, i32);
            add_field!("_GID", 21, i32);
            add_field!("_COMM", 22, String);
            add_field!("_EXE", 23, String);
            add_field!("_CMDLINE", 24, String);
            add_field!("_CAP_EFFECTIVE", 25, String);
            add_field!("_AUDIT_SESSION", 26, i32);
            add_field!("_AUDIT_LOGINUID", 27, i32);
            add_field!("_SYSTEMD_CGROUP", 28, String);
            add_field!("_SYSTEMD_SLICE", 29, String);
            add_field!("_SYSTEMD_UNIT", 30, String);
            add_field!("_SYSTEMD_USER_UNIT", 31, String);
            add_field!("_SYSTEMD_USER_SLICE", 32, String);
            add_field!("_SYSTEMD_SESSION", 33, String);
            add_field!("_SYSTEMD_OWNER_UID", 34, i32);
            add_field!("_SELINUX_CONTEXT", 35, String);
            add_field!("_SOURCE_REALTIME_TIMESTAMP", 36, i64);
            add_field!("_SOURCE_BOOTTIME_TIMESTAMP", 37, i64);
            add_field!("_BOOT_ID", 38, String);
            add_field!("_MACHINE_ID", 39, String);
            add_field!("_SYSTEMD_INVOCATION_ID", 40, String);
            add_field!("_HOSTNAME", 41, String);
            add_field!("_TRANSPORT", 42, String);
            add_field!("_STREAM_ID", 43, String);
            add_field!("_LINE_BREAK", 44, String);
            add_field!("_NAMESPACE", 45, String);
            add_field!("_RUNTIME_SCOPE", 46, String);

            // Kernel journal fields
            add_field!("_KERNEL_DEVICE", 47, String);
            add_field!("_KERNEL_SUBSYSTEM", 48, String);
            add_field!("_UDEV_SYSNAME", 49, String);
            add_field!("_UDEV_DEVNODE", 50, String);
            add_field!("_UDEV_DEVLINK", 51, String);

            // Fields to log on behalf of another program
            add_field!("COREDUMP_UNIT", 52, String);
            add_field!("COREDUMP_USER_UNIT", 53, String);
            add_field!("OBJECT_PID", 54, i32);
            add_field!("OBJECT_UID", 55, i32);
            add_field!("OBJECT_GID", 56, i32);
            add_field!("OBJECT_COMM", 57, String);
            add_field!("OBJECT_EXE", 58, String);
            add_field!("OBJECT_CMDLINE", 59, String);
            add_field!("OBJECT_AUDIT_SESSION", 60, i32);
            add_field!("OBJECT_AUDIT_LOGINUID", 61, i32);
            add_field!("OBJECT_SYSTEMD_CGROUP", 62, String);
            add_field!("OBJECT_SYSTEMD_SESSION", 63, String);
            add_field!("OBJECT_SYSTEMD_OWNER_UID", 64, i32);
            add_field!("OBJECT_SYSTEMD_UNIT", 65, String);
            add_field!("OBJECT_SYSTEMD_USER_UNIT", 66, String);
            add_field!("OBJECT_SYSTEMD_USER_SLICE", 67, String);
            add_field!("OBJECT_SYSTEMD_INVOCATION_ID", 68, String);

            // Address fields
            add_field!("__CURSOR", 69, String);
            add_field!("__REALTIME_TIMESTAMP", 70, i64);
            add_field!("__MONOTONIC_TIMESTAMP", 71, i64);
            add_field!("__SEQNUM", 72, i64);
            add_field!("__SEQNUM_ID", 73, i64);

            // Add extra fields if they exist
            if let Some(extra_fields_json) = row.get::<_, Option<String>>(74)? {
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
            let minute_key: DateTime<Utc> = DateTime::parse_from_rfc3339(&minute_key_str)
                .map_err(|e| {
                    anyhow::anyhow!("Failed to parse minute_key '{}': {}", minute_key_str, e)
                })?
                .with_timezone(&Utc);
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
                    let dt = DateTime::parse_from_rfc3339(&s)
                        .map_err(|e| {
                            anyhow::anyhow!("Failed to parse newest minute_key '{}': {}", s, e)
                        })?
                        .with_timezone(&Utc);
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
                    let dt = DateTime::parse_from_rfc3339(&s)
                        .map_err(|e| {
                            anyhow::anyhow!("Failed to parse oldest minute_key '{}': {}", s, e)
                        })?
                        .with_timezone(&Utc);
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
        if let Err(e) = &result {
            eprintln!("DuckDB buffer creation error: {:?}", e);
        }
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
        if let Err(e) = &result {
            eprintln!("Add entry error: {:?}", e);
        }
        assert!(result.is_ok());

        let minute_key = entry.minute_key();
        let retrieved_result = buffer.get_entries_for_minute(minute_key);
        if let Err(e) = &retrieved_result {
            eprintln!("Get entries error: {:?}", e);
        }
        let retrieved = retrieved_result.unwrap();
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
