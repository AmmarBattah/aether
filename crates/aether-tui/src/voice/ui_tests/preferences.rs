use super::*;

fn start_install(app: &mut App, model: ModelKind) -> u64 {
    let effects = app.voice_install(model);
    let [Effect::Voice(VoiceEffect::Setup { id, .. })] = effects.as_slice() else {
        panic!("expected one install: {effects:?}");
    };
    *id
}

#[test]
fn voice_on_is_remembered_off_clears_it_and_mute_only_lasts_this_run() {
    let (mut app, dir) = app_with_prefs();
    let effects = app.voice_action(Action::On);
    finish_check(&mut app, &effects, Ok("checked".into()));
    assert!(app.voice.enabled);
    assert_eq!(on_disk(&dir), ON);
    let notice = app.live_flash().unwrap();
    assert!(!notice.contains("NOT saved"), "{notice}");

    app.voice_action(Action::Mute);
    assert!(!app.voice.enabled);
    assert_eq!(on_disk(&dir), ON, "mute is not an opt-out");
    assert!(app.live_flash().unwrap().contains("unchanged"));

    let effects = app.voice_action(Action::On);
    finish_check(&mut app, &effects, Ok("checked".into()));
    app.voice_action(Action::Off);
    assert!(!app.voice.enabled);
    assert_eq!(on_disk(&dir), VoicePrefs::default());
}

#[test]
fn replies_on_saves_both_settings_and_replies_off_keeps_question_speech() {
    let (mut app, dir) = app_with_prefs();
    let effects = app.voice_action(Action::RepliesOn);
    finish_check(&mut app, &effects, Ok("checked".into()));
    assert!(app.voice.enabled && app.voice_replies_enabled());
    assert_eq!(on_disk(&dir), REPLIES);

    app.voice_action(Action::RepliesOff);
    assert!(!app.voice_replies_enabled());
    assert!(app.voice.enabled, "question speech is a separate setting");
    assert_eq!(on_disk(&dir), ON);
}

#[test]
fn a_preference_that_cannot_be_saved_is_reported_and_still_applies_this_run() {
    let (mut app, dir) = app_with_prefs();
    save_fixture(&dir, ON);
    app.load_voice_prefs();
    app.options.prefs_dir = Some(UNWRITABLE.into());

    let effects = app.voice_action(Action::On);
    finish_check(&mut app, &effects, Ok("checked".into()));
    assert!(app.voice.enabled, "the run still gets what was asked for");
    app.voice_action(Action::Off);
    assert!(!app.voice.enabled);
    let notice = app.live_flash().unwrap();
    assert!(notice.contains("NOT saved for next launch"), "{notice}");
    assert!(
        notice.contains("still restores questions on"),
        "a failed save must not claim the new value: {notice}"
    );
    assert_eq!(app.voice.saved, ON, "the cache names what is on disk");
    app.now += chrono::Duration::seconds(10);
    assert!(app.live_flash().is_none());
    let errors = app.transcripts[&app.selected.unwrap()]
        .all_cells()
        .flat_map(crate::plain::describe)
        .collect::<Vec<_>>()
        .join(" ");
    assert!(errors.contains("NOT saved for next launch"), "{errors}");
}

#[test]
fn a_saved_opt_in_is_restored_once_after_connecting_and_never_records() {
    let (mut app, dir) = app_with_prefs();
    save_fixture(&dir, ON);
    app.load_voice_prefs();
    assert!(!app.voice.enabled, "loading must not enable anything");
    assert!(app.voice.capture.is_none());

    let effects = app.voice_reconcile();
    assert!(
        matches!(
            effects.as_slice(),
            [Effect::Voice(VoiceEffect::Check { speech: true, .. })]
        ),
        "{effects:?}"
    );
    assert!(app.voice_reconcile().is_empty(), "restoration runs once");
    finish_check(&mut app, &effects, Ok("local voice ready".into()));
    assert!(app.voice.enabled);
    assert!(app.voice.capture.is_none() && app.voice.replies.armed.is_none());
    let notice = app.live_flash().unwrap();
    assert!(notice.contains("restored"), "{notice}");
    assert!(
        notice.contains("F4"),
        "recording stays shortcut-only: {notice}"
    );
    assert_eq!(on_disk(&dir), ON);
    assert!(app.voice_reconcile().is_empty(), "no second restoration");
}

