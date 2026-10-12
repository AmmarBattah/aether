#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::os::unix::fs::PermissionsExt;

use super::super::config;
use super::*;

fn executable(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    super::super::test_script::write(&path, &format!("#!/bin/sh\n{body}\n"));
    path
}

fn fixture(dir: &Path) -> (PathBuf, PathBuf) {
    let piper = executable(dir, "piper", "exit 0");
    let model = dir.join(VOICE_NAME);
    std::fs::write(&model, "fake model").unwrap();
    std::fs::write(
        dir.join(format!("{VOICE_NAME}.json")),
        r#"{"phoneme_type":"espeak","audio":{"sample_rate":22050}}"#,
    )
    .unwrap();
    (piper, model)
}

#[test]
fn install_preserves_both_stt_backends_and_all_existing_settings() {
    let dir = tempfile::tempdir().unwrap();
    let (piper, model) = fixture(dir.path());
    for backend in ["parakeet", "whisper_cpp"] {
        let language = if backend == "whisper_cpp" {
            "fr"
        } else {
            "auto"
        };
        let original = format!(
            "ffmpeg={ffmpeg:?}\nstt_backend='{backend}'\nwhisper='/old/whisper'\nwhisper_model='/old/model'\nsherpa_python='/old/python'\nsherpa_model_dir='/old/parakeet'\nlanguage='{language}'\ninput_device='mic'\noutput_device='0'\npulse_socket='/a/local/socket'\n",
            ffmpeg = piper.to_str().unwrap()
        );
        std::fs::write(dir.path().join(config::FILE), &original).unwrap();
        let table = setup::parse_config(Some(original.as_bytes())).unwrap();
        let before = table.clone();
        let table = merged_config(table, Path::new("/not/used")).unwrap();
        save_config(dir.path(), Some(original.as_bytes()), table, &piper, &model).unwrap();
        let config = Config::load(dir.path()).unwrap();
        assert_eq!(config.speech().unwrap(), 22050);
        let after =
            setup::parse_config(setup::config_snapshot(dir.path()).unwrap().as_deref()).unwrap();
        for (key, value) in before {
            assert_eq!(after.get(&key), Some(&value), "changed existing {key}");
        }
        assert_eq!(after.len(), 12);
    }
}

#[test]
fn fresh_config_adds_only_tts_and_ffmpeg_and_rejects_unknown_or_invalid_settings() {
    let dir = tempfile::tempdir().unwrap();
    let (piper, model) = fixture(dir.path());
    let table = merged_config(toml::Table::new(), &piper).unwrap();
    save_config(dir.path(), None, table, &piper, &model).unwrap();
    let config = Config::load(dir.path()).unwrap();
    assert_eq!(config.stt_backend, config::SttBackend::WhisperCpp);
    assert_eq!(config.whisper, None);
    assert_eq!(config.speech().unwrap(), 22050);
    let bytes = setup::config_snapshot(dir.path()).unwrap().unwrap();
    assert_eq!(setup::parse_config(Some(&bytes)).unwrap().len(), 3);
    for invalid in [
        "unknown_setting = true",
        "stt_backend='invalid'",
        "piper=23",
    ] {
        let table = setup::parse_config(Some(invalid.as_bytes())).unwrap();
        assert!(merged_config(table, &piper).is_err());
        assert_eq!(setup::config_snapshot(dir.path()).unwrap().unwrap(), bytes);
    }
}

#[test]
fn stale_config_and_invalid_model_roll_back_without_replacing_existing_paths() {
    let dir = tempfile::tempdir().unwrap();
    let (piper, model) = fixture(dir.path());
    let table = merged_config(toml::Table::new(), &piper).unwrap();
    let original = b"# concurrent user edit\nffmpeg='/keep/me'\n";
    std::fs::write(dir.path().join(config::FILE), original).unwrap();
    let holder = tempfile::tempdir_in(dir.path()).unwrap();
    let stage = holder.path().to_owned();
    let result = setup::publish_install(holder, || {
        save_config(dir.path(), None, table.clone(), &piper, &model)
    });
    assert!(format!("{:#}", result.unwrap_err()).contains("changed while installing"));
    assert!(!stage.exists());
    assert_eq!(
        setup::config_snapshot(dir.path()).unwrap().unwrap(),
        original
    );
    std::fs::write(dir.path().join(format!("{VOICE_NAME}.json")), "{}").unwrap();
    assert!(save_config(dir.path(), Some(original), table, &piper, &model).is_err());
    assert_eq!(
        setup::config_snapshot(dir.path()).unwrap().unwrap(),
        original
    );
}

