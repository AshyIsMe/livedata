use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use sysinfo::{ProcessesToUpdate, System};
use tokio::time::{interval, Duration};

/// Process information snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu_percent: f32,
    pub memory_bytes: u64,
    pub user_id: Option<String>,
    pub runtime_secs: u64,
}

/// Background process collection service
pub struct ProcessMonitor {
    system: Arc<Mutex<System>>,
    snapshot: Arc<Mutex<Vec<ProcessInfo>>>,
}

impl Default for ProcessMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessMonitor {
    /// Initialize a new process monitor with fresh system state
    pub fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_all();

        Self {
            system: Arc::new(Mutex::new(system)),
            snapshot: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Start background collection task (run once at startup)
    pub fn start_collection(&self, interval_secs: u64) {
        let system = self.system.clone();
        let snapshot = self.snapshot.clone();

        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(interval_secs));

            loop {
                ticker.tick().await;

                let mut sys = system.lock().unwrap();
                sys.refresh_processes(ProcessesToUpdate::All, true);

                // Collect snapshot
                let processes: Vec<ProcessInfo> = sys
                    .processes()
                    .iter()
                    .map(|(pid, process)| ProcessInfo {
                        pid: pid.as_u32(),
                        name: process.name().to_string_lossy().to_string(),
                        cpu_percent: process.cpu_usage(),
                        memory_bytes: process.memory(),
                        user_id: process.user_id().map(|u| format!("{:?}", u)),
                        runtime_secs: process.run_time(),
                    })
                    .collect();

                *snapshot.lock().unwrap() = processes;
            }
        });
    }

    /// Get current process snapshot (called by API handler)
    pub fn get_snapshot(&self) -> Vec<ProcessInfo> {
        self.snapshot.lock().unwrap().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_monitor_creation() {
        let monitor = ProcessMonitor::new();
        let snapshot = monitor.get_snapshot();
        // Initially empty before collection starts
        assert!(snapshot.is_empty());
    }
}
