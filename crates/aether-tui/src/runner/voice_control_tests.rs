#![allow(clippy::expect_used, clippy::unwrap_used)]

use super::*;
use crate::voice::{Action, Effect as VoiceEffect, Reply as VoiceReply};

fn enabled_app(socket: Option<std::path::PathBuf>) -> App {
    let mut app = crate::testing::app_with_transcript(&[]);
    app.voice.available = true;
    // A path that can never be a directory, so no test writes preferences.
    app.options.prefs_dir = Some("/dev/null/unused-voice-fixture".into());
    app.options.socket_path = socket;
    let effects = app.voice_action(Action::RepliesOn);
    let id = effects
        .iter()
        .find_map(|effect| match effect {
            Effect::Voice(VoiceEffect::CheckReplies { id, .. }) => Some(*id),
            _ => None,
        })
        .expect("explicit configuration check");
    app.voice_reply(VoiceReply::Checked {
        id,
        result: Ok("fixture checked".into()),
    });
    assert!(app.voice_replies_enabled());
    app
}

#[tokio::test]
async fn no_listener_without_opt_in_and_no_guessed_daemon_socket() {
    let mut app = crate::testing::app_with_transcript(&[]);
    let mut listener = None;
    assert!(
        reconcile_voice_control(&mut app, &mut listener)
            .await
            .is_empty()
    );
    assert!(listener.is_none());

    let mut app = enabled_app(None);
    reconcile_voice_control(&mut app, &mut listener).await;
    assert!(!app.voice_replies_enabled());
    assert!(listener.is_none());
    assert!(app.live_flash().unwrap().contains("no local daemon socket"));
    assert!(
        app.voice.enabled,
        "listener failure need not mute ordinary question speech"
    );
}

#[tokio::test]
#[cfg(unix)]
async fn suspend_resume_restores_reply_control_only_when_previously_enabled() {
    for enabled in [true, false] {
        let dir = tempfile::tempdir().expect("private fixture");
        let mut app = enabled_app(Some(dir.path().join("a.sock")));
        app.options.prefs_dir = Some(dir.path().to_path_buf());
        if !enabled {
            app.voice_action(Action::RepliesOff);
        }
        let saved = crate::prefs::UiPrefs::load(dir.path()).expect("saved preferences");
        let mut listener = None;
        reconcile_voice_control(&mut app, &mut listener).await;
        assert_eq!(listener.is_some(), enabled);

        let replies_enabled = app.voice_replies_enabled();
        listener = None;
        app.voice_suspend();
        assert!(!app.voice_replies_enabled());
        app.voice_resume(replies_enabled);
        let effects = reconcile_voice_control(&mut app, &mut listener).await;

        assert_eq!(app.voice_replies_enabled(), enabled);
        assert_eq!(listener.is_some(), enabled);
        assert!(effects.is_empty(), "resume must not start recording");
        assert_eq!(crate::prefs::UiPrefs::load(dir.path()).unwrap(), saved);
    }
}

#[tokio::test]
async fn listener_failure_disarms_replies_without_disabling_f4() {
    let mut app = enabled_app(None);
    let mut listener = None;
    let effects = handle_voice_control(
        &mut app,
        Err(anyhow::anyhow!("fixture listener failed")),
        &mut listener,
    );
    assert!(effects.is_empty());
    assert!(!app.voice_replies_enabled());
    assert!(
        app.live_flash()
            .unwrap()
            .contains("fixture listener failed")
    );
    let effects = app.voice_action(Action::Start);
    assert!(
        effects
            .iter()
            .any(|effect| matches!(effect, Effect::Voice(VoiceEffect::Record { .. })))
    );
}

