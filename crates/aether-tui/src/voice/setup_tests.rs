#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::os::unix::fs::PermissionsExt;

use tokio::sync::watch;

use super::download::download_inner;
use super::*;

fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    super::super::test_script::write(&path, &format!("#!/bin/sh\nset -e\n{body}\n"));
    path
}

/// A fake `git` supporting exactly the two invocations `install_backend`
/// makes: `clone ... <dest>` and `-C <dir> rev-parse HEAD`.
fn fake_git(dir: &Path, head: &str) -> PathBuf {
    script(
        dir,
        "git",
        &format!(
            r#"case "$1" in
  clone) dest=""; for a in "$@"; do dest="$a"; done; mkdir -p "$dest" ;;
  -C) echo "{head}" ;;
  *) exit 1 ;;
esac"#
        ),
    )
}

/// A fake `cmake`: `-B ... -S ...` is a no-op, `--build <dir> ...` either
/// produces `<dir>/bin/whisper-cli` or, if `produce_binary` is false, does
/// nothing - simulating a build that silently fails to produce its output.
fn fake_cmake(dir: &Path, produce_binary: bool) -> PathBuf {
    let build_case = if produce_binary {
        r#"build_dir="$2"; mkdir -p "$build_dir/bin"; printf '#!/bin/sh\nexit 0\n' > "$build_dir/bin/whisper-cli"; chmod +x "$build_dir/bin/whisper-cli""#
    } else {
        r#"build_dir="$2"; mkdir -p "$build_dir""#
    };
    script(
        dir,
        "cmake",
        &format!(
            r#"case "$1" in
  -B) exit 0 ;;
  --build) {build_case} ;;
  *) exit 1 ;;
esac"#
        ),
    )
}

/// Keeps the sender alive: a dropped sender makes `changed()` resolve
/// immediately with an error, which `interrupted()` reads as "cancelled".
fn run_control() -> (watch::Sender<Control>, watch::Receiver<Control>) {
    watch::channel(Control::Run)
}

#[test]
fn prerequisite_detection_reports_every_gap_and_nothing_when_all_present() {
    let dir = tempfile::tempdir().unwrap();
    let empty = std::ffi::OsString::from(dir.path());
    assert_eq!(missing_prerequisites_in(Some(&empty)).len(), 5);

    for name in ["git", "cmake", "ffmpeg", "cc", "c++", "make"] {
        script(dir.path(), name, "exit 0");
    }
    assert!(missing_prerequisites_in(Some(&empty)).is_empty());
    assert!(missing_prerequisites_in(None).len() >= 5);
}

#[test]
fn parakeet_requires_no_whisper_build_tools() {
    let dir = tempfile::tempdir().unwrap();
    let path = std::ffi::OsString::from(dir.path());
    for name in ["python3", "ffmpeg"] {
        script(dir.path(), name, "exit 0");
    }
    assert!(prerequisites_for(ModelKind::Parakeet, Some(&path)).is_empty());
    assert!(prerequisites_for(ModelKind::Piper, Some(&path)).is_empty());
    assert!(!prerequisites_for(ModelKind::Base, Some(&path)).is_empty());
}

#[test]
fn publication_retains_install_before_config_becomes_visible() {
    let root = tempfile::tempdir().unwrap();
    let install = tempfile::tempdir_in(root.path()).unwrap();
    let path = install.path().to_owned();
    let config = root.path().join("published");
    let result = std::panic::catch_unwind(|| {
        let _ = publish_install(install, || {
            std::fs::write(&config, path.to_string_lossy().as_bytes()).unwrap();
            panic!("simulated unwind after publication")
        });
    });
    assert!(result.is_err());
    assert!(config.is_file());
    assert!(path.is_dir(), "published install must not be RAII-deleted");

    let failed = tempfile::tempdir_in(root.path()).unwrap();
    let failed_path = failed.path().to_owned();
    let error = publish_install(failed, || anyhow::bail!("config changed")).unwrap_err();
    assert!(error.to_string().contains("config changed"));
    assert!(
        !failed_path.exists(),
        "failed publication should clean only its new install"
    );
    assert!(path.is_dir());
}

