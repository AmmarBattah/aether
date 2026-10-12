#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::process::Command;

#[test]
fn invalid_ascii_environment_fails_before_daemon_start_even_in_plain_mode() {
    let root = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_aether"))
        .args(["--ascii", "--plain"])
        .env("AETHER_CONFIG_DIR", root.path().join("config"))
        .env("AETHER_DATA_DIR", root.path().join("data"))
        .env("AETHER_SOCKET", root.path().join("missing.sock"))
        .env("AETHER_ASCII", "perhaps")
        .env_remove("AETHER_THEME")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("AETHER_ASCII must be"), "{stderr}");
    assert!(!root.path().join("missing.sock").exists());
}

#[test]
fn invalid_ascii_preference_is_visible_not_silently_coerced() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(config.join("ui.toml"), "ascii = 'yes'\n").unwrap();
    // Stop at environment validation after the preference warning; no daemon is needed.
    let output = Command::new(env!("CARGO_BIN_EXE_aether"))
        .args(["--plain"])
        .env("AETHER_CONFIG_DIR", &config)
        .env("AETHER_DATA_DIR", root.path().join("data"))
        .env("AETHER_SOCKET", root.path().join("missing.sock"))
        .env("AETHER_ASCII", "invalid")
        .env_remove("AETHER_THEME")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("ignoring bad preferences"), "{stderr}");
    assert!(stderr.contains("ui.toml"), "{stderr}");
    assert!(stderr.contains("AETHER_ASCII must be"), "{stderr}");
}
