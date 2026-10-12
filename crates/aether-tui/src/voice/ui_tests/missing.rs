//! A backend the installer provides is missing: the notice says so in the
//! popup's own terms and the popup selects its install row.
use super::*;
use crate::voice::SttBackend;

/// The label of the row the open `/voice` popup has selected, as painted.
fn selected_row(app: &App) -> String {
    let frame = render_to(app, 140, 40);
    let rows: Vec<&str> = frame.lines().filter(|line| line.contains('›')).collect();
    let [row] = rows.as_slice() else {
        panic!("expected exactly one selected row: {frame}");
    };
    flat(row)
}

fn open_popup(app: &mut App) {
    assert!(app.voice_action(Action::Setup).is_empty());
    assert_eq!(top_view(app).as_deref(), Some("VoiceView"));
    assert!(selected_row(app).contains("Speak questions"));
}

/// Nothing a failed check leaves behind may install, enable or record.
fn assert_inert(app: &App) {
    assert!(!app.voice.enabled && !app.voice_replies_enabled());
    assert!(app.voice.install.is_none(), "a notice never installs");
    assert!(app.voice.capture.is_none() && app.voice.replies.active.is_none());
}

#[test]
fn question_speech_without_piper_names_it_and_selects_its_install_row() {
    let mut app = app();
    open_popup(&mut app);
    let effects = key(&mut app, KeyCode::Enter);
    assert!(matches!(
        effects.as_slice(),
        [Effect::Voice(VoiceEffect::Check { speech: true, .. })]
    ));
    finish_check(&mut app, &effects, Err(Missing::Speech.into()));

    let notice = app.live_flash().unwrap().to_owned();
    assert!(notice.starts_with("question speech stays off"), "{notice}");
    assert!(notice.contains("speech output (Piper) is not installed"));
    assert!(notice.contains("press Enter on the \"Install piper\" row in /voice"));
    assert!(
        notice.contains("voice.toml"),
        "the manual route stays visible"
    );
    assert!(selected_row(&app).contains("Install piper"));
    assert_inert(&app);

    // The next Enter opens the explicit confirmation; it starts no install.
    assert!(key(&mut app, KeyCode::Enter).is_empty());
    let confirm = format!("{:?}", app.bottom.top_view().unwrap());
    assert!(confirm.starts_with("VoiceConfirmView"), "{confirm}");
    assert!(confirm.contains("model: Piper"), "{confirm}");
    assert_inert(&app);
    assert!(key(&mut app, KeyCode::Esc).is_empty());
    assert!(selected_row(&app).contains("Install piper"));
}

#[test]
fn replies_without_a_backend_select_the_row_that_installs_it() {
    for (missing, said, row, model) in [
        (
            Missing::Speech,
            "speech output (Piper)",
            "Install piper",
            "Piper",
        ),
        (
            Missing::Recognition(SttBackend::Parakeet),
            "speech recognition (Parakeet)",
            "Install parakeet",
            "Parakeet",
        ),
        (
            Missing::Recognition(SttBackend::WhisperCpp),
            "speech recognition is not installed",
            "Install base",
            "Base",
        ),
    ] {
        let mut app = app();
        open_popup(&mut app);
        let effects = key(&mut app, KeyCode::Char('2'));
        assert!(matches!(
            effects.as_slice(),
            [Effect::Voice(VoiceEffect::CheckReplies { .. })]
        ));
        finish_check(&mut app, &effects, Err(missing.into()));

        let notice = app.live_flash().unwrap().to_owned();
        assert!(
            notice.starts_with("global voice replies stay off"),
            "{notice}"
        );
        assert!(notice.contains(said) && notice.contains(row), "{notice}");
        assert!(notice.contains("in /voice"), "{notice}");
        assert!(selected_row(&app).contains(row), "{missing:?}");
        assert!(key(&mut app, KeyCode::Enter).is_empty());
        let confirm = format!("{:?}", app.bottom.top_view().unwrap());
        assert!(confirm.contains(&format!("model: {model}")), "{confirm}");
        assert_inert(&app);
    }
}

