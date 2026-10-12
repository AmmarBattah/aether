use std::collections::HashMap;
use std::path::Path;

use tokio::sync::{mpsc, watch};
use tokio::task::JoinSet;

use crate::app::AppEvent;
use crate::runner::LoopEvent;

use super::config::{Config, Missing, SttBackend};
use super::process::{CleanupFailure, Control};
use super::{CheckFailure, Effect, Reply, backend};

#[derive(Debug, Default)]
pub(crate) struct Service {
    controls: HashMap<u64, watch::Sender<Control>>,
    tasks: JoinSet<Result<(), String>>,
}

fn reply(tx: &mpsc::UnboundedSender<LoopEvent>, reply: Reply) {
    // A closed UI channel means shutdown; the owning task has already cleaned up.
    let _ = tx.send(LoopEvent::App(AppEvent::Voice(reply)));
}

async fn load(dir: &Path) -> anyhow::Result<Config> {
    let dir = dir.to_owned();
    tokio::task::spawn_blocking(move || Config::load(&dir)).await?
}

async fn load_needing(dir: &Path, missing: Missing) -> anyhow::Result<Config> {
    let dir = dir.to_owned();
    tokio::task::spawn_blocking(move || Config::load_needing(&dir, missing)).await?
}

impl Service {
    pub fn dispatch(&mut self, effect: Effect, tx: mpsc::UnboundedSender<LoopEvent>) {
        while let Some(result) = self.tasks.try_join_next() {
            let error = match result {
                Ok(Ok(())) => None,
                Ok(Err(error)) => Some(error),
                Err(error) => Some(error.to_string()),
            };
            if let Some(error) = error {
                let _ = tx.send(LoopEvent::App(AppEvent::EffectFailed {
                    action: "local voice worker".into(),
                    error,
                }));
            }
        }
        match effect {
            Effect::Stop(id) | Effect::Cancel(id) => {
                if let Some(control) = self.controls.get(&id) {
                    control.send_replace(if matches!(effect, Effect::Stop(_)) {
                        Control::Stop
                    } else {
                        Control::Cancel
                    });
                }
            }
            effect => {
                let id = match &effect {
                    Effect::CheckReplies { id, .. }
                    | Effect::RecordReply { id, .. }
                    | Effect::Check { id, .. }
                    | Effect::Record { id, .. }
                    | Effect::Synthesize { id, .. }
                    | Effect::Play { id, .. }
                    | Effect::Setup { id, .. } => *id,
                    _ => unreachable!("control effects handled above"),
                };
                self.controls
                    .retain(|_, sender| sender.receiver_count() > 0);
                let (control, receiver) = watch::channel(Control::Run);
                self.controls.insert(id, control);
                self.tasks.spawn(execute(effect, receiver, tx));
            }
        }
    }

    pub fn cancel_all(&self) {
        for control in self.controls.values() {
            control.send_replace(Control::Cancel);
        }
    }

    pub async fn shutdown(&mut self) -> anyhow::Result<()> {
        self.cancel_all();
        let mut failure = None;
        while let Some(result) = self.tasks.join_next().await {
            match result {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    failure.get_or_insert(anyhow::anyhow!(error));
                }
                Err(error) => {
                    failure.get_or_insert(error.into());
                }
            }
        }
        if let Some(error) = failure {
            return Err(error);
        }
        Ok(())
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        self.cancel_all();
    }
}

