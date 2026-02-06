use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use sysinfo::{ProcessesToUpdate, System};
use tokio::sync::mpsc;
use tokio::time::{Duration, interval};

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

/// Batch of process metrics with timestamp
#[derive(Debug, Clone)]
pub struct ProcessMetricsBatch {
    pub processes: Vec<ProcessInfo>,
    pub timestamp: DateTime<Utc>,
}

/// Background process collection service
pub struct ProcessMonitor {
    system: Arc<Mutex<System>>,
    snapshot: Arc<Mutex<Vec<ProcessInfo>>>,
    metrics_tx: Option<mpsc::Sender<ProcessMetricsBatch>>,
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
            metrics_tx: None,
        }
    }

    /// Initialize with a metrics sender for persistence
    pub fn with_metrics_sender(metrics_tx: mpsc::Sender<ProcessMetricsBatch>) -> Self {
        let mut system = System::new_all();
        system.refresh_all();

        Self {
            system: Arc::new(Mutex::new(system)),
            snapshot: Arc::new(Mutex::new(Vec::new())),
            metrics_tx: Some(metrics_tx),
        }
    }

    /// Start background collection task (run once at startup)
    /// Spawns a dedicated thread with its own tokio runtime for the collection loop
    pub fn start_collection(&self, interval_secs: u64) {
        let system = self.system.clone();
        let snapshot = self.snapshot.clone();
        let metrics_tx = self.metrics_tx.clone();

        std::thread::spawn(move || {
            // Create a local tokio runtime for this thread
            let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

            rt.block_on(async move {
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

                    *snapshot.lock().unwrap() = processes.clone();

                    // Send batch to persistence channel if available
                    if let Some(ref tx) = metrics_tx {
                        let batch = ProcessMetricsBatch {
                            processes: processes.clone(),
                            timestamp: Utc::now(),
                        };

                        log::debug!(
                            "Sending batch with {} processes to persistence",
                            batch.processes.len()
                        );

                        // Try to send without blocking - log warning if channel is full
                        if let Err(e) = tx.try_send(batch) {
                            log::warn!("Failed to send process metrics batch: {}", e);
                        } else {
                            log::debug!("Successfully sent process metrics batch");
                        }
                    }
                }
            });
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
