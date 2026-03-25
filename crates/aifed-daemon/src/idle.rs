//! Idle timeout monitoring

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time::sleep;

/// Monitors idle time and signals when timeout is reached.
pub struct IdleMonitor {
    last_activity: Arc<AtomicU64>,
    timeout_secs: u64,
    shutdown_tx: broadcast::Sender<()>,
}

impl IdleMonitor {
    /// Create a new idle monitor with the specified timeout in seconds.
    pub fn new(timeout_secs: u64) -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let last_activity = Arc::new(AtomicU64::new(now));

        Self { last_activity, timeout_secs, shutdown_tx }
    }

    /// Record activity (resets idle timer).
    pub fn record_activity(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.last_activity.store(now, Ordering::Relaxed);
    }

    /// Subscribe to shutdown signals.
    pub fn subscribe_shutdown(&self) -> broadcast::Receiver<()> {
        self.shutdown_tx.subscribe()
    }

    /// Trigger daemon shutdown programmatically.
    pub fn trigger_shutdown(&self) {
        tracing::info!("Shutdown triggered via API");
        let _ = self.shutdown_tx.send(());
    }

    /// Start the background idle monitoring task.
    pub fn start_monitor(self: Arc<Self>) {
        tokio::spawn(async move {
            let check_interval = Duration::from_secs(60);

            loop {
                sleep(check_interval).await;

                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                let last = self.last_activity.load(Ordering::Relaxed);
                let idle_secs = now.saturating_sub(last);

                if idle_secs >= self.timeout_secs {
                    tracing::info!(
                        "Idle timeout reached ({}s >= {}s), shutting down",
                        idle_secs,
                        self.timeout_secs
                    );
                    let _ = self.shutdown_tx.send(());
                    break;
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_activity() {
        let monitor = IdleMonitor::new(1800);
        let before = monitor.last_activity.load(Ordering::Relaxed);
        monitor.record_activity();
        let after = monitor.last_activity.load(Ordering::Relaxed);
        assert!(after >= before);
    }
}
