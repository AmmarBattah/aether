//! Log setup. Interactive processes never log to the terminal they draw on.

use std::path::{Path, PathBuf};

use anyhow::Context as _;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::prelude::*;

mod bounded;
pub(crate) use bounded::check_log_file;

const LOG_SEGMENT_BYTES: u64 = 16 * 1024 * 1024;
const LOG_SEGMENTS: usize = 7;
const SEGMENTS_DIR: &str = "segments-v1";

/// Log to a rolling file under the data directory; also to stderr when
/// `stderr` is set. Returns the guard that flushes the file on drop.
pub fn init(
    file_name: &str,
    stderr: bool,
) -> anyhow::Result<tracing_appender::non_blocking::WorkerGuard> {
    let dir = aether_protocol::paths::log_dir();
    let appender = managed_appender(&dir, file_name)?;
    let (file_writer, guard) = tracing_appender::non_blocking(appender);
    let filter =
        || EnvFilter::try_from_env("AETHER_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(file_writer)
        .with_ansi(false)
        .with_filter(filter());
    let registry = tracing_subscriber::registry().with(file_layer);
    if stderr {
        registry
            .with(
                tracing_subscriber::fmt::layer()
                    .with_writer(std::io::stderr)
                    .with_filter(filter()),
            )
            .init();
    } else {
        registry.init();
    }
    Ok(guard)
}

pub fn log_path_hint(file_name: &str) -> String {
    log_file(file_name).display().to_string()
}

/// The file a stream is written to now; older segments sit beside it.
pub fn log_file(file_name: &str) -> PathBuf {
    aether_protocol::paths::log_dir()
        .join(file_name)
        .join(SEGMENTS_DIR)
        .join("events.0.log")
}

fn managed_appender(log_dir: &Path, file_name: &str) -> anyhow::Result<bounded::BoundedLog> {
    // A versioned namespace keeps size rotation separate from daily and
    // pre-migration files, which may still have active writers.
    let directory = log_dir.join(file_name).join(SEGMENTS_DIR);
    bounded::BoundedLog::new(directory.clone(), LOG_SEGMENT_BYTES, LOG_SEGMENTS).with_context(
        || {
            format!(
                "initialize log stream at {}",
                directory.join("events.0.log").display()
            )
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_retention_uses_its_own_namespace_and_preserves_legacy_logs() {
        use std::io::Write;
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("aetherd.log");
        std::fs::create_dir(&directory).unwrap();
        for day in 1..=12 {
            std::fs::write(
                directory.join(format!("events.2020-01-{day:02}.log")),
                "old",
            )
            .unwrap();
        }
        let legacy = root.path().join("aetherd.log.2020-01-01");
        std::fs::write(&legacy, "legacy").unwrap();
        let other = root.path().join("aether.log");
        std::fs::create_dir(&other).unwrap();
        let other_log = other.join("events.2020-01-01.log");
        std::fs::write(&other_log, "other stream").unwrap();
        let mut appender = managed_appender(root.path(), "aetherd.log").unwrap();
        appender.write_all(b"current").unwrap();
        drop(appender);
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 13);
        assert_eq!(
            std::fs::read_to_string(directory.join("segments-v1/events.0.log")).unwrap(),
            "current"
        );
        assert_eq!(std::fs::read_to_string(legacy).unwrap(), "legacy");
        assert_eq!(std::fs::read_to_string(other_log).unwrap(), "other stream");
    }
}
