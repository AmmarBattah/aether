use super::*;

#[test]
fn global_replies_require_opt_in_checked_and_successfully_spoken_pending_question() {
    let mut app = app();
    assert!(reply_toggle(&mut app).is_err());
    enable_replies(&mut app);
    assert!(reply_toggle(&mut app).is_err());
    question(&mut app, 1);
    assert!(reply_toggle(&mut app).is_err(), "synthesis is not speech");
    let id = app.voice.speech.as_ref().unwrap().id;
    app.reduce(AppEvent::Voice(Reply::Played {
        id,
        result: Err("speaker failed".into()),
    }));
    assert!(reply_toggle(&mut app).is_err());
    question(&mut app, 2);
    finish_speech(&mut app);
    assert!(app.voice.replies.armed.is_some());
    app.voice_action(Action::RepliesOff);
    assert!(reply_toggle(&mut app).is_err());
}

#[test]
fn global_replies_two_toggles_send_exact_spoken_target_once_without_touching_composer() {
    let mut app = app();
    let agent = app.selected.unwrap();
    enable_replies(&mut app);
    question(&mut app, 1);
    finish_speech(&mut app);
    app.select(AgentId::new());
    app.input.set_text("untouched draft");
    let revision = app.input.revision;
    let id = start_reply(&mut app);
    assert!(reply_toggle(&mut app).is_err(), "debounce key repeats");
    assert!(
        key(&mut app, KeyCode::F(4)).is_empty(),
        "F4 must not stop or open a second capture"
    );
    app.tick += crate::app::TICKS_PER_SECOND;
    assert_eq!(
        reply_toggle(&mut app).unwrap(),
        vec![Effect::Voice(VoiceEffect::Stop(id))]
    );
    assert!(reply_toggle(&mut app).is_err());
    let effects = app.reduce(AppEvent::Voice(Reply::Recorded {
        id,
        result: Ok("spoken answer".into()),
    }));
    assert!(matches!(effects.as_slice(), [Effect::Request {
        command: aether_protocol::Command::ReplyToQuestion { to, question_seq: 1, content, idempotency_key: Some(key) },
        then: crate::app::Then::VoiceReply(request_id),
    }] if *to == agent && *request_id == id && content == "spoken answer" && !key.is_empty()));
    assert_eq!(app.input.text, "untouched draft");
    assert_eq!(app.input.revision, revision);
    assert!(app.voice.recovered.is_none());
    assert!(app.live_flash().unwrap().contains("sending"));
    app.reduce(AppEvent::Voice(Reply::Transcribing(id)));
    assert!(
        app.reduce(AppEvent::Voice(Reply::Recorded {
            id,
            result: Ok("duplicate".into())
        }))
        .is_empty()
    );
    app.reduce(AppEvent::Reply {
        then: crate::app::Then::VoiceReply(id),
        result: Ok(aether_protocol::Reply::MessageSent {
            message: aether_core::id::MessageId::new(),
        }),
    });
    assert!(app.live_flash().unwrap().contains("voice reply sent"));
    app.tick += crate::app::TICKS_PER_SECOND;
    assert!(
        reply_toggle(&mut app).is_err(),
        "sent question is never rearmed"
    );
}

#[test]
fn global_reply_timeout_or_unsolicited_transcription_never_authorizes_send() {
    for transcribing in [false, true] {
        let mut app = app();
        enable_replies(&mut app);
        question(&mut app, 1);
        finish_speech(&mut app);
        let id = start_reply(&mut app);
        if transcribing {
            app.reduce(AppEvent::Voice(Reply::Transcribing(id)));
            app.tick += crate::app::TICKS_PER_SECOND;
            assert!(
                reply_toggle(&mut app).is_err(),
                "late shortcut must not authorize an auto-stop"
            );
        }
        assert!(
            app.reduce(AppEvent::Voice(Reply::Recorded {
                id,
                result: Ok("automatic stop".into())
            }))
            .is_empty()
        );
        assert!(app.input.text.is_empty());
        assert!(app.voice.recovered.is_none());
    }
}

