use super::*;
use crate::ui::views::voice::{VoiceConfirmView, VoiceInstallView};
use crate::voice::setup::missing_prerequisites;

/// Starts installing `model` and returns the one install id it started.
fn install(app: &mut App, model: ModelKind) -> u64 {
    match app.voice_install(model).as_slice() {
        [Effect::Voice(VoiceEffect::Setup { id, dir, model: m })] => {
            assert_eq!(dir, &PathBuf::from(UNWRITABLE));
            assert_eq!(*m, model);
            *id
        }
        other => panic!("expected exactly one Setup effect, got {other:?}"),
    }
}

#[test]
fn setup_action_gated_like_every_other_voice_action_and_never_touches_recording() {
    let mut app = app();
    app.voice.available = false;
    assert!(app.voice_action(Action::Setup).is_empty());
    assert!(app.bottom.view_stack.is_empty());
    assert!(app.live_flash().unwrap().contains("interactive"));

    app.voice.available = true;
    for command in ["/voice", "/voice setup"] {
        app.input.set_text(command);
        assert!(key(&mut app, KeyCode::Enter).is_empty());
        assert_eq!(top_view(&app).as_deref(), Some("VoiceView"), "{command}");
        assert!(app.voice.capture.is_none());
        assert!(!app.voice.enabled);
        assert!(key(&mut app, KeyCode::Esc).is_empty());
        assert!(app.bottom.view_stack.is_empty());
    }
    app.input.set_text("/voice status");
    assert!(key(&mut app, KeyCode::Enter).is_empty());
    assert!(app.bottom.view_stack.is_empty());
    assert!(app.live_flash().unwrap().contains("F4 record/stop"));
}

#[test]
fn confirm_esc_or_n_returns_to_the_voice_view_without_ever_installing() {
    for back in [KeyCode::Esc, KeyCode::Char('n')] {
        let mut app = app();
        app.voice_action(Action::Setup);
        // The fourth row is the first install.
        assert!(key(&mut app, KeyCode::Char('4')).is_empty());
        assert_eq!(app.bottom.view_stack.len(), 2, "the confirmation is pushed");
        assert!(key(&mut app, back).is_empty());
        assert_eq!(app.bottom.view_stack.len(), 1);
        assert_eq!(top_view(&app).as_deref(), Some("VoiceView"));
        assert!(app.voice.install.is_none());
    }
}

#[test]
fn an_install_starts_exactly_one_effect_and_cancel_is_idempotent() {
    let mut app = app();
    let id = install(&mut app, ModelKind::Small);
    assert_eq!(app.voice.install_stage(ModelKind::Small), Some("starting"));
    assert!(
        app.voice_install(ModelKind::Tiny).is_empty(),
        "a second install must not start while one runs"
    );
    assert!(app.live_flash().unwrap().contains("already running"));

    // The install view's `c` asks the backend to stop; a second cancel must not repeat it.
    app.bottom
        .push_view(Box::new(VoiceInstallView::new(ModelKind::Small)));
    assert_eq!(
        key(&mut app, KeyCode::Char('c')),
        vec![Effect::Voice(VoiceEffect::Cancel(id))]
    );
    assert!(app.bottom.view_stack.is_empty());
    assert!(app.voice_action(Action::Cancel).is_empty());
}

#[test]
fn setup_progress_only_updates_the_matching_install_and_done_ends_it() {
    let mut app = app();
    let id = install(&mut app, ModelKind::Tiny);

    app.reduce(AppEvent::Voice(Reply::SetupProgress {
        id: id + 99,
        stage: "stale".into(),
    }));
    assert_eq!(app.voice.install_stage(ModelKind::Tiny), Some("starting"));
    app.reduce(AppEvent::Voice(Reply::SetupProgress {
        id,
        stage: "cloning whisper.cpp v1.9.4".into(),
    }));
    assert_eq!(
        app.voice.install_stage(ModelKind::Tiny),
        Some("cloning whisper.cpp v1.9.4")
    );

    app.reduce(AppEvent::Voice(Reply::SetupDone {
        id: id + 99,
        result: Ok("stale".into()),
    }));
    assert!(app.voice.install.is_some(), "a stale result ends nothing");
    app.reduce(AppEvent::Voice(Reply::SetupDone {
        id,
        result: Ok("installed whisper.cpp v1.9.4 (tiny model) at /x".into()),
    }));
    assert!(app.voice.install.is_none());
    assert!(app.live_flash().unwrap().contains("installed whisper.cpp"));
    // Voice must remain off after a successful install; F4 never auto-fires.
    assert!(!app.voice.enabled);
    assert!(app.voice.capture.is_none());

    let id = install(&mut app, ModelKind::Base);
    app.reduce(AppEvent::Voice(Reply::SetupDone {
        id,
        result: Err("missing prerequisites:\n  - git: install git".into()),
    }));
    assert!(app.voice.install.is_none());
    assert!(app.live_flash().unwrap().contains("voice setup failed"));
}

