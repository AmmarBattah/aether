#![cfg(unix)]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::Path;
use std::time::Duration;
use tokio::process::Command;

fn fixture(path: &Path, logged_in: bool) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        path,
        r#"#!/bin/sh
case "$*" in
  --version) echo '2.1.269' ;;
  'auth status')
    if [ -f "$0.ready" ]; then echo '{"loggedIn": true}'; else echo '{"loggedIn": false}'; fi ;;
  'auth login') : > "$0.login"; : > "$0.ready" ;;
  'auth logout') : > "$0.logout"; /bin/rm "$0.ready" ;;
  *) exit 3 ;;
esac
"#,
    )
    .unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    if logged_in {
        std::fs::write(path.with_extension("ready"), "").unwrap();
    }
    assert!(path.is_absolute());
    assert!(path.is_file());
}

fn command(root: &Path, both_installs: bool, args: &[&str]) -> Command {
    assert!(root.is_dir());
    assert!(!args.is_empty());
    let dirs = if both_installs {
        vec![root.join("first"), root.join("second")]
    } else {
        vec![root.join("first")]
    };
    let mut command = Command::new(env!("CARGO_BIN_EXE_aether"));
    command
        .arg("--local")
        .args(args)
        .current_dir(root)
        .env("HOME", root)
        .env("PATH", std::env::join_paths(dirs).unwrap())
        .env("AETHER_SOCKET", root.join("a.sock"))
        .env("AETHER_CONFIG_DIR", root.join("config"))
        .env("AETHER_DATA_DIR", root.join("data"))
        .kill_on_drop(true);
    command
}

async fn output(root: &Path, both_installs: bool, args: &[&str]) -> String {
    let output = tokio::time::timeout(
        Duration::from_secs(20),
        command(root, both_installs, args).output(),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(!text.contains("invalid CLI auth configuration"), "{text}");
    text
}

async fn connect(root: &Path, daemon: &mut tokio::process::Child) -> aether_client::Client {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Ok(client) = aether_client::Client::connect(root.join("a.sock")).await {
                break client;
            }
            assert!(
                daemon.try_wait().unwrap().is_none(),
                "fixture daemon exited before connecting"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap()
}

async fn check_duplicate_warning(root: &Path, first: &Path, second: &Path) {
    let doctor = command(root, true, &["doctor", "--json"])
        .output()
        .await
        .unwrap();
    let report: serde_json::Value = serde_json::from_slice(&doctor.stdout).unwrap();
    let warning = report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["name"] == "cli-installs")
        .unwrap();
    assert_eq!(warning["level"], "warn");
    let detail = warning["detail"].as_str().unwrap();
    assert!(
        detail.contains(first.to_str().unwrap()) && detail.contains(second.to_str().unwrap()),
        "{detail}"
    );
    assert!(
        detail.contains(&format!("Aether uses {}", second.display())),
        "{detail}"
    );
}

#[tokio::test]
async fn client_login_and_logout_use_the_daemons_path_and_doctor_names_duplicates() {
    let root = tempfile::tempdir().unwrap();
    let first = root.path().join("first/claude");
    let second = root.path().join("second/claude");
    fixture(&first, false);
    fixture(&second, true);
    let mut daemon = command(root.path(), true, &["daemon", "--foreground"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let client = connect(root.path(), &mut daemon).await;
    let info = client
        .auth_status()
        .await
        .unwrap()
        .into_iter()
        .find(|info| info.provider == "claude-code")
        .unwrap();
    assert_eq!(info.program.as_deref(), second.to_str());
    assert!(
        info.methods.is_empty(),
        "ready CLI auth methods must remain unchanged"
    );
    output(root.path(), false, &["login", "claude-code"]).await;
    assert!(second.with_extension("login").exists());
    assert!(
        !first.with_extension("login").exists(),
        "client PATH install must not be used"
    );
    check_duplicate_warning(root.path(), &first, &second).await;
    output(root.path(), false, &["logout", "claude-code"]).await;
    assert!(second.with_extension("logout").exists());
    assert!(!first.with_extension("logout").exists());
    let table = output(root.path(), false, &["auth"]).await;
    assert!(
        table.contains("first/claude: not logged in") && table.contains("other installs exist"),
        "{table}"
    );
    daemon.kill().await.unwrap();
}
