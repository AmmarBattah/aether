#![allow(clippy::unwrap_used, clippy::expect_used)]

use aether_client::SubscriptionFrame;
use aether_core::event::{Actor, Event, EventDraft, EventKind};
use aether_core::id::EventId;
use aether_core::time::now;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::*;
use crate::app::AppEvent;
use crate::testing::{app_with_transcript, render_to};

/// A path that can never be a directory, so no test writes preferences and
/// every save fails visibly.
const UNWRITABLE: &str = "/dev/null/unused-voice-config";

fn app() -> App {
    let mut app = app_with_transcript(&[]);
    app.voice.available = true;
    app.options.prefs_dir = Some(UNWRITABLE.into());
    app
}

fn key(app: &mut App, code: KeyCode) -> Vec<Effect> {
    app.reduce(AppEvent::Key(KeyEvent::new(code, KeyModifiers::NONE)))
}

fn record(app: &mut App) -> u64 {
    let effects = key(app, KeyCode::F(4));
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::Voice(VoiceEffect::Record { .. })))
    );
    app.voice.capture.as_ref().unwrap().id
}

fn event(app: &mut App, agent: AgentId, seq: u64, kind: EventKind) -> Vec<Effect> {
    app.reduce(AppEvent::Frame(SubscriptionFrame::Event(Box::new(
        Event::from_draft(
            EventDraft::new(Actor::System, kind).agent(agent),
            EventId::new(),
            seq,
            now(),
        ),
    ))))
}

fn question(app: &mut App, seq: u64) -> Vec<Effect> {
    event(
        app,
        app.selected.unwrap(),
        seq,
        EventKind::QuestionAsked {
            question: "名前 café --help; $(nope)".into(),
        },
    )
}

/// Type name of the view on top of the bottom pane, e.g. `VoiceView`.
fn top_view(app: &App) -> Option<String> {
    let debug = format!("{:?}", app.bottom.top_view()?);
    debug.split([' ', '{', '(']).next().map(str::to_owned)
}

/// Mark every question in `agent`'s transcript answered.
fn answer_questions(app: &mut App, agent: AgentId) {
    for cell in &mut app.transcripts.get_mut(&agent).unwrap().cells {
        if let CellModel::Question { state, .. } = &mut cell.model {
            *state = crate::app::transcript::QuestionState::Answered;
        }
    }
}

fn enable_replies(app: &mut App) {
    let effects = app.voice_action(Action::RepliesOn);
    let [Effect::Voice(VoiceEffect::CheckReplies { id, .. })] = effects.as_slice() else {
        panic!("expected combined STT/TTS check: {effects:?}");
    };
    assert!(!app.voice_replies_enabled());
    let id = *id;
    app.reduce(AppEvent::Voice(Reply::Checked {
        id,
        result: Ok("checked".into()),
    }));
    assert!(app.voice_replies_enabled());
}

fn finish_speech(app: &mut App) {
    let id = app.voice.speech.as_ref().unwrap().id;
    let config = toml::from_str("ffmpeg = '/unused'").unwrap();
    app.reduce(AppEvent::Voice(Reply::Synthesized {
        id,
        result: Ok((config, vec![0; 32], 16000)),
    }));
    app.reduce(AppEvent::Voice(Reply::Played { id, result: Ok(()) }));
}

fn reply_toggle(app: &mut App) -> Result<Vec<Effect>, String> {
    app.handle_voice_control(super::super::control::ControlAction::Reply)
}

fn start_reply(app: &mut App) -> u64 {
    let effects = reply_toggle(app).unwrap();
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::Voice(VoiceEffect::RecordReply { .. })))
    );
    app.voice.replies.active.as_ref().unwrap().id
}

/// An app whose preferences really can be written; the directory has to
/// outlive the app that saves into it.
fn app_with_prefs() -> (App, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("a temporary configuration directory");
    let mut app = app();
    app.options.prefs_dir = Some(dir.path().to_path_buf());
    (app, dir)
}

/// The voice settings currently on disk, as the next launch would read them.
fn on_disk(dir: &tempfile::TempDir) -> VoicePrefs {
    UiPrefs::load(dir.path())
        .expect("saved preferences must stay readable")
        .voice()
}

fn save_fixture(dir: &tempfile::TempDir, voice: VoicePrefs) {
    let mut prefs = UiPrefs::default();
    prefs.set_voice(voice);
    prefs
        .save(dir.path())
        .expect("a writable fixture directory");
}

/// Complete the one configuration check `effects` started.
fn finish_check(app: &mut App, effects: &[Effect], result: Result<String, CheckFailure>) {
    let id = match effects {
        [Effect::Voice(VoiceEffect::Check { id, .. })] => *id,
        [Effect::Voice(VoiceEffect::CheckReplies { id, .. })] => *id,
        other => panic!("expected exactly one configuration check, got {other:?}"),
    };
    app.reduce(AppEvent::Voice(Reply::Checked { id, result }));
}

/// A rendered frame with its line breaks and padding collapsed, so wrapped
/// disclosures can be matched as the sentences they are.
fn flat(frame: &str) -> String {
    let text: String = frame
        .chars()
        .map(|c| {
            if "│─╭╮╰╯├┤┬┴┼".contains(c) {
                ' '
            } else {
                c
            }
        })
        .collect();
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

const ON: VoicePrefs = VoicePrefs {
    questions: true,
    replies: false,
};
const REPLIES: VoicePrefs = VoicePrefs {
    questions: true,
    replies: true,
};

/// How many configuration checks `effects` would start.
fn checks(effects: &[Effect]) -> usize {
    effects
        .iter()
        .filter(|effect| {
            matches!(
                effect,
                Effect::Voice(VoiceEffect::Check { .. } | VoiceEffect::CheckReplies { .. })
            )
        })
        .count()
}

fn reply_status(app: &mut App) -> String {
    use super::super::control::ControlAction;
    let before = format!("{:?}", app.voice);
    let dirty = app.dirty;
    let draft = app.input.text.clone();
    assert!(
        app.handle_voice_control(ControlAction::Status)
            .unwrap()
            .is_empty()
    );
    let status = app.voice_control_ack(ControlAction::Status);
    assert_eq!(
        format!("{:?}", app.voice),
        before,
        "status must not mutate voice state"
    );
    assert_eq!(app.dirty, dirty);
    assert_eq!(app.input.text, draft);
    assert!(
        status.len() <= 200,
        "IPC must not truncate diagnostics: {status}"
    );
    status
}

#[path = "ui_tests/diagnostics.rs"]
mod diagnostics;
#[path = "ui_tests/missing.rs"]
mod missing;
#[path = "ui_tests/preferences.rs"]
mod preferences;
#[path = "ui_tests/recording.rs"]
mod recording;
#[path = "ui_tests/replies.rs"]
mod replies;
#[path = "ui_tests/setup.rs"]
mod setup_scenarios;