async fn execute(
    effect: Effect,
    mut control: watch::Receiver<Control>,
    tx: mpsc::UnboundedSender<LoopEvent>,
) -> Result<(), String> {
    let mut cleanup_failure = None;
    let explicit_stop = matches!(effect, Effect::RecordReply { .. });
    let result = match effect {
        Effect::CheckReplies { id, dir } => {
            let result = async {
                tokio::task::spawn_blocking(move || Config::load_replies(&dir)).await??;
                Ok::<_, anyhow::Error>(
                    "STT and TTS configuration checked; devices and models were not run".to_owned(),
                )
            }
            .await;
            Reply::Checked {
                id,
                result: check_result(result),
            }
        }
        Effect::Check { id, dir, speech } => {
            let result = async {
                if speech {
                    load_needing(&dir, Missing::Speech).await?.speech()?;
                } else {
                    let config = load(&dir).await?;
                    // Parakeet is configured via stt_backend, not whisper paths.
                    let has_recording = config.stt_backend == SttBackend::Parakeet
                        || config.whisper.is_some()
                        || config.whisper_model.is_some();
                    if has_recording {
                        config.recording()?;
                    }
                    if config.piper.is_some() || config.piper_model.is_some() {
                        config.speech()?;
                    }
                    if !has_recording && config.piper.is_none() {
                        anyhow::bail!(
                            "no voice backend is installed: press Enter on an install row in /voice, or configure whisper/Parakeet and/or piper in voice.toml yourself (docs/voice.md)"
                        );
                    }
                }
                Ok::<_, anyhow::Error>(
                    "local paths/configuration checked; devices and models were not run".to_owned(),
                )
            }
            .await;
            Reply::Checked {
                id,
                result: check_result(result),
            }
        }
        Effect::Record { id, dir } | Effect::RecordReply { id, dir } => {
            let result = async {
                let config = load(&dir).await?;
                let file = if explicit_stop {
                    let progress = tx.clone();
                    backend::record_reply(&config, &mut control, move || {
                        reply(&progress, Reply::Recording(id));
                    })
                    .await?
                } else {
                    backend::record(&config, &mut control).await?
                };
                reply(&tx, Reply::Transcribing(id));
                let result = backend::transcribe(&config, file.path(), &mut control).await;
                let cleanup = file.close();
                match (result, cleanup) {
                    (Err(cause), Err(cleanup)) => {
                        Err(cause.context(CleanupFailure(cleanup.into())))
                    }
                    (Err(cause), Ok(())) => Err(cause),
                    (Ok(_), Err(cleanup)) => Err(CleanupFailure(cleanup.into()).into()),
                    (Ok(text), Ok(())) => Ok(text),
                }
            }
            .await;
            Reply::Recorded {
                id,
                result: display_result(result, &mut cleanup_failure),
            }
        }
        Effect::Synthesize { id, dir, text } => {
            let result = async {
                let config = load(&dir).await?;
                let (pcm, rate) = backend::synthesize(&config, &text, &mut control).await?;
                Ok::<_, anyhow::Error>((config, pcm, rate))
            }
            .await;
            Reply::Synthesized {
                id,
                result: display_result(result, &mut cleanup_failure),
            }
        }
        Effect::Play {
            id,
            config,
            pcm,
            rate,
        } => {
            let result = backend::play(&config, &pcm, rate, &mut control).await;
            Reply::Played {
                id,
                result: display_result(result, &mut cleanup_failure),
            }
        }
        Effect::Setup { id, dir, model } => {
            let result = super::setup::install(dir, model, &mut control, |stage| {
                reply(&tx, Reply::SetupProgress { id, stage });
            })
            .await;
            Reply::SetupDone {
                id,
                result: display_result(result, &mut cleanup_failure),
            }
        }
        Effect::Stop(_) | Effect::Cancel(_) => return Ok(()),
    };
    reply(&tx, result);
    cleanup_failure.map_or(Ok(()), Err)
}

/// A check runs no process, so it has no cleanup to fail; what it can carry
/// is the installable backend the configuration lacks.
fn check_result(result: anyhow::Result<String>) -> Result<String, CheckFailure> {
    result.map_err(|error| {
        assert!(
            error.downcast_ref::<CleanupFailure>().is_none(),
            "a configuration check starts nothing to clean up"
        );
        let missing = error
            .chain()
            .find_map(|cause| cause.downcast_ref::<Missing>().copied());
        CheckFailure {
            message: format!("{error:#}"),
            missing,
        }
    })
}