#[test]
fn a_set_but_broken_path_keeps_its_precise_error_and_the_selection() {
    let mut app = app();
    open_popup(&mut app);
    let effects = key(&mut app, KeyCode::Enter);
    finish_check(&mut app, &effects, Err("piper: /opt/piper: gone".into()));
    assert_eq!(
        app.live_flash(),
        Some("voice configuration failed: piper: /opt/piper: gone")
    );
    assert!(selected_row(&app).contains("Speak questions"));

    let effects = key(&mut app, KeyCode::Char('2'));
    finish_check(&mut app, &effects, Err("whisper_model: /m: gone".into()));
    assert_eq!(
        app.live_flash(),
        Some("global voice replies configuration failed: whisper_model: /m: gone")
    );
    assert!(selected_row(&app).contains("Voice replies"));
    assert_inert(&app);
}

#[test]
fn the_lead_is_followed_once_and_a_typed_command_opens_nothing() {
    let mut app = app();
    for command in ["/voice on", "/voice replies on"] {
        app.input.set_text(command);
        let effects = key(&mut app, KeyCode::Enter);
        finish_check(&mut app, &effects, Err(Missing::Speech.into()));
        assert!(app.bottom.view_stack.is_empty(), "{command} opens no popup");
        let notice = app.live_flash().unwrap();
        assert!(
            notice.contains("\"Install piper\" row in /voice"),
            "{notice}"
        );
    }
    // An earlier failure was answered by its notice: the popup opens as usual.
    open_popup(&mut app);
    let effects = key(&mut app, KeyCode::Enter);
    finish_check(&mut app, &effects, Err(Missing::Speech.into()));
    assert!(selected_row(&app).contains("Install piper"));

    // Moving away is the person's choice; the same failure does not pull back.
    assert!(key(&mut app, KeyCode::Up).is_empty());
    assert!(selected_row(&app).contains("Install parakeet"));
    assert!(key(&mut app, KeyCode::Home).is_empty());
    assert!(selected_row(&app).contains("Speak questions"));
    assert_inert(&app);
}

#[test]
fn the_reply_shortcut_diagnostic_gives_the_same_guidance_until_a_check_passes() {
    let mut app = app();
    assert!(reply_status(&mut app).contains("enable /voice replies on"));
    for missing in [
        Missing::Speech,
        Missing::Recognition(SttBackend::Parakeet),
        Missing::Recognition(SttBackend::WhisperCpp),
    ] {
        let effects = app.voice_action(Action::RepliesOn);
        finish_check(&mut app, &effects, Err(missing.into()));
        let status = reply_status(&mut app);
        assert!(status.starts_with("disabled: speech"), "{status}");
        assert!(status.contains("is not installed"), "{status}");
        assert!(status.contains("at last check"), "{status}");
        assert!(status.contains("re-check /voice replies on"), "{status}");
        assert!(status.contains("open /voice, press Enter on"), "{status}");
        let refused = reply_toggle(&mut app).unwrap_err();
        assert!(refused.contains("is not installed"), "{refused}");
    }
    let effects = app.voice_action(Action::RepliesOn);
    finish_check(&mut app, &effects, Ok("checked".into()));
    assert!(app.voice_replies_enabled());
    assert!(!reply_status(&mut app).contains("not installed"));
}

#[test]
fn a_finished_install_forgets_what_was_missing_without_enabling_anything() {
    let mut app = app();
    let effects = app.voice_action(Action::On);
    finish_check(&mut app, &effects, Err(Missing::Speech.into()));
    assert!(reply_status(&mut app).contains("not installed"));

    let started = app.voice_install(ModelKind::Piper);
    let [Effect::Voice(VoiceEffect::Setup { id, .. })] = started.as_slice() else {
        panic!("expected one install effect: {started:?}");
    };
    app.reduce(AppEvent::Voice(Reply::SetupDone {
        id: *id,
        result: Ok("installed Piper".into()),
    }));
    assert!(reply_status(&mut app).contains("enable /voice replies on"));
    assert_inert(&app);
}