#[test]
fn global_reply_cancel_off_mute_disconnect_suspend_quit_reject_late_audio() {
    for mode in 0..7 {
        let mut app = app();
        enable_replies(&mut app);
        question(&mut app, 1);
        finish_speech(&mut app);
        let id = start_reply(&mut app);
        app.tick += crate::app::TICKS_PER_SECOND;
        reply_toggle(&mut app).unwrap();
        let effects = match mode {
            0 => app
                .handle_voice_control(crate::voice::control::ControlAction::Cancel)
                .unwrap(),
            1 => app.voice_action(Action::RepliesOff),
            2 => app.voice_action(Action::Mute),
            3 => app.reduce(AppEvent::Disconnected { because: None }),
            4 => app.voice_suspend(),
            5 => {
                app.quit_requested = true;
                app.voice_reconcile()
            }
            _ => app.voice_action(Action::Off),
        };
        assert!(
            effects.contains(&Effect::Voice(VoiceEffect::Cancel(id))),
            "mode {mode}"
        );
        if mode != 0 {
            assert!(!app.voice_replies_enabled());
        }
        assert!(
            app.reduce(AppEvent::Voice(Reply::Recorded {
                id,
                result: Ok("late answer".into())
            }))
            .is_empty()
        );
        assert!(app.input.text.is_empty());
    }
}

#[test]
#[cfg(unix)]
fn resume_keeps_cancelled_reply_audio_and_old_question_disarmed() {
    let mut app = app();
    enable_replies(&mut app);
    question(&mut app, 1);
    finish_speech(&mut app);
    let id = start_reply(&mut app);
    let replies_enabled = app.voice_replies_enabled();
    let effects = app.voice_suspend();
    assert!(effects.contains(&Effect::Voice(VoiceEffect::Cancel(id))));
    app.voice_resume(replies_enabled);

    assert!(app.voice_replies_enabled());
    assert!(app.voice.replies.armed.is_none());
    assert!(app.voice_reply_diagnostic().contains("cancelling"));
    assert!(reply_toggle(&mut app).is_err());
    assert!(
        app.reduce(AppEvent::Voice(Reply::Recorded {
            id,
            result: Ok("late answer after fg".into()),
        }))
        .is_empty()
    );
    assert!(app.voice.replies.active.is_none());
    assert!(app.input.text.is_empty());
    question(&mut app, 2);
    finish_speech(&mut app);
    assert!(app.voice.replies.armed.is_some(), "new questions still arm");
}

#[test]
fn global_reply_stale_answered_newer_terminal_questions_cancel_in_flight() {
    for mode in 0..3 {
        let mut app = app();
        enable_replies(&mut app);
        question(&mut app, 1);
        finish_speech(&mut app);
        let id = start_reply(&mut app);
        app.tick += crate::app::TICKS_PER_SECOND;
        reply_toggle(&mut app).unwrap();
        let agent = app.selected.unwrap();
        let effects = match mode {
            0 => {
                answer_questions(&mut app, agent);
                app.voice_reconcile()
            }
            1 => question(&mut app, 2),
            _ => event(
                &mut app,
                agent,
                2,
                EventKind::AgentArchived { archived: true },
            ),
        };
        assert!(effects.contains(&Effect::Voice(VoiceEffect::Cancel(id))));
        assert!(
            app.reduce(AppEvent::Voice(Reply::Recorded {
                id,
                result: Ok("stale".into())
            }))
            .is_empty()
        );
    }
}