#[test]
fn a_saved_reply_opt_in_restores_through_one_combined_check_without_arming() {
    let (mut app, dir) = app_with_prefs();
    save_fixture(&dir, REPLIES);
    app.load_voice_prefs();
    let effects = app.voice_reconcile();
    assert!(
        matches!(
            effects.as_slice(),
            [Effect::Voice(VoiceEffect::CheckReplies { .. })]
        ),
        "replies validate speech and dictation together: {effects:?}"
    );
    finish_check(&mut app, &effects, Ok("local voice ready".into()));
    assert!(app.voice.enabled && app.voice_replies_enabled());
    assert!(app.voice.replies.armed.is_none(), "restoring never arms");
    assert!(app.voice.replies.active.is_none());
    assert!(app.live_flash().unwrap().contains("shortcut"));
    assert_eq!(on_disk(&dir), REPLIES);
}

#[test]
fn a_restoration_that_fails_validation_stays_off_and_keeps_the_saved_setting() {
    let (mut app, dir) = app_with_prefs();
    save_fixture(&dir, REPLIES);
    app.load_voice_prefs();
    let effects = app.voice_reconcile();
    finish_check(&mut app, &effects, Err("piper is not installed".into()));
    assert!(!app.voice.enabled && !app.voice_replies_enabled());
    let notice = app.live_flash().unwrap();
    assert!(notice.contains("not restored"), "{notice}");
    assert!(notice.contains("piper is not installed"), "{notice}");
    assert_eq!(on_disk(&dir), REPLIES, "a failed check is not an opt-out");
}

#[test]
fn a_saved_opt_in_never_restores_without_a_connected_native_tui() {
    let (mut app, dir) = app_with_prefs();
    save_fixture(&dir, ON);
    app.load_voice_prefs();

    app.voice.available = false;
    assert!(
        app.voice_reconcile().is_empty(),
        "plain mode never restores"
    );
    app.voice.available = true;
    app.reduce(AppEvent::Disconnected { because: None });
    assert!(
        app.voice_reconcile().is_empty(),
        "no daemon, no restoration"
    );
    assert!(!app.voice.enabled);

    let mut effects = app.reduce(AppEvent::Connected);
    effects.extend(app.voice_reconcile());
    assert_eq!(checks(&effects), 1, "restored once reconnected");
}

#[test]
fn declining_replies_during_a_restoration_keeps_the_saved_question_opt_in() {
    let (mut app, dir) = app_with_prefs();
    save_fixture(&dir, ON);
    app.load_voice_prefs();
    let effects = app.voice_reconcile();
    app.voice_action(Action::RepliesOff);
    assert_eq!(on_disk(&dir), ON, "declining replies is not /voice off");
    finish_check(&mut app, &effects, Ok("checked".into()));
    assert!(app.voice.enabled, "the question opt-in still restores");
    assert!(!app.voice_replies_enabled());
    assert_eq!(on_disk(&dir), ON, "the restored state matches the disk");

    let (mut app, dir) = app_with_prefs();
    save_fixture(&dir, REPLIES);
    app.load_voice_prefs();
    let effects = app.voice_reconcile();
    app.voice_action(Action::RepliesOff);
    assert_eq!(on_disk(&dir), ON, "only the reply half is given up");
    finish_check(&mut app, &effects, Ok("checked".into()));
    assert!(app.voice.enabled, "the combined check restores questions");
    assert!(!app.voice_replies_enabled());
    assert!(app.voice.replies.armed.is_none() && app.voice.replies.active.is_none());
    assert_eq!(on_disk(&dir), ON, "a late check never rewrites preferences");
}

#[test]
fn off_mute_or_a_disconnect_during_a_check_never_lets_it_enable_voice_later() {
    let (mut app, dir) = app_with_prefs();
    save_fixture(&dir, ON);
    app.load_voice_prefs();
    let effects = app.voice_reconcile();
    app.voice_action(Action::Off);
    finish_check(&mut app, &effects, Ok("checked".into()));
    assert!(
        !app.voice.enabled,
        "an in-flight check cannot undo /voice off"
    );
    assert_eq!(on_disk(&dir), VoicePrefs::default());

    let effects = app.voice_action(Action::On);
    app.voice_action(Action::Mute);
    finish_check(&mut app, &effects, Ok("checked".into()));
    assert!(!app.voice.enabled, "an in-flight check cannot undo a mute");

    let effects = app.voice_action(Action::On);
    app.reduce(AppEvent::Disconnected { because: None });
    finish_check(&mut app, &effects, Ok("checked".into()));
    assert!(
        !app.voice.enabled,
        "a late success cannot revive a dead link"
    );
}

