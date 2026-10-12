//! The frame policy driven the way the loop drives it, on a simulated clock:
//! what gets drawn, when, and what never wakes the loop at all.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use aether_client::SubscriptionFrame;
use aether_core::agent::AgentStatus;
use aether_core::event::{Actor, Event, EventDraft, EventKind};
use aether_core::id::{AgentId, EventId, InvocationId};
use aether_core::time::now;
use aether_protocol::{AgentTree, Delta, Reply};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use super::frames::{BACKGROUND_INTERVAL, FRAME_INTERVAL, Frames};
use crate::app::board::BoardFocus;
use crate::app::{App, AppEvent, Flash, Then, TuiOptions};
use crate::testing::{apply_status_with_suspended, loaded_app};
use crate::ui;
use crate::ui::theme::ColorDepth;
use crate::ui::theme::tokens::{Rgb, TerminalPalette, Theme, ThemeSpec};
use crate::ui::transcript::ViewCache;

/// The loop in `run_inner` minus the terminal and the socket: deliver each
/// event at its time, wake when asked, draw when the policy says so. Returns
/// when each frame was drawn, relative to the start.
fn simulate(app: &mut App, events: Vec<(Duration, AppEvent)>, until: Duration) -> Vec<Duration> {
    let start = Instant::now();
    let mut frames = Frames::new(start);
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).expect("test terminal");
    let mut cache = ViewCache::default();
    let mut queue: VecDeque<(Duration, AppEvent)> = events.into();
    let mut now = start;
    let mut drawn = Vec::new();
    for _ in 0..1_000_000 {
        frames.advance_clock(app, now);
        let due_event =
            |queue: &VecDeque<(Duration, AppEvent)>| queue.front().map(|(at, _)| start + *at);
        let backlog = due_event(&queue).is_some_and(|at| at <= now);
        if frames.should_draw(app, now, backlog) {
            terminal
                .draw(|f| ui::render(f, app, &mut cache))
                .expect("render to the test backend");
            frames.drew(app, now);
            drawn.push(now - start);
        }
        let next = match (frames.next_wake(app), due_event(&queue)) {
            (Some(wake), Some(event)) => wake.min(event),
            (wake, event) => match wake.or(event) {
                Some(next) => next,
                None => return drawn,
            },
        };
        if next > start + until {
            return drawn;
        }
        now = now.max(next);
        if due_event(&queue).is_some_and(|at| at <= now)
            && let Some((_, event)) = queue.pop_front()
        {
            frames.advance_clock(app, now);
            app.reduce(event);
        }
    }
    panic!("the loop never went to sleep");
}

fn delta(agent: AgentId, invocation: InvocationId, text: &str) -> AppEvent {
    AppEvent::Frame(SubscriptionFrame::Delta {
        agent,
        invocation,
        delta: Delta::Text { text: text.into() },
    })
}

/// The same tree with every agent idle: no status row, strip or clock left.
fn settled(tree: &AgentTree) -> AgentTree {
    let mut tree = tree.clone();
    for node in tree.nodes.values_mut() {
        node.agent.status = AgentStatus::Ready;
    }
    tree
}

/// A fleet with nothing on screen that moves by itself: every agent idle, the
/// "connected" flash gone, and the board closed.
fn still_fleet() -> App {
    let mut app = loaded_app(4, 50, 20);
    let tree = settled(&app.tree);
    app.reduce(AppEvent::Reply {
        then: Then::Tree,
        result: Ok(Reply::Tree(tree)),
    });
    app.flash = None;
    app.board.open = false;
    app
}

fn other_agent(app: &App) -> AgentId {
    *app.tree
        .nodes
        .keys()
        .find(|id| Some(**id) != app.selected)
        .expect("the fleet has a second agent")
}

#[test]
fn a_burst_of_events_lands_in_one_frame() {
    let mut app = still_fleet();
    let agent = app.selected.expect("selected agent");
    let invocation = InvocationId::new();
    let at = Duration::from_secs(1);
    let burst = (0..200)
        .map(|i| (at, delta(agent, invocation, &format!("word {i} "))))
        .collect();
    let drawn = simulate(&mut app, burst, Duration::from_secs(3));
    // The first frame paints the fixture and the 200 deltas share the next.
    assert_eq!(drawn, [Duration::ZERO, at]);
    let text = &app.transcripts[&agent].stream.text;
    assert!(text.ends_with("word 199 "), "every delta landed: {text:?}");
}

#[test]
fn a_fast_stream_is_capped_at_the_frame_rate() {
    let mut app = loaded_app(4, 50, 20);
    let agent = app.selected.expect("selected agent");
    let invocation = InvocationId::new();
    // 1 000 deltas over one second, five times the frame rate.
    let stream = (0..1_000u64)
        .map(|i| (Duration::from_millis(i), delta(agent, invocation, "token ")))
        .collect();
    let drawn = simulate(&mut app, stream, Duration::from_secs(1));
    for pair in drawn.windows(2) {
        assert!(
            pair[1] - pair[0] >= FRAME_INTERVAL,
            "two frames {:?} apart",
            pair[1] - pair[0]
        );
    }
    assert!(drawn.len() <= 61, "{} frames in one second", drawn.len());
}

/// Connected and quiet. The "connected" flash is dropped: flashes expire on
/// the wall clock, which the simulated loop does not advance.
fn idle_app() -> App {
    let mut app = App::new(TuiOptions::default());
    app.reduce(AppEvent::Connected);
    apply_status_with_suspended(&mut app, 0);
    app.flash = None;
    app
}

#[test]
fn an_idle_tui_draws_nothing() {
    for board_open in [false, true] {
        let mut app = idle_app();
        app.board.open = board_open;
        let drawn = simulate(&mut app, Vec::new(), Duration::from_secs(60));
        // The first frame; the status polls every two seconds change nothing.
        assert_eq!(drawn.len(), 1, "board open {board_open}: {drawn:?}");
    }
}

