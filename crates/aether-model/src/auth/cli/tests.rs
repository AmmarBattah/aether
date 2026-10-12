#![allow(clippy::unwrap_used, clippy::expect_used)]
use super::*;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

fn executable(directory: &Path, name: &str, script: &str) -> PathBuf {
    assert!(directory.is_dir(), "fixture directory must exist");
    assert!(!name.contains('/'), "fixture name must be a program name");
    let path = directory.join(name);
    // A child writes the script so sibling forks cannot inherit a writable handle.
    let status = std::process::Command::new("/bin/sh")
        .args(["-c", r#"printf '%s' "$1" > "$2""#, "sh"])
        .arg(format!("#!/bin/sh\n{script}\n"))
        .arg(&path)
        .status()
        .expect("write executable fixture");
    assert!(status.success(), "writing fixture failed: {status}");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
        .expect("fixture filesystem operation");
    path.canonicalize().expect("fixture filesystem operation")
}

fn auth(program: &str) -> CliAuth {
    CliAuth {
        program: program.into(),
        status: vec!["status".into()],
        login: vec!["login".into()],
        logout: vec!["logout".into()],
        logged_in_markers: vec!["Logged in".into()],
        label: "fixture".into(),
    }
}

#[tokio::test]
async fn second_install_logged_in_and_path_order_wins() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let a = executable(first.path(), "claude", "echo 'Not logged in'");
    let b = executable(second.path(), "claude", "echo 'Logged in'");
    let path = std::env::join_paths([first.path(), second.path()]).unwrap();
    let candidates = discover_programs("claude", Some(&path), None);
    assert_eq!(candidates, [a.clone(), b.clone()]);
    let status = auth("claude").status_on(candidates).await;
    assert!(status.available && status.logged_in, "{status:?}");
    assert_eq!(status.program.as_deref(), b.to_str());
    executable(first.path(), "claude", "echo 'Logged in'");
    auth("claude").forget();
    let status = auth("claude").status_on(vec![a.clone(), b]).await;
    assert_eq!(status.program.as_deref(), a.to_str());
}

#[tokio::test]
async fn failures_name_every_install_and_reason() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let a = executable(first.path(), "codex", "echo 'Not logged in'");
    let b = executable(second.path(), "codex", "echo 'Logged in'; exit 7");
    let status = auth("codex").status_on(vec![a.clone(), b.clone()]).await;
    assert!(!status.logged_in && status.available);
    assert_eq!(status.program.as_deref(), a.to_str());
    for text in [
        a.to_str().unwrap(),
        b.to_str().unwrap(),
        "not logged in",
        "non-zero exit",
        "other installs exist",
    ] {
        assert!(status.detail.contains(text), "{status:?}");
    }
    assert_eq!(status.detail.lines().count(), 1);
}

#[tokio::test]
async fn all_timeouts_share_one_deadline_and_do_not_hide_a_later_login() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let a = executable(first.path(), "claude", "exec /bin/sleep 30");
    let b = executable(second.path(), "claude", "exec /bin/sleep 30");
    let started = Instant::now();
    let status = auth("claude")
        .probe_candidates(vec![a.clone(), b.clone()], Duration::from_millis(100))
        .await;
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "timeouts serialized"
    );
    assert!(!status.logged_in && status.detail.contains("timed out"));
    assert!(status.detail.contains(a.to_str().unwrap()));
    executable(second.path(), "claude", "echo 'Logged in'");
    let status = auth("claude")
        .probe_candidates(vec![a, b.clone()], Duration::from_millis(100))
        .await;
    assert!(status.logged_in);
    assert_eq!(status.program.as_deref(), b.to_str());
}

#[test]
fn discovery_deduplicates_paths_and_symlinks_and_finds_user_installs() {
    let root = tempfile::tempdir().unwrap();
    let first = root.path().join("path");
    let local = root.path().join(".local/bin");
    let bin = root.path().join("bin");
    for dir in [&first, &local, &bin] {
        std::fs::create_dir_all(dir).unwrap();
    }
    let a = executable(&first, "claude", "exit 0");
    std::os::unix::fs::symlink(&a, local.join("claude")).unwrap();
    let b = executable(&bin, "claude", "exit 0");
    let path = std::env::join_paths([&first, &first, &local]).unwrap();
    let candidates = discover_programs("claude", Some(&path), Some(root.path()));
    assert_eq!(candidates, [a, b]);
    assert!(candidates.iter().all(|p| p.is_absolute()));
}

#[test]
fn explicit_path_bypasses_discovery_even_if_missing_or_relative() {
    let root = tempfile::tempdir().unwrap();
    executable(root.path(), "claude", "echo 'Logged in'");
    for program in ["./custom/claude", "/missing/claude"] {
        let candidates =
            discover_programs(program, Some(root.path().as_os_str()), Some(root.path()));
        assert_eq!(candidates, [PathBuf::from(program)]);
        assert_eq!(candidates.len(), 1);
    }
}

#[tokio::test]
async fn login_and_logout_invalidate_cached_status() {
    let root = tempfile::tempdir().unwrap();
    let program = executable(
        root.path(),
        "claude",
        r#"
case "$1" in
  login) : > "$0.ready" ;;
  logout) /bin/rm "$0.ready" ;;
  status) if [ -f "$0.ready" ]; then echo 'Logged in'; else echo 'Not logged in'; fi ;;
esac"#,
    );
    let cli = auth(program.to_str().unwrap());
    assert!(!cli.status().await.logged_in);
    cli.login().await.unwrap();
    assert!(cli.status().await.logged_in);
    cli.logout().await.unwrap();
    assert!(!cli.status().await.logged_in);
}

#[tokio::test]
async fn missing_program_has_no_selected_install() {
    let cli = auth("uninstalled-fixture");
    let status = cli.status_on(Vec::new()).await;
    assert!(!status.available && !status.logged_in);
    assert!(status.program.is_none());
    assert!(status.detail.contains("uninstalled-fixture not found"));
}

#[tokio::test]
async fn invalid_cli_configuration_returns_a_useful_error() {
    for cli in [
        auth(""),
        CliAuth {
            logged_in_markers: vec![String::new()],
            ..auth("claude")
        },
    ] {
        let status = cli.status_on(Vec::new()).await;
        assert!(!status.available && !status.logged_in);
        assert!(status.detail.contains("invalid CLI auth configuration"));
    }
}

#[tokio::test]
async fn explicit_relative_path_is_probed_as_configured_and_reported_absolute() {
    let root = tempfile::tempdir_in(".").unwrap();
    let absolute = executable(root.path(), "claude", "echo 'Logged in'");
    let current = std::env::current_dir().unwrap();
    let relative = absolute.strip_prefix(current).unwrap();
    let cli = auth(relative.to_str().unwrap());
    assert_eq!(cli.candidates(), [relative.to_path_buf()]);
    let status = cli.status().await;
    assert!(status.logged_in, "{status:?}");
    assert_eq!(status.program.as_deref(), absolute.to_str());
}
