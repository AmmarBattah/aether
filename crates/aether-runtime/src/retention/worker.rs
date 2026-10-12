use std::{sync::Arc, time::Duration};

use crate::Runtime;

const INTERVAL: Duration = Duration::from_secs(60 * 60);

pub(crate) fn spawn(runtime: Arc<Runtime>) {
    tokio::spawn(run(runtime, INTERVAL));
}

pub(super) async fn run(runtime: Arc<Runtime>, interval: Duration) {
    loop {
        tokio::select! {
            biased;
            _ = runtime.shutdown.cancelled() => return,
            _ = tokio::time::sleep(interval) => {}
        }
        let sweeping = runtime.clone();
        // Await this worker before starting another interval. Slow sweeps
        // never overlap or accumulate a backlog of blocking jobs.
        let result = tokio::task::spawn_blocking(move || {
            if sweeping.shutdown.is_cancelled() {
                return;
            }
            match sweeping.sweep_artifacts() {
                Ok(0) => {}
                Ok(evicted) => tracing::info!(evicted, "retention dropped artifact bytes"),
                Err(error) => tracing::error!(%error, "artifact retention sweep failed"),
            }
            if sweeping.shutdown.is_cancelled() {
                return;
            }
            match sweeping.sweep_run_history_inner(aether_core::time::now(), true) {
                Ok(report) => tracing::info!(?report, "completed-run retention sweep finished"),
                Err(error) => tracing::error!(%error, "completed-run retention sweep failed"),
            }
        })
        .await;
        if let Err(error) = result {
            tracing::error!(%error, "retention worker failed");
        }
    }
}
