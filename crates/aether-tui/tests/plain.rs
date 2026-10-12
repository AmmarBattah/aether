#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

//! `--plain` must carry the same information as the screen, in text a screen
//! reader or a log can take: no escapes, no glyphs, no motion. And it must
//! end: a pipe that closes is the only way out a script has.

use aether_protocol::Command;
use aether_tui::app::transcript::{CallStatus, CallTarget, Cell, CellId, CellModel, CellRevision};
use aether_tui::app::{App, AppEvent, Effect, TuiOptions};
use aether_tui::commands::{InputAction, SlashCommand, parse};
use aether_tui::plain;
use aether_tui::testing::{app_with_transcript, apply_status_with_suspended};
use aether_tui::ui::theme::tokens::UNICODE_GLYPHS;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Every block kind §9.9 has to flatten, in ASCII, so anything non-ASCII in
/// the output is decoration this crate added.
const MARKDOWN: &str = "\
## Plan

Read `Cargo.toml` and the **workspace** members, then the *kernel* suite.

- one
- two

1. first
2. second

- [x] done
- [ ] pending

```rust
fn main() {}
```

| crate | tests |
|---|---|
| aether-core | 41 |

> Note: nothing is written until you approve.

---

See [the notes](https://example.com/arch).";

fn described(reply: &str) -> String {
    let app = app_with_transcript(&["how do I build this", reply]);
    let agent = app.selected.expect("an agent");
    let mut text = plain::summary(&app, agent);
    for cell in app.transcripts[&agent].all_cells() {
        for line in plain::describe(cell) {
            text.push('\n');
            text.push_str(&line);
        }
    }
    text
}

fn cell(model: CellModel) -> Cell {
    Cell {
        id: CellId(1),
        revision: CellRevision(0),
        at: aether_core::time::now(),
        model,
    }
}

/// Every glyph the screen draws, from the theme's table plus markdown's own.
fn screen_glyphs() -> Vec<&'static str> {
    let g = UNICODE_GLYPHS;
    let mut glyphs = vec![
        g.prompt,
        g.bullet,
        g.child,
        g.pipe,
        g.tee,
        g.bar,
        g.rule,
        g.running,
        g.waiting,
        g.idle,
        g.needs_you,
        g.blocked,
        g.paused,
        g.done,
        g.failed,
        g.cancelled,
        g.queued,
        g.caret,
        g.ellipsis,
        g.dot,
    ];
    glyphs.extend(["\u{258e}", "\u{2611}", "\u{2610}", "\u{2192}"]);
    glyphs
}

#[test]
fn plain_output_adds_nothing_a_reader_has_to_skip() {
    let text = described(MARKDOWN);
    assert!(!text.contains('\x1b'), "an escape sequence got through");
    for glyph in screen_glyphs() {
        assert!(
            !text.contains(glyph),
            "the glyph {glyph:?} got through:\n{text}"
        );
    }
    // The model wrote only ASCII, so anything else in the output is ours.
    assert!(
        text.is_ascii(),
        "plain mode drew something the model did not:\n{text}"
    );
}

/// Harness-authored notices name a change with an arrow; plain mode spells it.
#[test]
fn plain_notices_are_ascii() {
    let lines = plain::describe(&cell(CellModel::Notice {
        kind: aether_tui::app::transcript::NoticeKind::ModelChange,
        text: "Model \u{2192} opus-5.5 high from the next turn".into(),
    }));
    let text = lines.join("\n");
    assert!(text.is_ascii(), "{text}");
    assert!(text.contains("opus-5.5 high"), "{text}");
}

