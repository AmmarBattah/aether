#![allow(clippy::unwrap_used)]

use super::*;
use crate::app::TuiOptions;
use crossterm::event::KeyModifiers;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn capture_cancel_refusal_reset_and_reset_all() {
    let app = App::new(TuiOptions::default());
    let mut view = KeysView::default();
    assert!(matches!(
        view.handle_key(key(KeyCode::Enter), &app),
        ViewOutcome::Consumed
    ));
    assert!(view.capturing);
    assert!(matches!(
        view.handle_chord(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            &app
        ),
        ViewOutcome::Consumed
    ));
    assert!(view.notice.as_ref().unwrap().contains("reserved"));
    assert!(view.capturing);
    assert!(matches!(
        view.handle_key(key(KeyCode::Esc), &app),
        ViewOutcome::Consumed
    ));
    assert!(!view.capturing);
    view.handle_key(key(KeyCode::Enter), &app);
    let outcome = view.handle_key(key(KeyCode::F(2)), &app);
    assert!(
        matches!(outcome, ViewOutcome::Act(ref actions) if matches!(actions.as_slice(), [ViewAction::SetKey { action: KeyAction::Agents, chord: Some(_) }]))
    );
    assert!(!view.capturing);
    assert!(
        matches!(view.handle_key(key(KeyCode::Backspace), &app), ViewOutcome::Act(ref actions) if matches!(actions.as_slice(), [ViewAction::SetKey { chord: None, .. }]))
    );
    view.handle_key(key(KeyCode::End), &app);
    assert!(
        matches!(view.handle_key(key(KeyCode::Enter), &app), ViewOutcome::Act(ref actions) if matches!(actions.as_slice(), [ViewAction::ResetKeys]))
    );
}

#[test]
fn settings_editor_dispatch_persistence_and_live_footer() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(TuiOptions {
        prefs_dir: Some(dir.path().into()),
        ..Default::default()
    });
    app.size = (120, 40);
    app.dispatch_key(key(KeyCode::Char('/')));
    for c in "settings".chars() {
        app.dispatch_key(key(KeyCode::Char(c)));
    }
    app.dispatch_key(key(KeyCode::Enter));
    assert_eq!(
        app.bottom.top_view().unwrap().kind(),
        Some(ViewKind::Settings)
    );
    app.dispatch_key(key(KeyCode::Char('5')));
    app.dispatch_key(key(KeyCode::Enter));
    assert_eq!(app.bottom.top_view().unwrap().kind(), Some(ViewKind::Keys));
    app.dispatch_key(key(KeyCode::Enter));
    app.dispatch_key(key(KeyCode::F(2)));
    assert!(app.keys.customised(KeyAction::Agents));
    assert_eq!(
        crate::prefs::UiPrefs::load(dir.path()).unwrap().keys["agents"].as_str(),
        Some("f2")
    );
    app.dispatch_key(key(KeyCode::Esc));
    app.dispatch_key(key(KeyCode::Esc));
    assert!(crate::ui::bottom::footer::hints(&app).contains("f2 agents"));
    app.dispatch_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL));
    assert!(!app.board.focused());
    app.dispatch_key(key(KeyCode::F(2)));
    assert!(app.board.focused());
}

#[test]
fn startup_names_bad_entries_and_retains_valid_overrides() {
    let app = App::new(TuiOptions {
        keys: [
            ("oops".into(), "f2".into()),
            ("details".into(), "f3".into()),
        ]
        .into(),
        ..Default::default()
    });
    assert!(app.live_flash().unwrap().contains("keys.oops"));
    assert!(app.keys.matches(KeyAction::Details, key(KeyCode::F(3))));
    assert!(!app.keys.matches(
        KeyAction::Details,
        KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL)
    ));
}

#[test]
fn rebound_composer_actions_replace_all_aliases_and_popups_keep_enter() {
    let mut app = App::new(TuiOptions::default());
    app.keys
        .assign(KeyAction::Newline, "f2".parse().unwrap())
        .unwrap();
    app.dispatch_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT));
    app.dispatch_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT));
    app.dispatch_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL));
    assert!(app.input.text.is_empty());
    app.dispatch_key(key(KeyCode::F(2)));
    assert_eq!(app.input.text, "\n");
    app.keys
        .assign(KeyAction::LineStart, "f3".parse().unwrap())
        .unwrap();
    app.input.set_text("abc");
    app.dispatch_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));
    app.dispatch_key(key(KeyCode::Home));
    assert_eq!(app.input.cursor, 3);
    app.dispatch_key(key(KeyCode::F(3)));
    assert_eq!(app.input.cursor, 0);
    app.keys
        .assign(KeyAction::Send, "f6".parse().unwrap())
        .unwrap();
    app.input.set_text("/settings");
    app.dispatch_key(key(KeyCode::Enter));
    assert!(app.bottom.view_stack.is_empty());
    app.dispatch_key(key(KeyCode::F(6)));
    assert_eq!(
        app.bottom.top_view().unwrap().kind(),
        Some(ViewKind::Settings)
    );
    app.dispatch_key(key(KeyCode::Esc));
    app.input.set_text("");
    for c in "/settings".chars() {
        app.dispatch_key(key(KeyCode::Char(c)));
    }
    app.dispatch_key(key(KeyCode::Enter));
    assert_eq!(
        app.bottom.top_view().unwrap().kind(),
        Some(ViewKind::Settings)
    );
}

