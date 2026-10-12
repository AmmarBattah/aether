#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

//! Motion is a property of the whole app, not just the rules: an idle TUI
//! must ask for no frames at all, and reduced motion must reach every
//! animation rather than only the ones a caller remembered to check.

use aether_core::agent::AgentStatus;
use aether_core::id::AgentId;
use aether_tui::app::{App, AppEvent, Effect, IDLE_PERIOD_MS, Then, TuiOptions};
use aether_tui::motion::{self, Motion};
use aether_tui::ui::theme::tokens::{Rgb, TerminalPalette};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn app(reduced: bool) -> App {
    App::new(TuiOptions {
        reduced_motion: reduced,
        ..TuiOptions::default()
    })
}

#[test]
fn an_idle_tui_asks_for_no_frames() {
    let mut app = app(false);
    assert_eq!(
        app.next_tick_in(),
        None,
        "nothing is moving, so nothing should wake the loop"
    );
    // A late tick, whatever its size, must not make the screen dirty.
    app.dirty = false;
    let frames = motion::frames(IDLE_PERIOD_MS);
    let effects = app.reduce(AppEvent::Tick { frames });
    assert!(effects.is_empty(), "an idle tick asked for {effects:?}");
    assert!(!app.dirty, "an idle tick must not force a redraw");
}

/// An idle viewed agent with one running child, so the agents strip is the
/// only thing that moves, on a terminal whose palette allows a pulse.
fn running_child_app(reduced: bool) -> App {
    let mut app = aether_tui::testing::populated_app();
    app.approvals.clear();
    if reduced {
        app.input.set_text("/motion reduced");
        app.reduce(AppEvent::Key(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE,
        )));
    }
    app.set_palette(Some(TerminalPalette {
        fg: Rgb(230, 230, 230),
        bg: Rgb(20, 20, 20),
    }));
    let viewed = app.selected.expect("the fixture views its agent");
    app.tree.nodes.get_mut(&viewed).expect("node").agent.status = AgentStatus::Ready;
    add_running_child(&mut app, viewed);
    app
}

#[test]
fn reduced_motion_only_ticks_the_once_a_second_clock() {
    let reduced = App::new(TuiOptions {
        reduced_motion: true,
        ..TuiOptions::default()
    });
    assert!(reduced.motion.reduced());
    assert!(reduced.theme.reduced_motion(), "the theme must hear it too");
    for tick in [0, 7, 1_000] {
        assert_eq!(reduced.motion.spinner(tick), motion::SPINNER_STILL);
    }

    // The control: with full motion the running child pulses.
    let full = running_child_app(false);
    let pulse = full.next_tick_in().expect("a running child moves");
    assert!(
        pulse < motion::frames(1_000),
        "full motion should pulse faster than the clock: {pulse}"
    );
    let still = running_child_app(true);
    assert!(
        still.motion.reduced(),
        "/motion reduced sets the clock still"
    );
    assert!(
        still.theme.reduced_motion(),
        "/motion reduced reaches the theme"
    );
    assert_eq!(
        still.next_tick_in(),
        Some(motion::frames(1_000)),
        "reduced motion keeps only the elapsed clock"
    );
}

/// Ticks until the periodic status refresh fires, delivered `batch` at a time.
fn ticks_until_status_refresh(batch: u64) -> u64 {
    let mut app = app(false);
    app.reduce(AppEvent::Connected);
    for _ in 0..1_000 {
        let effects = app.reduce(AppEvent::Tick { frames: batch });
        let refresh = effects.iter().any(|e| {
            matches!(
                e,
                Effect::Request {
                    then: Then::StatusPoll,
                    ..
                }
            )
        });
        if refresh {
            return app.tick;
        }
    }
    panic!("the status refresh never fired at a batch of {batch}");
}

#[test]
fn a_batched_tick_advances_deadlines_by_wall_time() {
    // A deadline must fall at the same elapsed time whether the clock ran at
    // 60 Hz or woke four times a second.
    let fast = ticks_until_status_refresh(1);
    let slow = ticks_until_status_refresh(motion::frames(IDLE_PERIOD_MS));
    assert_eq!(fast, slow, "the two clocks must agree on elapsed");
}

#[test]
fn the_environment_can_ask_for_reduced_motion() {
    // The parse, not the process environment: a test that mutates the
    // environment races every other test in the binary.
    for on in ["1", "true", "yes", " ON "] {
        assert_eq!(Motion::reduced_from_env(on), Some(true), "{on:?}");
    }
    for off in ["0", "false", "no", "off"] {
        assert_eq!(Motion::reduced_from_env(off), Some(false), "{off:?}");
    }
    // Empty is not an answer, and neither is an unset variable.
    assert_eq!(Motion::reduced_from_env("  "), None);
    assert_eq!(Motion::env_reduced(), None);
}

/// A running child of `parent`, delivered the way the daemon's tree reply is.
fn add_running_child(app: &mut App, parent: AgentId) {
    let mut tree = app.tree.clone();
    let mut child = tree.nodes[&parent].clone();
    let id = AgentId::new();
    child.agent.id = id;
    child.agent.parent = Some(parent);
    child.agent.name = "child".into();
    child.agent.status = AgentStatus::Running;
    child.children.clear();
    tree.nodes.insert(id, child);
    tree.nodes
        .get_mut(&parent)
        .expect("parent")
        .children
        .push(id);
    app.reduce(AppEvent::Reply {
        then: Then::Tree,
        result: Ok(aether_protocol::Reply::Tree(tree)),
    });
}