#[test]
fn plain_output_strips_what_the_model_can_do_to_a_terminal() {
    // A reply that tries to colour the terminal, ring it, and write its
    // clipboard through OSC 52.
    let hostile = "red \x1b[31mtext\x1b[0m\x07 and \x1b]52;c;aGk=\x07 done\u{9b}5m";
    let text = described(hostile);
    let typed = {
        let app = app_with_transcript(&[hostile]);
        let agent = app.selected.expect("an agent");
        app.transcripts[&agent]
            .all_cells()
            .flat_map(plain::describe)
            .collect::<Vec<_>>()
            .join("\n")
    };
    for text in [&text, &typed] {
        for bad in ['\x1b', '\x07', '\u{9b}'] {
            assert!(!text.contains(bad), "{bad:?} survived:\n{text}");
        }
        // What is left is inert text: the model's own words are never guessed at.
        assert!(text.contains("red "), "{text}");
        assert!(text.contains(" done"), "{text}");
        assert!(text.lines().all(|l| !l.chars().any(char::is_control)));
    }
}

#[test]
fn plain_output_renders_markdown_as_lines() {
    let text = described(MARKDOWN);
    // Structure markers are gone, the words are not.
    for marker in ["## ", "**", "```", "|---", "> Note", "]("] {
        assert!(!text.contains(marker), "{marker:?} survived:\n{text}");
    }
    for word in [
        "Plan",
        "Cargo.toml",
        "workspace",
        "kernel",
        "- one",
        "1. first",
        "[x] done",
        "[ ] pending",
        "fn main() {}",
        "aether-core",
        "41",
        "nothing is written until you approve",
        "https://example.com/arch",
    ] {
        assert!(text.contains(word), "{word:?} is missing:\n{text}");
    }
    // A table is columns, not pipes.
    let row = text
        .lines()
        .find(|l| l.contains("aether-core"))
        .expect("the table row");
    assert!(!row.contains('|'), "the table kept its pipes: {row:?}");
}

#[test]
fn plain_output_keeps_what_the_screen_shows() {
    let app = app_with_transcript(&["how do I build this", "Run cargo build."]);
    let agent = app.selected.expect("an agent");
    let lines: Vec<String> = app.transcripts[&agent]
        .all_cells()
        .flat_map(plain::describe)
        .collect();
    let text = lines.join("\n");

    // The screen's grammar in ASCII: `> ` for the person, `* ` for the agent.
    assert!(lines.iter().any(|l| l == "> how do I build this"), "{text}");
    assert!(lines.iter().any(|l| l == "* Run cargo build."), "{text}");
    // The status names the agent and its state in words, since there is no glyph to read.
    let summary = plain::summary(&app, agent);
    assert!(summary.contains(&app.agent_name(agent)), "{summary}");
    assert!(summary.contains("running"), "{summary}");
}

/// Over a pipe every line arrives before the first agent exists. Each one
/// must wait for that agent, not start one of its own — otherwise the
/// replies land in transcripts nobody is watching.
#[test]
fn messages_typed_before_the_first_agent_all_go_to_it() {
    let mut app = App::new(TuiOptions::default());
    app.reduce(AppEvent::Connected);
    app.status = aether_tui::testing::loaded_app(0, 0, 0).status;
    let mut creates = 0;
    for line in ["hello", "and again", "once more"] {
        app.reduce(AppEvent::Paste(line.into()));
        for effect in app.reduce(AppEvent::Key(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE,
        ))) {
            if let Effect::Request {
                command: Command::CreateAgent(_),
                ..
            } = effect
            {
                creates += 1;
            }
        }
    }
    assert_eq!(creates, 1, "three messages must not start three agents");
}

#[test]
fn ctrl_d_and_slash_quit_end_the_session() {
    let mut app = App::new(TuiOptions::default());
    let ctrl_d = AppEvent::Key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
    assert_eq!(app.reduce(ctrl_d), vec![Effect::Quit]);
    assert!(app.quit_requested);

    // Never on top of a draft: EOF is an empty line, not a lost message.
    let mut app = App::new(TuiOptions::default());
    app.reduce(AppEvent::Paste("half a thought".into()));
    let ctrl_d = AppEvent::Key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
    assert!(app.reduce(ctrl_d).is_empty(), "Ctrl+D dropped a draft");
    assert!(!app.quit_requested);

    assert_eq!(parse("/quit"), InputAction::Command(SlashCommand::Quit));
    let mut app = App::new(TuiOptions::default());
    app.reduce(AppEvent::Paste("/quit".into()));
    let effects = app.reduce(AppEvent::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )));
    assert_eq!(effects, vec![Effect::Quit]);
}

