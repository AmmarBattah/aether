use super::*;

#[test]
fn reply_diagnostics_distinguish_eligibility_and_preserve_preferences() {
    let (mut app, dir) = app_with_prefs();
    assert!(reply_status(&mut app).starts_with("disabled:"));
    enable_replies(&mut app);
    let saved = on_disk(&dir);
    assert!(reply_status(&mut app).starts_with("no_pending_question:"));
    assert!(
        reply_toggle(&mut app)
            .unwrap_err()
            .starts_with("no_pending_question:")
    );
    question(&mut app, 1);
    assert!(reply_status(&mut app).starts_with("synthesizing:"));
    assert!(
        reply_toggle(&mut app)
            .unwrap_err()
            .starts_with("synthesizing:")
    );
    let id = app.voice.speech.as_ref().unwrap().id;
    app.reduce(AppEvent::Voice(Reply::Synthesized {
        id,
        result: Ok((
            toml::from_str("ffmpeg = '/unused'").unwrap(),
            vec![0; 32],
            16000,
        )),
    }));
    assert!(reply_status(&mut app).starts_with("playback_pending:"));
    app.reduce(AppEvent::Voice(Reply::Played { id, result: Ok(()) }));
    assert!(reply_status(&mut app).starts_with("ready:"));
    app.tick += 120 * crate::app::TICKS_PER_SECOND;
    assert!(reply_status(&mut app).starts_with("expired:"));
    app.voice_reconcile();
    assert!(app.voice.replies.armed.is_none());
    assert!(
        reply_status(&mut app).starts_with("expired:"),
        "expiry survives reconciliation"
    );
    assert!(reply_toggle(&mut app).unwrap_err().starts_with("expired:"));
    assert!(app.voice_action(Action::Status).is_empty());
    assert!(app.live_flash().unwrap().contains("expired:"));
    assert!(
        app.live_flash()
            .unwrap()
            .contains("next launch: questions on, replies on")
    );
    assert_eq!(on_disk(&dir), saved);
    question(&mut app, 2);
    assert!(
        reply_status(&mut app).starts_with("synthesizing:"),
        "new target must not inherit expiry"
    );
}

#[test]
fn reply_diagnostics_retain_speech_failures_without_private_error_text() {
    for playback in [false, true] {
        let mut app = app();
        enable_replies(&mut app);
        question(&mut app, 1);
        let id = app.voice.speech.as_ref().unwrap().id;
        let result = Err("private backend detail".into());
        let event = if playback {
            Reply::Played { id, result }
        } else {
            Reply::Synthesized {
                id,
                result: Err("private backend detail".into()),
            }
        };
        app.reduce(AppEvent::Voice(event));
        let status = reply_status(&mut app);
        assert!(
            status.starts_with(if playback {
                "playback_failed:"
            } else {
                "speech_failed:"
            }),
            "{status}"
        );
        assert!(!status.contains("private"));
        assert_eq!(reply_toggle(&mut app).unwrap_err(), status);
        question(&mut app, 2);
        assert!(reply_status(&mut app).starts_with("synthesizing:"));
    }
}

#[test]
fn reply_diagnostics_follow_confirmed_lifecycle_and_keep_sent_outcome() {
    use crate::voice::control::ControlAction;
    let mut app = app();
    enable_replies(&mut app);
    question(&mut app, 1);
    finish_speech(&mut app);
    let id = start_reply(&mut app);
    assert!(reply_status(&mut app).starts_with("starting:"));
    assert!(
        app.voice_control_ack(ControlAction::Reply)
            .contains("microphone not yet confirmed")
    );
    app.voice_reply(Reply::Recording(id + 1));
    assert!(reply_status(&mut app).starts_with("starting:"));
    app.voice_reply(Reply::Recording(id));
    assert!(reply_status(&mut app).starts_with("recording:"));
    app.tick += crate::app::TICKS_PER_SECOND;
    assert_eq!(
        reply_toggle(&mut app).unwrap(),
        vec![Effect::Voice(VoiceEffect::Stop(id))]
    );
    assert!(reply_status(&mut app).starts_with("stopping:"));
    app.voice_reply(Reply::Recording(id));
    assert!(
        reply_status(&mut app).starts_with("stopping:"),
        "late audio cannot rewind stop"
    );
    app.tick += crate::app::TICKS_PER_SECOND;
    assert!(reply_toggle(&mut app).unwrap_err().starts_with("stopping:"));
    app.voice_reply(Reply::Transcribing(id));
    assert!(reply_status(&mut app).starts_with("transcribing:"));
    assert!(
        reply_toggle(&mut app)
            .unwrap_err()
            .starts_with("transcribing:")
    );
    let effects = app.voice_reply(Reply::Recorded {
        id,
        result: Ok("private answer".into()),
    });
    assert_eq!(effects.len(), 1);
    assert!(reply_status(&mut app).starts_with("sending:"));
    assert!(reply_toggle(&mut app).unwrap_err().starts_with("sending:"));
    app.voice_send_result(
        id,
        Ok(aether_protocol::Reply::MessageSent {
            message: aether_core::id::MessageId::new(),
        }),
    );
    app.flash("unrelated notice");
    app.tick += 11 * crate::app::TICKS_PER_SECOND;
    let status = reply_status(&mut app);
    assert!(status.contains("last=sent (11s ago)"), "{status}");
    assert!(!status.contains("private"));
    assert!(app.voice.replies.active.is_none());
    assert!(reply_toggle(&mut app).is_err());
    question(&mut app, 2);
    assert!(
        reply_status(&mut app).contains("last=sent"),
        "latest outcome remains explicitly separate from new eligibility"
    );
}

