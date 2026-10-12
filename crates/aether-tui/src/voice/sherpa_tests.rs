#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;

fn fixture(dir: &Path) -> (PathBuf, PathBuf) {
    let python = dir.join("python");
    super::super::test_script::write(
        &python,
        "#!/usr/bin/env python3\nimport sys\nassert sys.argv[1:3] == ['-I', '-c']\nassert sys.argv[-3] == 'transcribe'\nprint('Parakeet café')\n",
    );
    let models = dir.join("models");
    std::fs::create_dir(&models).unwrap();
    for name in MODEL_FILES {
        std::fs::write(models.join(name), "fake").unwrap();
    }
    (python, models)
}

#[test]
fn config_switch_preserves_whisper_speech_and_devices_and_rejects_stale_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let (python, models) = fixture(dir.path());
    let original = b"whisper = '/old/whisper'\nwhisper_model = '/old/model'\npiper = '/old/piper'\npiper_model = '/old/voice'\ninput_device = 'mic'\noutput_device = 'headphones'\n";
    std::fs::write(dir.path().join(config::FILE), original).unwrap();
    let snapshot = config_snapshot(dir.path()).unwrap().unwrap();
    save_config(
        dir.path(),
        Some(&snapshot),
        parse_config(Some(&snapshot)).unwrap(),
        &python,
        &python,
        &models,
    )
    .unwrap();
    let config = Config::load(dir.path()).unwrap();
    assert_eq!(config.stt_backend, config::SttBackend::Parakeet);
    assert_eq!(config.whisper, Some("/old/whisper".into()));
    assert_eq!(config.piper, Some("/old/piper".into()));
    assert_eq!(config.output_device.as_deref(), Some("headphones"));
    assert!(
        save_config(
            dir.path(),
            Some(&snapshot),
            parse_config(Some(&snapshot)).unwrap(),
            &python,
            &python,
            &models
        )
        .is_err()
    );
    assert_eq!(Config::load(dir.path()).unwrap(), config);
    std::fs::remove_file(models.join("tokens.txt")).unwrap();
    assert!(config.recording().is_err());
}

#[test]
fn configuration_boundary_and_cross_installer_lock_fail_closed() {
    let dir = tempfile::tempdir().unwrap();
    assert!(config_snapshot(dir.path()).unwrap().is_none());
    let lock = lock_setup(dir.path()).unwrap();
    assert!(lock_setup(dir.path()).is_err());
    drop(lock);
    assert!(lock_setup(dir.path()).is_ok());
    std::os::unix::fs::symlink("missing", dir.path().join(config::FILE)).unwrap();
    assert!(config_snapshot(dir.path()).is_err());
    assert!(parse_config(Some(b"not valid [toml")).is_err());
    let legacy: Config = toml::from_str("ffmpeg='/bin/ffmpeg'").unwrap();
    assert_eq!(legacy.stt_backend, config::SttBackend::WhisperCpp);
    assert!(toml::from_str::<Config>("ffmpeg='/bin/ffmpeg'\nstt_backend='unknown'").is_err());
}

#[tokio::test]
async fn parakeet_dispatch_is_local_bounded_and_cancellable() {
    let dir = tempfile::tempdir().unwrap();
    let (python, models) = fixture(dir.path());
    save_config(
        dir.path(),
        None,
        toml::Table::new(),
        &python,
        &python,
        &models,
    )
    .unwrap();
    let config = Config::load(dir.path()).unwrap();
    let (tx, mut control) = watch::channel(Control::Run);
    let transcript =
        super::super::backend::transcribe(&config, Path::new("/unused.wav"), &mut control)
            .await
            .unwrap();
    assert_eq!(transcript, "Parakeet café");
    tx.send_replace(Control::Cancel);
    assert!(
        transcribe(&config, Path::new("/unused.wav"), &mut control)
            .await
            .is_err()
    );
}

#[test]
fn adapter_unpacks_only_expected_regular_files() {
    let dir = tempfile::tempdir().unwrap();
    let test = r#"
import io, sys, tarfile
from pathlib import Path
scope = {'__name__': 'adapter_test'}
exec(sys.stdin.read(), scope)
root = Path(sys.argv[1])
prefix = scope['ARCHIVE_ROOT'] + '/'
names = list(scope['MODEL_FILES'])
def archive(path, entries):
    with tarfile.open(path, 'w:bz2') as output:
        for name, link in entries:
            info = tarfile.TarInfo(name)
            if link:
                info.type = tarfile.SYMTYPE
                info.linkname = '../../outside'
                output.addfile(info)
            else:
                info.size = 4
                output.addfile(info, io.BytesIO(b'fake'))
normal = [(prefix + name, False) for name in names]
archive(root / 'ok.tar.bz2', normal + [('../outside', False)])
scope['unpack'](root / 'ok.tar.bz2', root / 'ok')
assert sorted(p.name for p in (root / 'ok').iterdir()) == sorted(names)
assert not (root / 'outside').exists()
for case, entries in [('missing', normal[:-1]), ('duplicate', normal + normal[:1]), ('link', [(prefix + names[0], True)] + normal[1:])]:
    archive(root / (case + '.tar.bz2'), entries)
    try:
        scope['unpack'](root / (case + '.tar.bz2'), root / case)
    except ValueError:
        pass
    else:
        raise AssertionError(case + ' accepted')
"#;
    let mut child = std::process::Command::new("python3")
        .args(["-I", "-c", test])
        .arg(dir.path())
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(ADAPTER.as_bytes())
        .unwrap();
    assert!(child.wait().unwrap().success());
}

#[tokio::test]
#[ignore = "downloads ~465 MiB plus runtime wheels; requires AETHER_VOICE_SMOKE_DIR and AETHER_VOICE_SMOKE_WAV"]
async fn real_parakeet_install_and_transcription() {
    let root = PathBuf::from(
        std::env::var_os("AETHER_VOICE_SMOKE_DIR").expect("explicit scratch directory required"),
    );
    let audio = PathBuf::from(
        std::env::var_os("AETHER_VOICE_SMOKE_WAV").expect("explicit public sample WAV required"),
    );
    std::fs::create_dir_all(&root).unwrap();
    let prefs = tempfile::Builder::new()
        .prefix("parakeet-smoke-")
        .tempdir_in(root)
        .unwrap();
    let (_tx, mut control) = watch::channel(Control::Run);
    let result = install(prefs.path().to_owned(), &mut control, |stage| {
        eprintln!("{stage}")
    })
    .await;
    assert!(result.is_ok(), "{result:?}");
    let config = Config::load(prefs.path()).unwrap();
    let started = std::time::Instant::now();
    let text = transcribe(&config, &audio, &mut control).await.unwrap();
    eprintln!("Public sample transcript ({:?}): {text}", started.elapsed());
    assert!(!text.is_empty());
    eprintln!("Retained smoke install: {}", prefs.keep().display());
}
