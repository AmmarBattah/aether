#![cfg(unix)]
#![allow(clippy::expect_used, clippy::unwrap_used)]

mod support;

use std::path::Path;
use std::time::Duration;
use support::TestDaemon;

const FIXTURE: &str = "AETHER_CLI_AUTH_TEST_ROOT";

fn fixture(path: &Path, logged_in: bool) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let script = r#"#!/bin/sh
case "$*" in
  --version) echo '2.1.269' ;;
  'auth status')
    if [ -f "$0.ready" ]; then echo '{"loggedIn": true}'; else echo '{"loggedIn": false}'; fi ;;
  'auth login') : > "$0.ready" ;;
  'auth logout') /bin/rm "$0.ready" ;;
  *) exit 3 ;;
esac
"#;
    std::fs::write(path, script).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    if logged_in {
        std::fs::write(path.with_extension("ready"), "").unwrap();
    }
    assert!(path.is_absolute());
    assert!(path.is_file());
}

async fn isolated() -> bool {
    if std::env::var_os(FIXTURE).is_some() {
        return false;
    }
    let root = tempfile::tempdir().unwrap();
    fixture(&root.path().join("first/claude"), false);
    fixture(&root.path().join("second/claude"), true);
    let path =
        std::env::join_paths([root.path().join("first"), root.path().join("second")]).unwrap();
    let output = tokio::time::timeout(
        Duration::from_secs(60),
        tokio::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "daemon_reports_selected_install_and_refreshes_after_client_login",
                "--nocapture",
            ])
            .env(FIXTURE, root.path())
            .env("HOME", root.path())
            .env("PATH", path)
            .kill_on_drop(true)
            .output(),
    )
    .await
    .unwrap()
    .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{stdout}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("1 passed"), "{stdout}");
    true
}

#[tokio::test]
async fn daemon_reports_selected_install_and_refreshes_after_client_login() {
    if isolated().await {
        return;
    }
    let root = std::path::PathBuf::from(std::env::var_os(FIXTURE).unwrap());
    let daemon = TestDaemon::start_with_providers("[providers.codex]\nenabled = false\n").await;
    let client = aether_client::Client::connect(&daemon.socket)
        .await
        .unwrap();
    let selected = |rows: Vec<aether_protocol::AuthInfo>| {
        rows.into_iter()
            .find(|info| info.provider == "claude-code")
            .unwrap()
    };
    let info = selected(client.auth_status().await.unwrap());
    let second = root.join("second/claude");
    assert!(info.configured);
    assert_eq!(info.program.as_deref(), second.to_str());
    let engines = client.status().await.unwrap().engines;
    assert!(
        engines
            .iter()
            .any(|label| label.contains("claude-code/2.1.269")),
        "{engines:?}"
    );
    let mut cli = aether_model::Registry::builtin()
        .provider("claude-code")
        .unwrap()
        .auth
        .cli
        .clone()
        .unwrap();
    cli.program = info.program.unwrap();
    cli.logout().await.unwrap();
    let info = selected(client.auth_status().await.unwrap());
    assert!(!info.configured);
    let detail = info.detail.unwrap();
    assert!(detail.contains("first/claude: not logged in"), "{detail}");
    assert!(detail.contains("second/claude: not logged in"), "{detail}");
    assert_eq!(info.program.as_deref(), root.join("first/claude").to_str());
    cli.program = info.program.unwrap();
    cli.login().await.unwrap();
    let info = selected(client.auth_status().await.unwrap());
    assert!(
        info.configured,
        "daemon must re-probe after client-side login"
    );
    assert_eq!(info.program.as_deref(), root.join("first/claude").to_str());
    daemon.stop().await;
}