#[test]
fn a_lost_control_socket_disarms_replies_without_forgetting_the_saved_setting() {
    let (mut app, dir) = app_with_prefs();
    let effects = app.voice_action(Action::RepliesOn);
    finish_check(&mut app, &effects, Ok("checked".into()));
    assert_eq!(on_disk(&dir), REPLIES);

    app.disarm_voice_replies_transiently();
    assert!(!app.voice_replies_enabled());
    assert_eq!(
        on_disk(&dir),
        REPLIES,
        "an environment failure saves nothing"
    );
    assert!(
        app.voice_reconcile().is_empty(),
        "a transient failure must not re-check and re-arm"
    );
}

#[test]
fn the_view_shows_what_is_on_now_and_status_names_the_next_launch() {
    let (mut app, _dir) = app_with_prefs();
    app.voice.saved = ON;
    app.voice_action(Action::Setup);
    let frame = flat(&render_to(&app, 100, 30));
    for text in [
        "Speak questions off",
        "Voice replies off",
        "Install piper",
        "voice off",
    ] {
        assert!(frame.contains(text), "missing {text}:\n{frame}");
    }
    app.voice_action(Action::Status);
    let status = app.live_flash().unwrap();
    assert!(
        status.contains("next launch: questions on, replies off"),
        "{status}"
    );
}

#[test]
fn enter_on_a_settings_row_toggles_through_the_same_path_as_the_command() {
    let (mut app, dir) = app_with_prefs();
    app.voice_action(Action::Setup);
    let effects = key(&mut app, KeyCode::Enter);
    assert!(app.voice.capture.is_none(), "a toggle opens no microphone");
    finish_check(&mut app, &effects, Ok("checked".into()));
    assert!(app.voice.enabled);
    assert_eq!(on_disk(&dir), ON);

    assert!(
        key(&mut app, KeyCode::Enter).is_empty(),
        "off needs no check"
    );
    assert!(!app.voice.enabled);
    assert_eq!(on_disk(&dir), VoicePrefs::default());
    assert!(
        top_view(&app).as_deref() == Some("VoiceView"),
        "toggles keep the view open"
    );
}

#[test]
fn the_piper_confirmation_discloses_licence_platform_limits_and_that_nothing_starts() {
    // Without its local tools the view lists those instead; see setup.rs.
    if !crate::voice::setup::missing_prerequisites(ModelKind::Piper).is_empty() {
        return;
    }
    let mut app = app();
    app.bottom
        .push_view(Box::new(crate::ui::views::voice::VoiceConfirmView::new(
            ModelKind::Piper,
        )));
    let frame = flat(&render_to(&app, 100, 30));
    for text in [
        "Hugging Face",
        "GPL-3.0",
        "public domain",
        "CPython 3.11",
        "FFmpeg",
        "glibc",
        "No recording or playback",
        "Installing does not enable speech or open the microphone",
        "Your saved voice settings are left alone",
    ] {
        assert!(frame.contains(text), "missing {text}:\n{frame}");
    }
}

#[test]
fn a_short_popup_scrolls_so_the_highlighted_row_stays_visible() {
    let (mut app, _dir) = app_with_prefs();
    app.voice_action(Action::Setup);
    key(&mut app, KeyCode::End);
    let frame = flat(&render_to(&app, 100, 16));
    assert!(
        frame.contains("Install piper"),
        "selected row cut off:\n{frame}"
    );

    key(&mut app, KeyCode::Home);
    let frame = flat(&render_to(&app, 100, 16));
    assert!(frame.contains("Speak questions"), "{frame}");
}

#[test]
fn a_finished_install_re_checks_a_saved_opt_in_but_never_enables_a_new_one() {
    let (mut app, dir) = app_with_prefs();
    save_fixture(&dir, ON);
    app.load_voice_prefs();
    let effects = app.voice_reconcile();
    finish_check(&mut app, &effects, Err("piper is not installed".into()));
    assert!(!app.voice.enabled);

    let id = start_install(&mut app, ModelKind::Piper);
    let mut effects = app.reduce(AppEvent::Voice(Reply::SetupDone {
        id,
        result: Ok("piper installed".into()),
    }));
    assert!(!app.voice.enabled, "installing is not consent to speak");
    effects.extend(app.voice_reconcile());
    assert_eq!(checks(&effects), 1, "the saved opt-in is validated again");
    finish_check(&mut app, &effects, Ok("local voice ready".into()));
    assert!(app.voice.enabled);

    // A user who turned voice off this session is never re-armed by a later
    // install.
    app.voice_action(Action::Off);
    let id = start_install(&mut app, ModelKind::Piper);
    let mut effects = app.reduce(AppEvent::Voice(Reply::SetupDone {
        id,
        result: Ok("piper installed".into()),
    }));
    effects.extend(app.voice_reconcile());
    assert_eq!(checks(&effects), 0, "an opt-out is never re-armed");
    assert!(!app.voice.enabled);
}
