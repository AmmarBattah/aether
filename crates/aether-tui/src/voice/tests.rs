#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::process::Command;
use tokio::sync::watch;

use super::config::Config;
use super::process::{self, Control};
use super::{Effect, Reply, backend, service::Service};
use crate::app::AppEvent;
use crate::runner::LoopEvent;

fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    super::test_script::write(&path, &format!("#!/usr/bin/env python3\n{body}\n"));
    path
}

fn fixture(dir: &Path) -> Config {
    let ffmpeg = script(
        dir,
        "ffmpeg",
        "import sys\nif 'pipe:1' in sys.argv: sys.stdout.buffer.write(b'\\0\\0' * 1600)\nelse: assert len(sys.stdin.buffer.read()) == 3200",
    );
    let whisper = script(
        dir,
        "whisper",
        "import sys, wave\np=sys.argv[sys.argv.index('-f')+1]\nwith wave.open(p) as w: assert w.getnchannels()==1 and w.getframerate()==16000\nprint('名前 café --help')",
    );
    let piper = script(
        dir,
        "piper",
        "import sys\nassert sys.argv[-1]=='--output-raw'\nassert sys.stdin.read() == '名前 asks: --help; $(echo nope)'\nsys.stdout.buffer.write(b'\\0\\0' * 1600)",
    );
    let model = dir.join("model.bin");
    let voice = dir.join("voice.onnx");
    std::fs::write(&model, b"fake model").unwrap();
    std::fs::write(&voice, b"fake model").unwrap();
    std::fs::write(
        dir.join("voice.onnx.json"),
        r#"{"audio":{"sample_rate":16000}}"#,
    )
    .unwrap();
    Config {
        ffmpeg,
        stt_backend: super::config::SttBackend::WhisperCpp,
        sherpa_python: None,
        sherpa_model_dir: None,
        whisper: Some(whisper),
        whisper_model: Some(model),
        piper: Some(piper),
        piper_model: Some(voice),
        language: "auto".into(),
        input_device: None,
        output_device: None,
        pulse_socket: None,
    }
}

#[tokio::test]
async fn fake_local_backend_record_transcribe_synthesize_play_and_private_cleanup() {
    let dir = tempfile::tempdir().unwrap();
    let config = fixture(dir.path());
    let (_tx, mut rx) = watch::channel(Control::Run);
    let file = backend::record(&config, &mut rx).await.unwrap();
    let path = file.path().to_owned();
    assert_eq!(
        file.as_file().metadata().unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        backend::transcribe(&config, file.path(), &mut rx)
            .await
            .unwrap(),
        "名前 café --help"
    );
    file.close().unwrap();
    assert!(!path.exists());
    let (pcm, rate) = backend::synthesize(&config, "名前 asks: --help; $(echo nope)", &mut rx)
        .await
        .unwrap();
    backend::play(&config, &pcm, rate, &mut rx).await.unwrap();
}

#[test]
fn configuration_missing_invalid_and_nonlocal_models_are_actionable() {
    let dir = tempfile::tempdir().unwrap();
    assert!(format!("{:#}", Config::load(dir.path()).unwrap_err()).contains("voice.toml"));
    let mut config = fixture(dir.path());
    config.whisper = Some(dir.path().join("missing"));
    assert!(
        config
            .recording()
            .unwrap_err()
            .to_string()
            .contains("whisper")
    );
    config = fixture(dir.path());
    config.whisper_model = Some(PathBuf::from("https://model.example/model.bin"));
    assert!(
        config
            .recording()
            .unwrap_err()
            .to_string()
            .contains("absolute local path")
    );
    config.whisper_model = Some(dir.path().join("absent-model"));
    assert!(config.recording().is_err());
    config = fixture(dir.path());
    config.language = "--download".into();
    assert!(config.recording().is_err());
    std::fs::write(
        dir.path().join("voice.onnx.json"),
        r#"{"audio":{"sample_rate":1}}"#,
    )
    .unwrap();
    assert!(config.speech().is_err());
    std::fs::write(
        dir.path().join("voice.toml"),
        "ffmpeg = 'relative'\ncloud = true",
    )
    .unwrap();
    assert!(Config::load(dir.path()).is_err());
    std::fs::write(dir.path().join("voice.toml"), vec![b'x'; 16385]).unwrap();
    assert!(format!("{:#}", Config::load(dir.path()).unwrap_err()).contains("exceeds"));
}

#[test]
fn transcript_validation_preserves_unicode_and_rejects_bad_outputs() {
    assert_eq!(
        backend::transcript("  名前 café --help\n".as_bytes().to_vec()).unwrap(),
        "名前 café --help"
    );
    for bytes in [
        vec![0xff],
        b"\x1b[2J".to_vec(),
        b"\0bad".to_vec(),
        b"  \n".to_vec(),
        vec![b'a'; backend::TEXT_LIMIT + 1],
    ] {
        assert!(backend::transcript(bytes).is_err());
    }
}