#[test]
fn active_install_survives_reopen_and_cancels_on_disconnect() {
    let mut app = app();
    let id = install(&mut app, ModelKind::Parakeet);
    assert!(app.voice_action(Action::Setup).is_empty());
    assert_eq!(top_view(&app).as_deref(), Some("VoiceView"));
    assert_eq!(
        app.voice.install_stage(ModelKind::Parakeet),
        Some("starting")
    );
    assert_eq!(app.voice.status(), Some("voice installing /voice cancel"));
    app.connected = false;
    assert!(
        app.voice_reconcile()
            .contains(&Effect::Voice(VoiceEffect::Cancel(id)))
    );
    assert!(app.voice_reconcile().is_empty());
}

#[test]
fn voice_cancel_and_suspend_cancel_setup_without_recording() {
    for suspend in [false, true] {
        let mut app = app();
        let id = install(&mut app, ModelKind::Parakeet);
        let effects = if suspend {
            app.voice_suspend()
        } else {
            app.voice_action(Action::Cancel)
        };
        assert_eq!(effects, vec![Effect::Voice(VoiceEffect::Cancel(id))]);
        assert!(app.voice.capture.is_none());
    }
}

#[test]
fn the_install_view_shows_live_progress() {
    let mut app = app();
    let id = install(&mut app, ModelKind::Base);
    app.reduce(AppEvent::Voice(Reply::SetupProgress {
        id,
        stage: "building whisper-cli".into(),
    }));
    app.bottom
        .push_view(Box::new(VoiceInstallView::new(ModelKind::Base)));
    let frame = render_to(&app, 100, 30);
    assert!(frame.contains("Install base"), "{frame}");
    assert!(frame.contains("building whisper-cli"), "{frame}");
}

/// The disclosure needs the local tools present; without them the view
/// names what is missing and says nothing was downloaded.
#[test]
fn narrow_confirmation_discloses_model_runtime_license_and_disk_before_install() {
    let disclosures: [(ModelKind, &[&str]); 2] = [
        (
            ModelKind::Parakeet,
            &["PyPI", "CC-BY-4.0", "Python 3.11+", "640 MiB"],
        ),
        (ModelKind::Base, &["no sudo"]),
    ];
    for (model, texts) in disclosures {
        let mut app = app();
        // A live flash would cover the view's key hints.
        app.flash = None;
        app.bottom.push_view(Box::new(VoiceConfirmView::new(model)));
        let frame = flat(&render_to(&app, 60, 24));
        let missing = missing_prerequisites(model);
        if !missing.is_empty() {
            assert!(frame.contains("nothing was downloaded"), "{frame}");
            for item in missing {
                assert!(frame.contains(item.name), "{frame}");
            }
            continue;
        }
        for text in texts {
            assert!(frame.contains(text), "missing {text}:\n{frame}");
        }
        assert!(frame.contains("enter install"), "{frame}");
    }
}

/// Piper's notice is the longest one, so a short terminal must scroll it
/// rather than silently drop the licence or the keys that act on it.
#[test]
fn the_long_piper_notice_scrolls_to_its_no_enable_assurance() {
    let mut app = app();
    if !missing_prerequisites(ModelKind::Piper).is_empty() {
        return;
    }
    app.flash = None;
    app.bottom
        .push_view(Box::new(VoiceConfirmView::new(ModelKind::Piper)));
    let frame = flat(&render_to(&app, 60, 24));
    assert!(frame.contains("GPL-3.0"), "{frame}");
    assert!(frame.contains("enter install"), "{frame}");
    let mut seen = frame.contains("Installing does not enable speech");
    for _ in 0..24 {
        if seen {
            break;
        }
        key(&mut app, KeyCode::Down);
        let frame = flat(&render_to(&app, 60, 24));
        assert!(
            frame.contains("enter install"),
            "keys stay pinned:\n{frame}"
        );
        seen = frame.contains("Installing does not enable speech");
    }
    assert!(seen, "the no-enable assurance must be reachable at 60x24");
}