#[test]
fn an_expired_flash_wakes_the_loop_and_is_cleared_on_that_tick() {
    let mut app = idle_app();
    app.flash("copied 12 lines");
    let wake = app.next_tick_in().expect("the flash's expiry is booked");
    assert!(wake > 1, "the flash stays up for more than a frame: {wake}");
    app.flash = Some(Flash {
        text: "copied 12 lines".into(),
        until: now() - chrono::Duration::seconds(1),
    });
    app.dirty = false;
    app.reduce(AppEvent::Tick { frames: 1 });
    assert_eq!(app.flash, None);
    assert!(app.dirty, "the hint row is redrawn without it");
}

#[test]
fn another_agents_stream_requests_no_frame() {
    let mut app = still_fleet();
    let other = other_agent(&app);
    app.dirty = false;
    app.background_dirty = false;
    app.reduce(delta(other, InvocationId::new(), "off screen"));
    assert!(
        !app.dirty && !app.background_dirty,
        "a delta nobody can see asked for a frame"
    );

    let invocation = InvocationId::new();
    let stream = (1..=1_000u64)
        .map(|i| {
            (
                Duration::from_millis(i * 5),
                delta(other, invocation, "token "),
            )
        })
        .collect();
    let drawn = simulate(&mut app, stream, Duration::from_secs(6));
    assert_eq!(
        drawn,
        Vec::<Duration>::new(),
        "six seconds of an unseen stream drew frames"
    );
}

#[test]
fn another_agents_events_redraw_at_the_background_rate() {
    let mut app = still_fleet();
    let other = other_agent(&app);
    let events = (1..=200u64)
        .map(|seq| {
            let event = Event::from_draft(
                EventDraft::new(Actor::System, EventKind::AgentReady).agent(other),
                EventId::new(),
                10_000 + seq,
                now(),
            );
            let frame = SubscriptionFrame::Event(Box::new(event));
            (Duration::from_millis(seq * 5), AppEvent::Frame(frame))
        })
        .collect();
    let drawn = simulate(&mut app, events, Duration::from_secs(2));
    for pair in drawn.windows(2) {
        assert!(
            pair[1] - pair[0] >= BACKGROUND_INTERVAL,
            "background frames {:?} apart",
            pair[1] - pair[0]
        );
    }
    assert!(drawn.len() >= 2, "the events never reached the screen");
}

#[test]
fn the_status_row_schedules_its_own_frames_only_while_an_agent_runs() {
    let mut app = loaded_app(4, 50, 20);
    // A probed palette, so the running status row pulses.
    app.theme = Theme::resolve(ThemeSpec {
        depth: ColorDepth::Truecolor,
        palette: Some(TerminalPalette {
            fg: Rgb(0xE6, 0xE6, 0xE6),
            bg: Rgb(0x1E, 0x1E, 0x1E),
        }),
        ..ThemeSpec::default()
    });
    app.flash = None;
    app.board.open = false;
    let quiet = Duration::from_secs(1);
    let reply = AppEvent::Reply {
        then: Then::Tree,
        result: Ok(Reply::Tree(settled(&app.tree))),
    };
    let drawn = simulate(&mut app, vec![(quiet, reply)], Duration::from_secs(10));
    let (running, after): (Vec<Duration>, Vec<Duration>) =
        drawn.into_iter().partition(|at| *at < quiet);
    // The pulse books a frame at least every 100 ms, never faster than the cap.
    assert!(running.len() >= 10, "frames: {running:?}");
    for pair in running.windows(2) {
        assert!(pair[1] - pair[0] >= FRAME_INTERVAL, "frames: {running:?}");
    }
    // The tree that stopped the fleet is drawn once, then nothing moves.
    assert_eq!(
        after.len(),
        1,
        "frames after the fleet went quiet: {after:?}"
    );
}

#[test]
fn a_confirmation_is_drawn_as_soon_as_its_snapshot_opens_it() {
    let mut app = still_fleet();
    let agent = app.selected.expect("selected agent");
    app.board.open = true;
    app.board.focus = BoardFocus::Board;
    app.board.cursor = Some(agent);
    let key = KeyEvent::new(KeyCode::Char('X'), KeyModifiers::NONE);
    // Off the 2 s status poll, whose branch refresh may draw a frame of its own.
    let snapshot = Duration::from_millis(1_500);
    let reply = AppEvent::Reply {
        then: Then::BoardConfirm {
            agent,
            remove: false,
            connection: app.connection,
        },
        result: Ok(Reply::Tree(app.tree.clone())),
    };
    let events = vec![
        (Duration::from_secs(1), AppEvent::Key(key)),
        (snapshot, reply),
    ];
    let drawn = simulate(&mut app, events, Duration::from_secs(3));
    assert_eq!(app.bottom.view_stack.len(), 1, "the confirmation opened");
    // Its `y` is live from here on, so it must be on screen too.
    assert!(drawn.contains(&snapshot), "frames: {drawn:?}");
}

#[test]
fn a_subscription_lag_signal_disconnects_without_advancing_the_replay_cursor() {
    let mut app = loaded_app(1, 0, 0);
    app.connected = true;
    app.last_seq = Some(123);
    app.reduce(AppEvent::Frame(SubscriptionFrame::Lagged {
        capacity: 1024,
    }));
    assert!(!app.connected);
    assert_eq!(app.last_seq, Some(123));
    assert!(app.subscription.is_none());
    let flash = &app.flash.as_ref().expect("lag notice").text;
    assert_eq!(flash.matches("reconnecting").count(), 1);
    assert!(!flash.contains("connection lost"));
}