fn display_result<T>(
    result: anyhow::Result<T>,
    cleanup_failure: &mut Option<String>,
) -> Result<T, String> {
    result.map_err(|error| {
        let text = format!("{error:#}");
        if error.downcast_ref::<CleanupFailure>().is_some() {
            *cleanup_failure = Some(text.clone());
        }
        text
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod reply_check_tests {
    use super::*;

    #[tokio::test]
    async fn replies_configuration_requires_both_stt_and_tts_without_running_either() {
        let dir = tempfile::tempdir().unwrap();
        let model = dir.path().join("model");
        std::fs::write(&model, b"model fixture").unwrap();
        std::fs::write(
            dir.path().join("model.json"),
            r#"{"audio":{"sample_rate":16000}}"#,
        )
        .unwrap();
        let ffmpeg = "ffmpeg = '/bin/sh'\npulse_socket = '/unused/pulse'\n";
        let stt = format!(
            "whisper = '/bin/sh'\nwhisper_model = '{}'\n",
            model.display()
        );
        let tts = format!("piper = '/bin/sh'\npiper_model = '{}'\n", model.display());
        for (id, contents, valid) in [
            (1, format!("{ffmpeg}{stt}"), false),
            (2, format!("{ffmpeg}{tts}"), false),
            (3, format!("{ffmpeg}{stt}{tts}"), true),
        ] {
            std::fs::write(dir.path().join("voice.toml"), contents).unwrap();
            let (_control, receiver) = watch::channel(Control::Run);
            let (tx, mut rx) = mpsc::unbounded_channel();
            execute(
                Effect::CheckReplies {
                    id,
                    dir: dir.path().to_owned(),
                },
                receiver,
                tx,
            )
            .await
            .unwrap();
            let Some(LoopEvent::App(AppEvent::Voice(Reply::Checked { id: actual, result }))) =
                rx.recv().await
            else {
                panic!("expected configuration result");
            };
            assert_eq!(actual, id);
            assert_eq!(result.is_ok(), valid, "{result:?}");
        }
    }

    /// Run one configuration check against `dir` and return what it found.
    async fn checked(dir: &Path, replies: bool) -> Result<String, CheckFailure> {
        let dir = dir.to_owned();
        let effect = if replies {
            Effect::CheckReplies { id: 7, dir }
        } else {
            let speech = true;
            Effect::Check { id: 7, dir, speech }
        };
        let (_control, receiver) = watch::channel(Control::Run);
        let (tx, mut rx) = mpsc::unbounded_channel();
        execute(effect, receiver, tx).await.unwrap();
        let Some(LoopEvent::App(AppEvent::Voice(Reply::Checked { id: 7, result }))) =
            rx.recv().await
        else {
            panic!("expected the configuration result of check 7");
        };
        result
    }

    #[tokio::test]
    async fn an_unset_backend_is_named_as_missing_and_a_broken_path_keeps_its_error() {
        let dir = tempfile::tempdir().unwrap();
        let write = |body: &str| {
            let base = "ffmpeg = '/bin/sh'\npulse_socket = '/unused/pulse'\n";
            std::fs::write(dir.path().join("voice.toml"), format!("{base}{body}")).unwrap();
        };
        let model = dir.path().join("model");
        std::fs::write(&model, b"model fixture").unwrap();
        let stt = format!(
            "whisper = '/bin/sh'\nwhisper_model = '{}'\n",
            model.display()
        );
        let whisper = Missing::Recognition(SttBackend::WhisperCpp);
        let parakeet = Missing::Recognition(SttBackend::Parakeet);

        // No voice.toml at all: nothing was ever installed.
        for (replies, missing) in [(false, Missing::Speech), (true, whisper)] {
            let failure = checked(dir.path(), replies).await.unwrap_err();
            assert_eq!(failure.missing, Some(missing));
            if replies {
                assert!(
                    failure
                        .message
                        .contains("speech output (Piper) is not installed")
                );
                assert!(
                    failure
                        .message
                        .contains("speech recognition is not installed")
                );
            }
        }
        for (replies, body, missing) in [
            (false, stt.as_str(), Missing::Speech),
            (true, stt.as_str(), Missing::Speech),
            (true, "piper = '/bin/sh'\n", whisper),
            (true, "stt_backend = 'parakeet'\n", parakeet),
        ] {
            write(body);
            let failure = checked(dir.path(), replies).await.unwrap_err();
            assert_eq!(failure.missing, Some(missing), "{failure}");
            assert!(failure.message.contains("is not installed"), "{failure}");
            assert!(failure.message.contains("row in /voice"), "{failure}");
        }
        // Set but broken: the installer is not the answer, the path is.
        let gone = dir.path().join("gone");
        for (replies, body, name) in [
            (false, format!("piper = '{}'\n", gone.display()), "piper"),
            (false, "piper = '/bin/sh'\n".to_owned(), "piper_model"),
            (true, format!("whisper = '{}'\n", gone.display()), "whisper"),
            (true, "whisper = '/bin/sh'\n".to_owned(), "whisper_model"),
        ] {
            write(&body);
            let failure = checked(dir.path(), replies).await.unwrap_err();
            assert_eq!(failure.missing, None, "{failure}");
            assert!(failure.message.contains(name), "{failure}");
            assert!(!failure.message.contains("not installed"), "{failure}");
        }
    }
}
