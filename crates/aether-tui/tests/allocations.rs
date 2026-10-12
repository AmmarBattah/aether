#![allow(clippy::expect_used)]

//! Run the counting allocator outside the workspace's unsafe-code prohibition.
//! Cargo's artifact messages select the exact libraries for this build, even
//! when the target directory contains multiple profiles or dependency versions.

use std::path::PathBuf;
use std::process::Command;

#[test]
fn warm_transcript_allocations_depend_on_the_viewport() {
    let mut cargo = Command::new(env!("CARGO"));
    cargo.current_dir(env!("CARGO_MANIFEST_DIR")).args([
        "build",
        "-p",
        "aether-tui",
        "--lib",
        "--message-format=json",
    ]);
    if !cfg!(debug_assertions) {
        cargo.arg("--release");
    }
    let build = cargo.output().expect("build allocation probe dependencies");
    assert!(
        build.status.success(),
        "probe dependency build failed:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let artifacts: Vec<serde_json::Value> = String::from_utf8_lossy(&build.stdout)
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("Cargo JSON message"))
        .filter(|message| message["reason"] == "compiler-artifact")
        .collect();
    let library = |name: &str| -> PathBuf {
        artifacts
            .iter()
            .find(|message| message["target"]["name"] == name)
            .expect("library artifact")["filenames"]
            .as_array()
            .expect("artifact filenames")
            .iter()
            .filter_map(|value| value.as_str())
            .map(PathBuf::from)
            .find(|path| path.extension().is_some_and(|ext| ext == "rlib"))
            .expect("rlib artifact")
    };
    let tui = library("aether_tui");
    let ratatui = library("ratatui");
    let temporary = tempfile::tempdir().expect("allocation probe directory");
    let binary = temporary.path().join("warm-allocations");
    let compile = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .args(["--edition=2024", "--test"])
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/support/warm_allocations.rs"))
        .arg("--extern")
        .arg(format!("aether_tui={}", tui.display()))
        .arg("--extern")
        .arg(format!("ratatui={}", ratatui.display()))
        .arg("-L")
        .arg(format!(
            "dependency={}",
            ratatui.parent().expect("deps directory").display()
        ))
        .arg("-o")
        .arg(&binary)
        .output()
        .expect("compile allocation probe");
    assert!(
        compile.status.success(),
        "allocation probe compilation failed:\n{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let measured = Command::new(binary)
        .args(["--test-threads=1", "--nocapture"])
        .output()
        .expect("run allocation probe");
    println!("{}", String::from_utf8_lossy(&measured.stdout));
    assert!(
        measured.status.success(),
        "allocation probe failed:\n{}\n{}",
        String::from_utf8_lossy(&measured.stdout),
        String::from_utf8_lossy(&measured.stderr)
    );
}