#[tokio::test]
async fn a_listener_failure_leaves_the_saved_voice_preference_alone() {
    let dir = tempfile::tempdir().expect("private fixture");
    let saved = crate::prefs::VoicePrefs {
        questions: true,
        replies: true,
    };
    let mut prefs = crate::prefs::UiPrefs::default();
    prefs.set_voice(saved);
    prefs.save(dir.path()).expect("a writable fixture");

    let mut app = enabled_app(None);
    app.options.prefs_dir = Some(dir.path().to_path_buf());
    let mut listener = None;
    handle_voice_control(
        &mut app,
        Err(anyhow::anyhow!("fixture listener failed")),
        &mut listener,
    );
    assert!(!app.voice_replies_enabled());
    assert!(app.voice.enabled, "question speech is a separate setting");
    assert_eq!(
        crate::prefs::UiPrefs::load(dir.path())
            .expect("saved preferences stay readable")
            .voice(),
        saved,
        "an environment failure is not the user opting out"
    );
}

#[tokio::test]
async fn global_request_reaches_reducer_and_reports_no_question_without_recording() {
    let dir = tempfile::tempdir().expect("private fixture");
    let daemon_socket = dir.path().join("a.sock");
    let socket = control::control_socket_path(&daemon_socket);
    let mut app = enabled_app(Some(daemon_socket));
    let mut listener = None;
    assert!(
        reconcile_voice_control(&mut app, &mut listener)
            .await
            .is_empty()
    );
    assert!(listener.is_some());
    let client = tokio::spawn(async move {
        control::send_request(
            &socket,
            control::ControlAction::Reply,
            Duration::from_secs(2),
            Duration::from_secs(2),
        )
        .await
    });
    let request = next_voice_control(&mut listener).await;
    let effects = handle_voice_control(&mut app, request, &mut listener);
    assert!(
        effects.is_empty(),
        "no question must never open the microphone"
    );
    let (ok, message) = client.await.unwrap().unwrap();
    assert!(!ok);
    assert!(message.contains("no_pending_question"));
    assert!(app.input.text.is_empty());
    app.voice_action(Action::RepliesOff);
    reconcile_voice_control(&mut app, &mut listener).await;
    assert!(listener.is_none());
}

#[tokio::test]
async fn second_tui_cannot_steal_reply_control() {
    let dir = tempfile::tempdir().expect("private fixture");
    let socket = dir.path().join("a.sock");
    let mut first = enabled_app(Some(socket.clone()));
    let mut second = enabled_app(Some(socket));
    let mut first_listener = None;
    let mut second_listener = None;
    reconcile_voice_control(&mut first, &mut first_listener).await;
    assert!(first_listener.is_some());
    reconcile_voice_control(&mut second, &mut second_listener).await;
    assert!(second_listener.is_none());
    assert!(!second.voice_replies_enabled());
    assert!(first.voice_replies_enabled());
    assert!(second.live_flash().unwrap().contains("another TUI"));
}

#[tokio::test]
async fn status_round_trip_is_read_only_and_cancel_ack_is_not_delivery_success() {
    let dir = tempfile::tempdir().expect("private fixture");
    let daemon_socket = dir.path().join("a.sock");
    let socket = control::control_socket_path(&daemon_socket);
    let mut app = enabled_app(Some(daemon_socket));
    let mut listener = None;
    reconcile_voice_control(&mut app, &mut listener).await;
    for action in [
        control::ControlAction::Status,
        control::ControlAction::Cancel,
    ] {
        let socket = socket.clone();
        let client = tokio::spawn(async move {
            control::send_request(
                &socket,
                action,
                Duration::from_secs(2),
                Duration::from_secs(2),
            )
            .await
        });
        let request = next_voice_control(&mut listener).await;
        app.flash("fixture notice");
        let before = format!("{:?}", app.voice);
        let effects = handle_voice_control(&mut app, request, &mut listener);
        let (ok, message) = client.await.unwrap().unwrap();
        assert!(ok);
        assert!(effects.is_empty());
        assert_eq!(format!("{:?}", app.voice), before);
        if action == control::ControlAction::Status {
            assert!(message.starts_with("no_pending_question:"));
            assert_eq!(app.live_flash(), Some("fixture notice"));
        } else {
            assert!(message.starts_with("idle: no active recording"));
        }
    }
}
