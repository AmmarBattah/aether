//! Real subprocess deadlines, without invoking installed coding harnesses.
#![cfg(target_os = "linux")]

use super::*;

async fn assert_probe_timeout(capability: bool) {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("probe.sh");
    let pid_file = dir.path().join("probe.sh.pid");
    std::fs::write(&script, "echo $$ > \"$0.pid\"\nexec sleep 30\n").unwrap();
    let script = script.to_str().unwrap();
    let error = if capability {
        probe_capability("/bin/sh", &[script], "app-server", None)
            .await
            .unwrap_err()
    } else {
        probe_version_with_args("/bin/sh", &[script], None)
            .await
            .unwrap_err()
    };
    assert!(
        matches!(&error, DetectError::TimedOut { probe, .. }
        if probe == if capability { script } else { "--version" }),
        "{error}"
    );
    let pid: i32 = std::fs::read_to_string(pid_file)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let reaped = tokio::time::timeout(Duration::from_secs(5), async {
        while std::path::Path::new(&format!("/proc/{pid}")).exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    if reaped.is_err() {
        nix::sys::signal::kill(
            nix::unistd::Pid::from_raw(pid),
            nix::sys::signal::Signal::SIGKILL,
        )
        .expect("clean up only the surviving test fixture");
    }
    assert!(reaped.is_ok(), "timed-out probe survived: {pid}");
}

#[tokio::test]
async fn version_probe_times_out_and_reaps_child() {
    assert_probe_timeout(false).await;
}

#[tokio::test]
async fn capability_probe_times_out_and_reaps_child() {
    assert_probe_timeout(true).await;
}
