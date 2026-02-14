use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use sysinfo::{ProcessesToUpdate, System};
use tokio::sync::mpsc;
use tokio::time::Duration;

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
    metrics_tx: Arc<Mutex<Option<mpsc::Sender<ProcessMetricsBatch>>>>,
    shutdown_signal: Arc<AtomicBool>,
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
            metrics_tx: Arc::new(Mutex::new(None)),
            shutdown_signal: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Initialize with a metrics sender for persistence
    pub fn with_metrics_sender(
        metrics_tx: mpsc::Sender<ProcessMetricsBatch>,
        shutdown_signal: Arc<AtomicBool>,
    ) -> Self {
        let mut system = System::new_all();
        system.refresh_all();

        Self {
            system: Arc::new(Mutex::new(system)),
            snapshot: Arc::new(Mutex::new(Vec::new())),
            metrics_tx: Arc::new(Mutex::new(Some(metrics_tx))),
            shutdown_signal,
        }
    }

    /// Start background collection task (run once at startup)
    /// Spawns a dedicated thread with its own tokio runtime for the collection loop
    pub fn start_collection(&self, interval_secs: u64) -> std::thread::JoinHandle<()> {
        let system = self.system.clone();
        let snapshot = self.snapshot.clone();
        let metrics_tx = self.metrics_tx.clone();
        let shutdown_signal = self.shutdown_signal.clone();

        std::thread::spawn(move || {
            // Create a local tokio runtime for this thread
            let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

            rt.block_on(async move {
                loop {
                    if shutdown_signal.load(Ordering::Relaxed) {
                        log::info!("Process monitor shutting down");
                        break;
                    }

                    // Sleep in short slices so shutdown can interrupt promptly.
                    let mut remaining_ms = interval_secs.saturating_mul(1000);
                    while remaining_ms > 0 {
                        if shutdown_signal.load(Ordering::Relaxed) {
                            break;
                        }
                        let step_ms = remaining_ms.min(100);
                        tokio::time::sleep(Duration::from_millis(step_ms)).await;
                        remaining_ms = remaining_ms.saturating_sub(step_ms);
                    }

                    if shutdown_signal.load(Ordering::Relaxed) {
                        log::info!("Process monitor shutting down");
                        break;
                    }

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
                    let tx = metrics_tx.lock().unwrap().as_ref().cloned();
                    if let Some(tx) = tx {
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
        })
    }

    /// Close the metrics sender so the receiver can exit cleanly.
    pub fn shutdown_metrics_channel(&self) {
        *self.metrics_tx.lock().unwrap() = None;
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