#[test]
fn reply_diagnostics_retain_failed_cancelled_and_unconfirmed_outcomes() {
    use crate::voice::control::ControlAction;
    for outcome in [
        "capture_failed",
        "transcription_failed",
        "cancelled",
        "delivery_unconfirmed_check_TUI",
    ] {
        let mut app = app();
        enable_replies(&mut app);
        question(&mut app, 1);
        finish_speech(&mut app);
        let id = start_reply(&mut app);
        if outcome == "cancelled" {
            assert_eq!(
                app.handle_voice_control(ControlAction::Cancel).unwrap(),
                vec![Effect::Voice(VoiceEffect::Cancel(id))]
            );
            assert!(reply_status(&mut app).starts_with("cancelling:"));
            assert!(
                app.handle_voice_control(ControlAction::Cancel)
                    .unwrap()
                    .is_empty()
            );
            app.tick += crate::app::TICKS_PER_SECOND;
            assert!(
                reply_toggle(&mut app)
                    .unwrap_err()
                    .starts_with("cancelling:")
            );
        } else if outcome != "capture_failed" {
            app.tick += crate::app::TICKS_PER_SECOND;
            reply_toggle(&mut app).unwrap();
            app.voice_reply(Reply::Transcribing(id));
        }
        if outcome == "delivery_unconfirmed_check_TUI" {
            app.voice_reply(Reply::Recorded {
                id,
                result: Ok("private answer".into()),
            });
            app.voice_send_result(id, Err(aether_client::ClientError::Disconnected));
        } else {
            app.voice_reply(Reply::Recorded {
                id,
                result: Err(if outcome == "cancelled" {
                    "cancelled"
                } else {
                    "private backend detail"
                }
                .into()),
            });
        }
        app.flash("unrelated notice");
        let status = reply_status(&mut app);
        assert!(status.contains(&format!("last={outcome} ")), "{status}");
        assert!(!status.contains("private"));
        app.voice_reply(Reply::Recording(id));
        app.voice_reply(Reply::Recorded {
            id,
            result: Ok("late answer".into()),
        });
        assert_eq!(
            reply_status(&mut app),
            status,
            "late events cannot revive capture"
        );
    }
}

#[test]
fn reply_status_does_not_toggle_or_debounce_and_distinguishes_f4_busy() {
    let mut app = app();
    enable_replies(&mut app);
    question(&mut app, 1);
    finish_speech(&mut app);
    for _ in 0..3 {
        assert!(reply_status(&mut app).starts_with("ready:"));
    }
    let id = start_reply(&mut app);
    app.tick += crate::app::TICKS_PER_SECOND;
    for _ in 0..3 {
        assert!(reply_status(&mut app).starts_with("starting:"));
    }
    assert_eq!(
        reply_toggle(&mut app).unwrap(),
        vec![Effect::Voice(VoiceEffect::Stop(id))]
    );
    app.voice_reply(Reply::Recorded {
        id,
        result: Err("fixture failure".into()),
    });
    app.voice_action(Action::Start);
    assert!(reply_status(&mut app).starts_with("busy_dictation:"));
}

#[test]
fn reply_diagnostics_target_change_cancels_without_retargeting_and_retains_reason() {
    let mut app = app();
    enable_replies(&mut app);
    question(&mut app, 1);
    finish_speech(&mut app);
    let id = start_reply(&mut app);
    let effects = question(&mut app, 2);
    assert!(effects.contains(&Effect::Voice(VoiceEffect::Cancel(id))));
    assert!(reply_status(&mut app).starts_with("cancelling: captured question changed"));
    assert!(
        app.voice_reply(Reply::Recorded {
            id,
            result: Ok("stale private answer".into())
        })
        .is_empty()
    );
    let status = reply_status(&mut app);
    assert!(
        status.starts_with("not_spoken:"),
        "new question was not spoken during capture: {status}"
    );
    assert!(status.contains("last=target_changed"));
    assert!(!status.contains("private"));
    assert!(reply_toggle(&mut app).is_err());
}

#[test]
fn cancelled_playback_reports_cancellation_not_device_failure() {
    let mut app = app();
    enable_replies(&mut app);
    question(&mut app, 1);
    let id = app.voice.speech.as_ref().unwrap().id;
    assert_eq!(
        app.cancel_speech(),
        vec![Effect::Voice(VoiceEffect::Cancel(id))]
    );
    app.voice_reply(Reply::Played {
        id,
        result: Err("cancelled".into()),
    });
    assert!(reply_status(&mut app).starts_with("speech_cancelled:"));
    assert!(reply_toggle(&mut app).is_err());
}