#[test]
fn offline_config_rejects_auxiliary_download_backends_and_remote_pulse_routes() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = fixture(dir.path());
    std::fs::write(
        dir.path().join("voice.onnx.json"),
        r#"{"audio":{"sample_rate":16000},"phoneme_type":"pinyin"}"#,
    )
    .unwrap();
    assert!(
        config
            .speech()
            .unwrap_err()
            .to_string()
            .contains("download")
    );
    config.pulse_socket = Some("tcp:remote.example".into());
    assert!(config.pulse_server().is_err());
    config.pulse_socket = Some("/local/socket tcp:remote.example".into());
    assert!(config.pulse_server().is_err());
    config.pulse_socket = Some("/local/socket".into());
    assert_eq!(config.pulse_server().unwrap(), "unix:/local/socket");
}

#[test]
fn voice_debug_output_does_not_expose_audio_transcripts_or_questions() {
    let reply = Reply::Recorded {
        id: 1,
        result: Ok("private transcript".into()),
    };
    assert!(!format!("{reply:?}").contains("private"));
    let effect = Effect::Synthesize {
        id: 2,
        dir: "/private/path".into(),
        text: "private question".into(),
    };
    assert!(!format!("{effect:?}").contains("private"));
}

#[tokio::test]
async fn process_missing_nonzero_invalid_and_bounded_outputs() {
    let dir = tempfile::tempdir().unwrap();
    let (_tx, mut rx) = watch::channel(Control::Run);
    let timeout = Duration::from_secs(3);
    assert!(
        process::run(
            Command::new(dir.path().join("missing")),
            &[],
            8,
            timeout,
            &mut rx,
            false
        )
        .await
        .is_err()
    );
    for (name, code, expected) in [
        ("exit", "raise SystemExit(17)", "exit status: 17"),
        (
            "stdout",
            "import sys; sys.stdout.buffer.write(b'x'*10000)",
            "output limit",
        ),
        (
            "stderr",
            "import sys; sys.stderr.buffer.write(b'x'*20000)",
            "output limit",
        ),
    ] {
        let command = Command::new(script(dir.path(), name, code));
        let error = process::run(command, &[], 8, timeout, &mut rx, false)
            .await
            .unwrap_err();
        assert!(format!("{error:#}").contains(expected), "{error:#}");
    }
    let mut config = fixture(dir.path());
    config.piper = Some(script(
        dir.path(),
        "odd-pcm",
        "import sys; sys.stdin.read(); sys.stdout.buffer.write(b'x')",
    ));
    assert!(
        backend::synthesize(&config, "test", &mut rx)
            .await
            .unwrap_err()
            .to_string()
            .contains("incomplete")
    );
}

async fn wait_file(path: &Path) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while !path.metadata().is_ok_and(|m| m.len() > 0) {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}

fn stopped(pid: i32) -> bool {
    #[cfg(target_os = "linux")]
    if std::fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|s| {
        s.split_once(") ")
            .is_some_and(|(_, rest)| rest.starts_with('Z'))
    }) {
        return true;
    }
    nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid), None) == Err(nix::errno::Errno::ESRCH)
}