#[tokio::test]
#[ignore = "downloads/builds whisper.cpp and a 75MiB model; requires AETHER_VOICE_SMOKE_DIR and AETHER_VOICE_SMOKE_WAV"]
async fn real_whisper_install_and_transcription() {
    let root = PathBuf::from(
        std::env::var_os("AETHER_VOICE_SMOKE_DIR").expect("explicit scratch directory required"),
    );
    let audio = PathBuf::from(
        std::env::var_os("AETHER_VOICE_SMOKE_WAV").expect("explicit public sample WAV required"),
    );
    std::fs::create_dir_all(&root).unwrap();
    let prefs = tempfile::Builder::new()
        .prefix("whisper-smoke-")
        .tempdir_in(root)
        .unwrap();
    let (_tx, mut control) = run_control();
    let result = install(
        prefs.path().to_owned(),
        ModelKind::Tiny,
        &mut control,
        |stage| eprintln!("{stage}"),
    )
    .await;
    assert!(result.is_ok(), "{result:?}");
    let config = Config::load(prefs.path()).unwrap();
    let text = super::super::backend::transcribe(&config, &audio, &mut control)
        .await
        .unwrap();
    eprintln!("Public sample transcript: {text}");
    assert!(!text.is_empty());
    eprintln!("Retained smoke install: {}", prefs.keep().display());
}

#[test]
fn find_in_path_requires_the_executable_bit() {
    let dir = tempfile::tempdir().unwrap();
    let path = std::ffi::OsString::from(dir.path());
    std::fs::write(dir.path().join("git"), "not executable").unwrap();
    assert!(find_in_path(Some(&path), "git").is_none());
    std::fs::set_permissions(
        dir.path().join("git"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    assert_eq!(
        find_in_path(Some(&path), "git"),
        Some(dir.path().join("git"))
    );
}

#[test]
fn blurbs_say_multilingual_not_any_language() {
    for model in [ModelKind::Tiny, ModelKind::Base, ModelKind::Small] {
        assert!(model.blurb().contains("multilingual"), "{}", model.blurb());
        assert!(!model.blurb().to_lowercase().contains("any language"));
    }
}

#[test]
fn config_snapshot_treats_absence_as_none_and_refuses_a_link_with_its_path_and_the_fix() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(config::FILE);
    assert!(config_snapshot(dir.path()).unwrap().is_none());

    // A link to a perfectly good file is refused as firmly as a dangling one.
    let target = dir.path().join("dotfiles-voice.toml");
    std::fs::write(&target, "ffmpeg = '/bin/ffmpeg'\n").unwrap();
    for points_to in [target.as_path(), Path::new("nowhere")] {
        std::os::unix::fs::symlink(points_to, &path).unwrap();
        let error = config_snapshot(dir.path()).unwrap_err().to_string();
        assert!(error.contains(path.to_str().unwrap()), "{error}");
        assert!(error.contains("is a link"), "{error}");
        assert!(
            error.contains("replace the link with a copy of the file it points to"),
            "{error}"
        );
        std::fs::remove_file(&path).unwrap();
    }
    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        "ffmpeg = '/bin/ffmpeg'\n",
        "a refused link leaves its target untouched"
    );

    std::fs::create_dir(&path).unwrap();
    let error = config_snapshot(dir.path()).unwrap_err().to_string();
    assert!(error.contains(path.to_str().unwrap()), "{error}");
    assert!(error.contains("is not a regular file"), "{error}");
    assert!(!error.contains("link"), "{error}");
}

#[test]
fn save_config_merges_new_fields_preserves_the_rest_and_rejects_a_stale_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let ffmpeg = script(dir.path(), "ffmpeg", "exit 0");
    let whisper = script(dir.path(), "whisper", "exit 0");
    let model = dir.path().join("model.bin");
    std::fs::write(&model, b"fake").unwrap();

    // `ffmpeg` is deliberately absent so `save_config` inserts the real,
    // executable script path rather than preserving a stale, nonexistent one.
    let original = b"sherpa_python = '/old/python'\nsherpa_model_dir = '/old/models'\npiper = '/old/piper'\nlanguage = 'en'\n";
    std::fs::write(dir.path().join(config::FILE), original).unwrap();
    let snapshot = config_snapshot(dir.path()).unwrap().unwrap();
    save_config(
        dir.path(),
        Some(&snapshot),
        parse_config(Some(&snapshot)).unwrap(),
        &ffmpeg,
        &whisper,
        &model,
    )
    .unwrap();

    let config = Config::load(dir.path()).unwrap();
    assert_eq!(config.stt_backend, config::SttBackend::WhisperCpp);
    assert_eq!(config.whisper, Some(whisper.clone()));
    assert_eq!(config.ffmpeg, ffmpeg);
    assert_eq!(config.sherpa_model_dir, Some("/old/models".into()));
    assert_eq!(config.piper, Some("/old/piper".into()));
    // An existing explicit language choice must survive a Whisper install.
    assert_eq!(config.language, "en");

    // The file changed underneath the stale snapshot: refuse rather than clobber it.
    let result = save_config(
        dir.path(),
        Some(&snapshot),
        parse_config(Some(&snapshot)).unwrap(),
        &ffmpeg,
        &whisper,
        &model,
    );
    assert!(result.is_err());
    assert_eq!(Config::load(dir.path()).unwrap(), config);
}