#[test]
fn rebound_help_and_ascii_editor_show_current_keys() {
    let mut app = App::new(TuiOptions {
        ascii: true,
        ..Default::default()
    });
    app.keys
        .assign(KeyAction::Shortcuts, "f2".parse().unwrap())
        .unwrap();
    app.keys
        .assign(KeyAction::Agents, "f3".parse().unwrap())
        .unwrap();
    app.size = (120, 60);
    let frame = crate::testing::render_to(&app, 120, 60);
    assert!(
        frame.contains("f3 agents") && frame.contains("f2 shortcuts"),
        "{frame}"
    );
    app.dispatch_key(key(KeyCode::F(2)));
    assert!(app.bottom.shortcuts);
    let frame = crate::testing::render_to(&app, 120, 60);
    assert!(
        frame.contains("F3") && frame.contains("F2") && frame.contains("f2 or esc to close"),
        "{frame}"
    );
    app.dispatch_key(key(KeyCode::F(2)));
    assert!(!app.bottom.shortcuts);
    app.bottom.push_view(Box::new(KeysView::default()));
    let frame = crate::testing::render_to(&app, 120, 40);
    assert!(frame.is_ascii(), "{frame}");
    assert!(frame.contains("f3") && frame.contains('*'), "{frame}");
}

#[test]
fn capture_ignores_release_and_repeat_and_names_conflicts() {
    let app = App::new(TuiOptions::default());
    let mut view = KeysView::default();
    view.handle_key(key(KeyCode::Enter), &app);
    view.handle_chord(
        KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL),
        &app,
    );
    assert!(
        view.notice
            .as_ref()
            .unwrap()
            .contains("show / hide the board")
    );
    assert!(view.capturing);
    for kind in [KeyEventKind::Release, KeyEventKind::Repeat] {
        let mut event = key(KeyCode::F(2));
        event.kind = kind;
        assert!(matches!(
            view.handle_key(event, &app),
            ViewOutcome::Consumed
        ));
        assert!(view.capturing);
    }
}

#[test]
fn failed_save_keeps_live_binding_and_bad_entries_survive_other_changes() {
    let mut app = App::new(TuiOptions {
        prefs_dir: Some("/dev/null/keys".into()),
        ..Default::default()
    });
    app.set_key(KeyAction::Details, Some("f2".parse().unwrap()));
    assert!(app.live_flash().unwrap().contains("not saved"));
    app.dispatch_key(key(KeyCode::F(2)));
    assert!(app.details);
    let dir = tempfile::tempdir().unwrap();
    let prefs = crate::prefs::UiPrefs {
        keys: [("bad".into(), 42.into())].into(),
        ..Default::default()
    };
    prefs.save(dir.path()).unwrap();
    app.options.prefs_dir = Some(dir.path().into());
    app.set_key(KeyAction::Agents, Some("f3".parse().unwrap()));
    let loaded = crate::prefs::UiPrefs::load(dir.path()).unwrap();
    assert_eq!(loaded.keys["bad"], toml::Value::Integer(42));
    app.set_key(KeyAction::Agents, None);
    assert!(
        !crate::prefs::UiPrefs::load(dir.path())
            .unwrap()
            .keys
            .contains_key("agents")
    );
    app.reset_keys();
    assert!(
        crate::prefs::UiPrefs::load(dir.path())
            .unwrap()
            .keys
            .is_empty()
    );
    assert_eq!(app.keys, crate::keys::KeyBindings::default());
}

#[test]
fn voice_hints_do_not_rewrite_one_rebound_key_as_another() {
    let mut app = App::new(TuiOptions::default());
    app.keys
        .assign(KeyAction::InsertDictation, "f6".parse().unwrap())
        .unwrap();
    app.keys
        .assign(KeyAction::Dictate, "f5".parse().unwrap())
        .unwrap();
    assert_eq!(
        app.voice_key_hint("F4 stops; F5 inserts"),
        "F5 stops; F6 inserts"
    );
}

#[test]
fn a_modified_enter_binding_cannot_also_choose_a_popup_row() {
    let mut app = App::new(TuiOptions::default());
    app.keys
        .assign(KeyAction::Newline, "f2".parse().unwrap())
        .unwrap();
    app.keys
        .assign(KeyAction::Details, "alt+enter".parse().unwrap())
        .unwrap();
    app.dispatch_key(key(KeyCode::Char('/')));
    app.dispatch_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT));
    assert!(app.details);
    assert!(app.bottom.popup.is_some());
    assert_eq!(app.input.text, "/");
}