#[test]
fn next_agents_speech_disarms_previous_target_and_arm_expiry_only_limits_start() {
    let mut app = app();
    enable_replies(&mut app);
    question(&mut app, 1);
    finish_speech(&mut app);
    let other = AgentId::new();
    event(
        &mut app,
        other,
        2,
        EventKind::QuestionAsked {
            question: "Other question?".into(),
        },
    );
    assert!(app.voice.speech.is_some());
    assert!(
        reply_toggle(&mut app).is_err(),
        "cannot reply to A while B is being spoken"
    );
    finish_speech(&mut app);
    app.tick += 119 * crate::app::TICKS_PER_SECOND;
    let id = start_reply(&mut app);
    app.tick += 61 * crate::app::TICKS_PER_SECOND;
    assert!(
        app.voice_reconcile().is_empty(),
        "arm expiry does not invalidate frozen capture"
    );
    assert!(
        reply_toggle(&mut app)
            .unwrap()
            .contains(&Effect::Voice(VoiceEffect::Stop(id)))
    );
    assert!(
        app.reduce(AppEvent::Voice(Reply::Recorded {
            id,
            result: Ok("answer".into())
        }))
        .iter()
        .any(|e| matches!(e, Effect::Request { .. }))
    );
}

#[test]
fn global_reply_check_failure_off_during_check_and_f4_mic_exclusion() {
    for off in [false, true] {
        let mut app = app();
        app.voice_action(Action::RepliesOn);
        let id = app.voice.replies.checking.unwrap();
        if off {
            app.voice_action(Action::RepliesOff);
        }
        app.reduce(AppEvent::Voice(Reply::Checked {
            id,
            result: if off {
                Ok("late check".into())
            } else {
                Err("missing TTS".into())
            },
        }));
        assert!(!app.voice_replies_enabled());
    }
    let mut app = app();
    enable_replies(&mut app);
    question(&mut app, 1);
    finish_speech(&mut app);
    let id = record(&mut app);
    assert!(reply_toggle(&mut app).is_err());
    app.voice_action(Action::RepliesOff);
    assert!(!app.voice.capture.as_ref().unwrap().cancelled);
    app.reduce(AppEvent::Voice(Reply::Recorded {
        id,
        result: Ok("review me".into()),
    }));
    assert_eq!(app.input.text, "review me");
}

#[test]
fn global_reply_failed_send_not_retried_and_reports_original_target() {
    let mut app = app();
    enable_replies(&mut app);
    question(&mut app, 1);
    finish_speech(&mut app);
    let id = start_reply(&mut app);
    app.tick += crate::app::TICKS_PER_SECOND;
    reply_toggle(&mut app).unwrap();
    app.reduce(AppEvent::Voice(Reply::Recorded {
        id,
        result: Ok("answer".into()),
    }));
    assert!(
        app.handle_voice_control(crate::voice::control::ControlAction::Cancel)
            .is_err()
    );
    assert!(
        app.reduce(AppEvent::Reply {
            then: crate::app::Then::VoiceReply(id),
            result: Err(aether_client::ClientError::Disconnected)
        })
        .is_empty()
    );
    assert!(app.live_flash().unwrap().contains("not retried"));
    assert!(app.voice.replies.active.is_none());
    assert!(reply_toggle(&mut app).is_err());
}

#[test]
fn global_reply_continuous_key_repeat_never_stops_recording() {
    let mut app = app();
    enable_replies(&mut app);
    question(&mut app, 1);
    finish_speech(&mut app);
    let id = start_reply(&mut app);
    app.voice_reply(Reply::Recording(id));
    for _ in 0..100 {
        app.tick += 1;
        assert!(reply_toggle(&mut app).is_err());
    }
    assert!(app.voice.status().unwrap().starts_with("recording:"));
    app.tick += crate::app::TICKS_PER_SECOND;
    assert_eq!(
        reply_toggle(&mut app).unwrap(),
        vec![Effect::Voice(VoiceEffect::Stop(id))]
    );
}

#[test]
fn global_reply_start_window_expires_without_opening_microphone() {
    let mut app = app();
    enable_replies(&mut app);
    question(&mut app, 1);
    finish_speech(&mut app);
    app.tick += 120 * crate::app::TICKS_PER_SECOND;
    assert!(reply_toggle(&mut app).is_err());
    assert!(app.voice.replies.active.is_none());
}