#[tokio::test]
async fn install_backend_verifies_the_pinned_commit_and_the_built_binary() {
    let dir = tempfile::tempdir().unwrap();
    let git = fake_git(dir.path(), WHISPER_COMMIT);
    let cmake = fake_cmake(dir.path(), true);
    let repo_dir = dir.path().join("repo");
    let logs_dir = dir.path().to_owned();
    let (_tx, mut control) = run_control();
    let mut noop = |_: String| {};

    let cli = install_backend(&git, &cmake, &repo_dir, &logs_dir, &mut control, &mut noop)
        .await
        .unwrap();
    assert_eq!(cli, repo_dir.join("build/bin/whisper-cli"));
    assert!(cli.is_file());
}

#[tokio::test]
async fn install_backend_refuses_a_clone_that_resolves_to_the_wrong_commit() {
    let dir = tempfile::tempdir().unwrap();
    let git = fake_git(dir.path(), "0000000000000000000000000000000000000000");
    let cmake = fake_cmake(dir.path(), true);
    let (_tx, mut control) = run_control();
    let mut noop = |_: String| {};

    let error = install_backend(
        &git,
        &cmake,
        &dir.path().join("repo"),
        dir.path(),
        &mut control,
        &mut noop,
    )
    .await
    .unwrap_err();
    assert!(format!("{error:#}").contains("not the pinned"), "{error:#}");
}

#[tokio::test]
async fn install_backend_reports_a_build_that_produced_no_binary() {
    let dir = tempfile::tempdir().unwrap();
    let git = fake_git(dir.path(), WHISPER_COMMIT);
    let cmake = fake_cmake(dir.path(), false);
    let (_tx, mut control) = run_control();
    let mut noop = |_: String| {};

    let error = install_backend(
        &git,
        &cmake,
        &dir.path().join("repo"),
        dir.path(),
        &mut control,
        &mut noop,
    )
    .await
    .unwrap_err();
    assert!(format!("{error:#}").contains("not produced"), "{error:#}");
}

#[tokio::test]
async fn install_backend_is_cancelled_before_running_anything() {
    let dir = tempfile::tempdir().unwrap();
    let git = fake_git(dir.path(), WHISPER_COMMIT);
    let cmake = fake_cmake(dir.path(), true);
    let (tx, mut control) = watch::channel(Control::Run);
    tx.send_replace(Control::Cancel);
    let mut noop = |_: String| {};

    let error = install_backend(
        &git,
        &cmake,
        &dir.path().join("repo"),
        dir.path(),
        &mut control,
        &mut noop,
    )
    .await
    .unwrap_err();
    assert!(format!("{error:#}").contains("cancelled"), "{error:#}");
    assert!(
        !dir.path().join("repo").exists(),
        "clone must never have run"
    );
}

#[tokio::test]
async fn download_inner_refuses_a_plain_http_url_without_any_network_access() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("model.bin");
    let error = download_inner("http://example.com/model.bin", &dest, 1024, "irrelevant")
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("HTTPS"), "{error:#}");
    assert!(!dest.exists());
}

#[test]
fn model_kind_covers_every_picker_entry_with_a_distinct_label() {
    let labels: Vec<&str> = ModelKind::ALL.iter().map(|m| m.label()).collect();
    assert_eq!(labels.len(), 5);
    assert_eq!(
        ModelKind::ALL.iter().filter(|m| m.speech_output()).count(),
        1
    );
    assert!(ModelKind::Piper.speech_output());
    let mut sorted = labels.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        labels.len(),
        "labels must be unique: {labels:?}"
    );
}

#[test]
fn setup_guard_releases_lock_even_when_an_inherited_descriptor_remains_open() {
    let dir = tempfile::tempdir().unwrap();
    let lock = lock_setup(dir.path()).unwrap();
    let inherited = lock.0.try_clone().unwrap();
    assert!(lock_setup(dir.path()).is_err());
    drop(lock);
    assert!(lock_setup(dir.path()).is_ok());
    drop(inherited);
}