#[test]
fn piper_shares_setup_lock_and_safe_config_boundary() {
    let dir = tempfile::tempdir().unwrap();
    let lock = setup::lock_setup(dir.path()).unwrap();
    assert!(setup::lock_setup(dir.path()).is_err());
    drop(lock);
    assert!(setup::lock_setup(dir.path()).is_ok());
    std::os::unix::fs::symlink("missing", dir.path().join(config::FILE)).unwrap();
    assert!(setup::config_snapshot(dir.path()).is_err());
}

#[test]
fn every_asset_is_immutable_and_rejects_corruption_or_wrong_size() {
    for asset in &ASSETS {
        assert!(asset.url().contains(VOICE_REVISION));
        assert_eq!(asset.sha256.len(), 64);
        assert!(asset.verify(asset.bytes, asset.sha256).is_ok());
        assert!(asset.verify(asset.bytes + 1, asset.sha256).is_err());
        assert!(asset.verify(asset.bytes - 1, asset.sha256).is_err());
        assert!(asset.verify(asset.bytes, &"0".repeat(64)).is_err());
    }
    assert!(ASSETS[1].name.ends_with(".onnx.json"));
}

#[test]
fn runtime_install_is_hash_locked_binary_only_and_private() {
    let command = pip_command(
        Path::new("/private/python"),
        Path::new("/private/root"),
        Path::new("/private/requirements"),
    );
    let args: Vec<_> = command
        .as_std()
        .get_args()
        .map(|v| v.to_str().unwrap())
        .collect();
    assert_eq!(&args[..4], &["-I", "-m", "pip", "--isolated"]);
    for flag in [
        "--require-hashes",
        "--only-binary=:all:",
        "--no-cache-dir",
        "--no-input",
        "https://pypi.org/simple",
    ] {
        assert!(args.contains(&flag));
    }
    let env: Vec<_> = command.as_std().get_envs().collect();
    assert!(env.contains(&(
        std::ffi::OsStr::new("TMPDIR"),
        Some(std::ffi::OsStr::new("/private/root"))
    )));
    assert!(env.contains(&(
        std::ffi::OsStr::new("PIP_CONFIG_FILE"),
        Some(std::ffi::OsStr::new("/dev/null"))
    )));
    let mut package_count = 0;
    for line in REQUIREMENTS
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        if line.starts_with("    ") {
            let hash = line
                .trim()
                .trim_end_matches(" \\")
                .strip_prefix("--hash=sha256:")
                .unwrap();
            assert_eq!(hash.len(), 64);
            assert!(hash.bytes().all(|b| b.is_ascii_hexdigit()));
        } else {
            package_count += 1;
            assert!(line.contains("=="));
            assert!(line.ends_with(" \\"));
        }
    }
    assert_eq!(package_count, 9);
}

#[tokio::test]
async fn launcher_quotes_paths_and_isolates_python_without_audio() {
    let dir = tempfile::Builder::new()
        .prefix("piper ' space")
        .tempdir()
        .unwrap();
    let python = executable(dir.path(), "python", "printf '%s\\n' \"$@\"");
    let launcher = write_launcher(dir.path(), &python).unwrap();
    let mode = std::fs::metadata(&launcher).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o700, "{mode:o}");
    // Run its bytes from a fresh inode: a sibling test's fork may still hold
    // the write fd write_launcher had open on this one (ETXTBSY).
    let script = std::fs::read_to_string(&launcher).unwrap();
    std::fs::remove_file(&launcher).unwrap();
    super::super::test_script::write(&launcher, &script);
    let mut command = Command::new(launcher);
    command.args(["--model", "name with spaces", "--output-raw"]);
    let (_tx, mut control) = watch::channel(Control::Run);
    let output = process::run(
        command,
        &[],
        1024,
        Duration::from_secs(2),
        &mut control,
        false,
    )
    .await
    .unwrap();
    assert_eq!(
        String::from_utf8(output).unwrap(),
        "-I\n-m\npiper\n--model\nname with spaces\n--output-raw\n"
    );
}