/// §9.5 lets a message be composed over several lines with Ctrl+J. Stripping
/// the newline with the other control characters glued the words at the seam.
#[test]
fn a_multi_line_message_stays_several_lines() {
    let app = app_with_transcript(&["first line\nsecond line", "fine"]);
    let agent = app.selected.expect("an agent");
    let first = app.transcripts[&agent].cells.first().expect("the message");
    assert!(matches!(first.model, CellModel::User { .. }));
    let lines: Vec<String> = plain::describe(first)
        .into_iter()
        .filter(|l| !l.is_empty())
        .collect();
    assert_eq!(lines, ["> first line", "  second line"], "{lines:?}");
}

/// A call that failed before it ran has no duration to report; plain mode
/// says it failed and why, rather than reading as an ordinary success.
#[test]
fn plain_output_says_why_a_call_that_never_ran_failed() {
    let lines = plain::describe(&cell(CellModel::Called {
        target: CallTarget::Tool {
            name: "ghost_tool".into(),
        },
        args: String::new(),
        result: String::new(),
        status: CallStatus::Failed,
        error: Some("unresolvable tool name".into()),
        artifacts: Vec::new(),
    }));
    let text = lines.join("\n");
    assert!(lines[0].contains("ghost_tool"), "{text}");
    assert!(text.contains("Error: unresolvable tool name"), "{text}");
    assert!(text.is_ascii(), "{text}");
}

#[test]
fn plain_questions_explain_whether_they_can_be_answered() {
    use aether_tui::app::transcript::QuestionState;
    let describe = |state| {
        plain::describe(&cell(CellModel::Question {
            asker: aether_core::id::AgentId::new(),
            asker_name: "tester".into(),
            text: "First line\nsecond line\x1b[31m".into(),
            state,
            seq: 1,
        }))
    };

    let awaiting = describe(QuestionState::Pending);
    assert_eq!(awaiting.last().unwrap(), "reply to this agent to answer");
    assert!(awaiting.iter().any(|line| line.contains("tester asks")));
    assert!(awaiting.iter().any(|line| line.contains("First line")));
    assert!(awaiting.iter().any(|line| line.contains("second line")));

    let answered = describe(QuestionState::Answered);
    assert_eq!(answered.last().unwrap(), "answered");
    assert!(!answered.iter().any(|line| line.contains("reply to")));

    let ended = describe(QuestionState::AskerEnded);
    assert_eq!(ended.last().unwrap(), "unanswered; this agent has ended");
    assert!(
        !ended
            .iter()
            .any(|line| line == "answered" || line.contains("reply to"))
    );
    for lines in [awaiting, answered, ended] {
        assert!(lines.iter().all(|line| line.is_ascii()), "{lines:?}");
        assert!(
            lines
                .iter()
                .all(|line| line.chars().all(|c| !c.is_control()))
        );
    }
}

/// A person reading a log, not the TUI, still has to see a suspended fleet.
#[test]
fn the_fleet_line_reports_suspended_agents() {
    let mut app = app_with_transcript(&["hi"]);
    let agent = app.selected.expect("an agent");
    apply_status_with_suspended(&mut app, 0);
    let quiet = plain::summary(&app, agent);
    assert!(
        !quiet.contains("suspended"),
        "a zero count should not render: {quiet:?}"
    );

    apply_status_with_suspended(&mut app, 2);
    let busy = plain::summary(&app, agent);
    assert!(
        busy.contains("2 suspended"),
        "suspended agents are not on the fleet line: {busy:?}"
    );
}