#[tokio::test]
async fn stop_cancel_timeout_kill_process_groups_and_reap_leaders() {
    for mode in [Control::Stop, Control::Cancel, Control::Run] {
        let dir = tempfile::tempdir().unwrap();
        let ready = dir.path().join("ready");
        let exe = script(
            dir.path(),
            "hanging",
            "import os, sys, subprocess, time\np=subprocess.Popen(['sleep','30'])\nsys.stdout.buffer.write(b'hello'); sys.stdout.buffer.flush()\nopen(sys.argv[1], 'w').write(str(os.getpid())+' '+str(p.pid))\ntime.sleep(30)",
        );
        let mut command = Command::new(exe);
        command.arg(&ready);
        let (tx, mut rx) = watch::channel(Control::Run);
        let timeout = if mode == Control::Run {
            Duration::from_secs(2)
        } else {
            Duration::from_secs(5)
        };
        let task = tokio::spawn(async move {
            process::run(command, &[], 1024, timeout, &mut rx, mode == Control::Stop).await
        });
        wait_file(&ready).await;
        let pids: Vec<i32> = std::fs::read_to_string(&ready)
            .unwrap()
            .split_whitespace()
            .map(|s| s.parse().unwrap())
            .collect();
        if mode != Control::Run {
            tx.send_replace(mode);
        }
        let result = task.await.unwrap();
        if mode == Control::Stop {
            assert_eq!(result.unwrap(), b"hello");
        } else {
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains(if mode == Control::Cancel {
                        "cancelled"
                    } else {
                        "timed out"
                    })
            );
        }
        assert!(stopped(pids[0]), "leader was not reaped");
        tokio::time::timeout(Duration::from_secs(3), async {
            while !stopped(pids[1]) {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("descendant killed");
    }
}

fn save_config(dir: &Path, config: &Config) {
    let quote = |p: &Path| serde_json::to_string(&p.to_string_lossy()).unwrap();
    let text = format!(
        "ffmpeg = {}\nwhisper = {}\nwhisper_model = {}\npiper = {}\npiper_model = {}\n",
        quote(&config.ffmpeg),
        quote(config.whisper.as_deref().unwrap()),
        quote(config.whisper_model.as_deref().unwrap()),
        quote(config.piper.as_deref().unwrap()),
        quote(config.piper_model.as_deref().unwrap())
    );
    std::fs::write(dir.join("voice.toml"), text).unwrap();
}

#[tokio::test]
async fn config_check_never_executes_and_service_shutdown_cancels_audio() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = fixture(dir.path());
    let ready = dir.path().join("recording");
    config.ffmpeg = script(
        dir.path(),
        "recording-ffmpeg",
        &format!(
            "import os, time\nopen({}, 'w').write(str(os.getpid()))\ntime.sleep(30)",
            serde_json::to_string(&ready.to_string_lossy()).unwrap()
        ),
    );
    save_config(dir.path(), &config);
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let mut service = Service::default();
    service.dispatch(
        Effect::Check {
            id: 1,
            dir: dir.path().into(),
            speech: false,
        },
        tx.clone(),
    );
    let event = tokio::time::timeout(Duration::from_secs(3), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        event,
        LoopEvent::App(AppEvent::Voice(Reply::Checked { result: Ok(_), .. }))
    ));
    assert!(!ready.exists(), "check must never execute audio tools");
    service.dispatch(
        Effect::Record {
            id: 2,
            dir: dir.path().into(),
        },
        tx,
    );
    wait_file(&ready).await;
    let pid: i32 = std::fs::read_to_string(&ready).unwrap().parse().unwrap();
    service.shutdown().await.unwrap();
    assert!(stopped(pid));
    assert!(matches!(
        rx.recv().await.unwrap(),
        LoopEvent::App(AppEvent::Voice(Reply::Recorded { result: Err(_), .. }))
    ));
}

#[tokio::test]
async fn reply_service_reports_audio_then_transcription_and_result_only_after_stop() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = fixture(dir.path());
    config.ffmpeg = script(
        dir.path(),
        "reply-recorder",
        "import sys, time\nsys.stdout.buffer.write(b'\\0\\0' * 1600); sys.stdout.buffer.flush()\ntime.sleep(30)",
    );
    save_config(dir.path(), &config);
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let mut service = Service::default();
    service.dispatch(
        Effect::RecordReply {
            id: 42,
            dir: dir.path().into(),
        },
        tx.clone(),
    );
    let event = tokio::time::timeout(Duration::from_secs(3), rx.recv())
        .await
        .unwrap();
    assert!(matches!(
        event,
        Some(LoopEvent::App(AppEvent::Voice(Reply::Recording(42))))
    ));
    assert!(
        rx.try_recv().is_err(),
        "no transcription without explicit stop"
    );
    service.dispatch(Effect::Stop(42), tx.clone());
    let event = tokio::time::timeout(Duration::from_secs(3), rx.recv())
        .await
        .unwrap();
    assert!(matches!(
        event,
        Some(LoopEvent::App(AppEvent::Voice(Reply::Transcribing(42))))
    ));
    let event = tokio::time::timeout(Duration::from_secs(3), rx.recv())
        .await
        .unwrap();
    assert!(matches!(
        event,
        Some(LoopEvent::App(AppEvent::Voice(Reply::Recorded {
            id: 42,
            result: Ok(_)
        })))
    ));
    service.shutdown().await.unwrap();

    config.ffmpeg = script(dir.path(), "failed-reply-recorder", "raise SystemExit(17)");
    save_config(dir.path(), &config);
    service.dispatch(
        Effect::RecordReply {
            id: 43,
            dir: dir.path().into(),
        },
        tx,
    );
    let event = tokio::time::timeout(Duration::from_secs(3), rx.recv())
        .await
        .unwrap();
    assert!(
        matches!(
            event,
            Some(LoopEvent::App(AppEvent::Voice(Reply::Recorded {
                id: 43,
                result: Err(_)
            })))
        ),
        "failure must not announce recording"
    );
    service.shutdown().await.unwrap();
}