#[tokio::test]
async fn cancelled_setup_and_insecure_download_do_not_touch_config() {
    let dir = tempfile::tempdir().unwrap();
    let prefs = dir.path().join("not-created");
    let (_tx, mut control) = watch::channel(Control::Cancel);
    let result = setup::install(prefs.clone(), setup::ModelKind::Piper, &mut control, |_| {
        panic!("cancelled setup made progress")
    })
    .await;
    assert!(format!("{:#}", result.unwrap_err()).contains("cancelled"));
    assert!(!prefs.exists());
    let destination = dir.path().join("download");
    assert!(
        download_inner("http://example.com", &destination, &ASSETS[0])
            .await
            .is_err()
    );
    assert!(!destination.exists());
}

#[tokio::test]
async fn subprocess_failure_timeout_and_cancellation_are_bounded() {
    let dir = tempfile::tempdir().unwrap();
    let failing = executable(dir.path(), "fail", "exit 7");
    let waiting = executable(dir.path(), "wait", "sleep 30 &\nwait");
    let (tx, mut control) = watch::channel(Control::Run);
    let error = run(
        Command::new(failing),
        Duration::from_secs(2),
        &mut control,
        "test stage",
    )
    .await
    .unwrap_err();
    assert!(format!("{error:#}").contains("test stage"));
    assert!(format!("{error:#}").contains("exit status: 7"));
    let error = run(
        Command::new(&waiting),
        Duration::from_millis(50),
        &mut control,
        "test timeout",
    )
    .await
    .unwrap_err();
    assert!(format!("{error:#}").contains("timed out"));
    let cancel = async {
        tokio::time::sleep(Duration::from_millis(50)).await;
        tx.send_replace(Control::Cancel);
    };
    let command = run(
        Command::new(waiting),
        Duration::from_secs(2),
        &mut control,
        "test cancel",
    );
    let (result, _) = tokio::join!(command, cancel);
    assert!(format!("{:#}", result.unwrap_err()).contains("cancelled"));
}

#[tokio::test]
#[ignore = "downloads verified voice and runtime; uses explicit worktree scratch only, synthesizes PCM without playback"]
async fn real_piper_install_and_synthesis_without_playback() {
    let cwd = std::env::current_dir().unwrap();
    let workspace = cwd
        .ancestors()
        .find(|p| p.join("crates/aether-tui").is_dir())
        .unwrap();
    let root = workspace.join("target/piper-smoke");
    std::fs::create_dir_all(&root).unwrap();
    let prefs = tempfile::tempdir_in(root).unwrap();
    let (_tx, mut control) = watch::channel(Control::Run);
    let result = setup::install(
        prefs.path().to_owned(),
        setup::ModelKind::Piper,
        &mut control,
        |stage| eprintln!("{stage}"),
    )
    .await
    .unwrap();
    assert!(result.contains("No audio was recorded or played"));
    let config = Config::load(prefs.path()).unwrap();
    assert_eq!(config.speech().unwrap(), 22050);
    assert!(config.whisper.is_none());
    assert!(config.sherpa_python.is_none());
    let (pcm, rate) = super::super::backend::synthesize(
        &config,
        "This is a local voice setup test.",
        &mut control,
    )
    .await
    .unwrap();
    assert_eq!(rate, 22050);
    assert!(!pcm.is_empty());
    assert_eq!(pcm.len() % 2, 0);
    eprintln!(
        "verified stdin --output-raw: {} PCM bytes at {rate} Hz; no playback",
        pcm.len()
    );
}

#[tokio::test]
async fn a_linked_voice_toml_stops_the_install_before_anything_runs_or_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let ran = dir.path().join("ran");
    let python = executable(dir.path(), "python", &format!("touch {:?}", ran));
    let prefs = std::fs::canonicalize(dir.path()).unwrap().join("prefs");
    std::fs::create_dir(&prefs).unwrap();
    let target = prefs.join("elsewhere.toml");
    std::fs::write(&target, "ffmpeg='/bin/ffmpeg'\n").unwrap();
    std::os::unix::fs::symlink(&target, prefs.join(config::FILE)).unwrap();
    let (_tx, mut control) = watch::channel(Control::Run);
    let error = install_with_tools(prefs.clone(), &python, &python, &mut control, |_| {})
        .await
        .unwrap_err();
    let error = format!("{error:#}");
    assert!(
        error.contains(prefs.join(config::FILE).to_str().unwrap()),
        "{error}"
    );
    assert!(error.contains("replace the link with a copy"), "{error}");
    assert!(!ran.exists(), "a refused install runs no tool");
    assert!(!prefs.join("voice").exists(), "and stages nothing");
    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        "ffmpeg='/bin/ffmpeg'\n"
    );
}

#[tokio::test]
async fn failed_runtime_rolls_back_staging_and_leaves_existing_config_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let python = executable(dir.path(), "python", "exit 1");
    let prefs = dir.path().join("prefs");
    std::fs::create_dir(&prefs).unwrap();
    let original = format!(
        "ffmpeg={:?}\nwhisper='/old/stt'\n",
        python.to_str().unwrap()
    );
    std::fs::write(prefs.join(config::FILE), &original).unwrap();
    let (_tx, mut control) = watch::channel(Control::Run);
    let error = install_with_tools(prefs.clone(), &python, &python, &mut control, |_| {})
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("check Python version"));
    assert_eq!(
        setup::config_snapshot(&prefs).unwrap().unwrap(),
        original.as_bytes()
    );
    assert_eq!(std::fs::read_dir(prefs.join("voice")).unwrap().count(), 0);
    assert!(setup::lock_setup(&prefs).is_ok());
}

#[test]
fn platform_preflight_rejects_unsupported_python_and_operating_systems() {
    use std::io::Write;
    let script = r#"
import platform, sys, sysconfig
check = sys.stdin.read()
cases = [
    ((3,11), 'linux', 'x86_64', ('glibc','2.27'), '14.0', 0, True),
    ((3,14), 'linux', 'aarch64', ('glibc','2.40'), '14.0', 0, True),
    ((3,14), 'darwin', 'arm64', ('',''), '14.0', 0, True),
    ((3,10), 'linux', 'x86_64', ('glibc','2.40'), '14.0', 0, False),
    ((3,15), 'linux', 'x86_64', ('glibc','2.40'), '14.0', 0, False),
    ((3,14), 'linux', 'x86_64', ('glibc','2.40'), '14.0', 1, False),
    ((3,14), 'linux', 'x86_64', ('musl','1.2'), '14.0', 0, False),
    ((3,14), 'linux', 'x86_64', ('glibc','2.17'), '14.0', 0, False),
    ((3,14), 'darwin', 'x86_64', ('',''), '14.0', 0, False),
    ((3,14), 'darwin', 'arm64', ('',''), '13.0', 0, False),
    ((3,14), 'win32', 'AMD64', ('',''), '14.0', 0, False),
]
for version, os, machine, libc, mac, free, expected in cases:
    sys.version_info = version
    sys.platform = os
    platform.machine = lambda: machine
    platform.libc_ver = lambda: libc
    platform.mac_ver = lambda: (mac, '', '')
    sysconfig.get_config_var = lambda _: free
    try:
        exec(check)
        accepted = True
    except SystemExit:
        accepted = False
    assert accepted == expected, (version, os, machine, libc, mac, free)
"#;
    let mut child = std::process::Command::new("python3")
        .args(["-I", "-c", script])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(CHECK_PYTHON.as_bytes())
        .unwrap();
    assert!(child.wait().unwrap().success());
}
