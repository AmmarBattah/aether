use super::*;

use aether_tui::ui::layout::{BOARD_DOCK_MIN_COLS, BoardMode, board_mode};
use aether_tui::ui::views::{ViewKind, open};

/// Everything that can sit in the frame at once: the bare frame, the board,
/// the two panels over the composer, and every slash view.
#[derive(Debug, Clone, Copy)]
enum Surface {
    Plain,
    Board,
    Shortcuts,
    Find,
    View(ViewKind),
}

const SURFACES: [Surface; 16] = [
    Surface::Plain,
    Surface::Board,
    Surface::Shortcuts,
    Surface::Find,
    Surface::View(ViewKind::Model),
    Surface::View(ViewKind::Usage),
    Surface::View(ViewKind::Approvals),
    Surface::View(ViewKind::Memory),
    Surface::View(ViewKind::Schedules),
    Surface::View(ViewKind::Mcp),
    Surface::View(ViewKind::Doctor),
    Surface::View(ViewKind::Report),
    Surface::View(ViewKind::Artifacts),
    Surface::View(ViewKind::Settings),
    Surface::View(ViewKind::Voice),
    Surface::View(ViewKind::Login),
];

fn with_surface(surface: Surface, theme: Theme) -> App {
    let mut app = stocked_app();
    app.theme = theme;
    match surface {
        Surface::Plain => {}
        Surface::Board => app.board.open = true,
        Surface::Shortcuts => app.bottom.shortcuts = true,
        Surface::Find => {
            app.reduce(ctrl('f'));
            for c in "listing".chars() {
                app.reduce(key(KeyCode::Char(c)));
            }
        }
        Surface::View(kind) => {
            let view = open(kind, &app);
            app.bottom.push_view(view);
        }
    }
    app
}

#[test]
fn every_frame_draws_at_every_size_in_every_theme() {
    for mode in ThemeMode::ALL {
        for ascii in [false, true] {
            for surface in SURFACES {
                for (w, h) in SIZES {
                    let app = with_surface(surface, theme(mode, ascii));
                    let buffer = draw(&app, w, h);
                    let label = format!("{mode:?} ascii={ascii} {surface:?} at {w}x{h}");
                    for cell in buffer.content() {
                        assert!(
                            !cell.symbol().chars().any(char::is_control),
                            "{label}: a control character reached the buffer: {:?}",
                            cell.symbol()
                        );
                    }
                    if ascii {
                        let mut rows = rows(&buffer);
                        // A view's hint row is `every_open_view_reads_in_ascii`'s.
                        if matches!(surface, Surface::View(_)) {
                            rows.pop();
                        }
                        let text = rows.join("\n");
                        assert!(
                            text.is_ascii(),
                            "{:?} {label}: non-ascii under --ascii:\n{text}",
                            non_ascii(&text)
                        );
                    }
                }
            }
        }
    }
}

/// SPEC §1.2: at 100 columns and up the board docks beside the transcript
/// behind one dim `│`; below that it covers the transcript and the bottom
/// pane and leaves only the hint row.
#[test]
fn the_board_docks_from_a_hundred_columns_and_covers_the_frame_below_that() {
    assert_eq!(BOARD_DOCK_MIN_COLS, 100);
    let (question, answer) = ("what is in this repo", "A workspace of crates.");
    let app_with_board = |open: bool| {
        let mut app = quiet(app_with_transcript(&[question, answer]));
        app.board.open = open;
        app
    };

    for (w, h) in [(160u16, 44u16), (120, 40), (100, 30)] {
        let app = app_with_board(true);
        let BoardMode::Docked { width } = board_mode(w, true) else {
            panic!("the board should dock at {w} columns");
        };
        let buffer = draw(&app, w, h);
        let rows = rows(&buffer);
        let separator = w - width - 1;
        let pipe = app.theme.glyphs().pipe;
        for y in 0..h {
            let cell = &buffer[(separator, y)];
            assert_eq!(cell.symbol(), pipe, "at {w}: row {y} has no separator");
            assert!(
                shows(cell, app.theme.rule()),
                "at {w}: the separator is not rule()"
            );
        }
        let left_of = |needle: &str| {
            let y = row_with(&rows, needle)
                .unwrap_or_else(|| panic!("at {w}: no {needle:?} in\n{}", rows.join("\n")));
            column_of(&rows[y], needle).expect("found") < separator
        };
        assert!(
            left_of(answer),
            "at {w}: the transcript is not beside the board"
        );
        assert!(left_of("Steer assistant"), "at {w}: the composer moved");
        let status_line = &rows[usize::from(h) - 2];
        assert!(
            column_of(status_line, "claude-opus-5").is_some_and(|x| x < separator),
            "at {w}: the status line moved: {status_line:?}"
        );
        let header = row_with(&rows, "Agents").expect("the board header");
        assert!(column_of(&rows[header], "Agents").expect("found") > separator);
    }

    for (w, h) in [(99u16, 30u16), (80, 24)] {
        assert_eq!(board_mode(w, true), BoardMode::Overlay);
        let rows = screen(&app_with_board(true), w, h);
        let text = rows.join("\n");
        assert!(text.contains("Agents"), "at {w}: no board:\n{text}");
        // The inspector quotes the answer as the last activity; the question is not.
        for covered in [question, "Steer assistant", "claude-opus-5 · assistant"] {
            assert!(
                !text.contains(covered),
                "at {w}: {covered:?} shows through the overlay:\n{text}"
            );
        }
        assert!(
            rows.last().expect("a hint row").contains("ctrl+g agents"),
            "at {w}: the overlay must leave the hint row:\n{text}"
        );
    }

    // Closed, nothing is held back for it at any width.
    for (w, h) in [(160u16, 44u16), (80, 24)] {
        let text = screen(&app_with_board(false), w, h).join("\n");
        assert!(
            !text.contains("Agents"),
            "at {w}: the board is drawn closed"
        );
        assert!(!text.contains('│'), "at {w}: a separator without a board");
        assert!(text.contains(answer) && text.contains("Steer assistant"));
    }
}

/// SPEC §1.3: Ctrl+G opens and focuses the board, the hint row turns into
/// the board's keys and the composer prompt dims; Ctrl+B puts it away.
#[test]
fn ctrl_g_focuses_the_board_and_ctrl_b_puts_it_away() {
    for (w, h) in [(160u16, 44u16), (80, 24)] {
        let mut app = quiet(app_with_transcript(&["hello", "hello back"]));
        app.theme = dark(false);
        app.size = (w, h);
        app.reduce(ctrl('g'));
        assert!(app.board.focused(), "at {w}");
        let buffer = draw(&app, w, h);
        let rows = rows(&buffer);
        let hint = rows.last().expect("a hint row");
        assert!(
            hint.contains("enter view")
                && hint.contains("esc back")
                && hint.contains("ctrl+b hide") == (w >= BOARD_DOCK_MIN_COLS),
            "at {w}: the hint row should name the board's keys: {hint:?}"
        );
        if w >= BOARD_DOCK_MIN_COLS {
            let prompt = row_with(&rows, "Steer assistant").expect("the composer");
            assert!(
                shows(&buffer[(0, prompt as u16)], app.theme.secondary()),
                "at {w}: the composer prompt should dim while the board has focus"
            );
        }
        app.reduce(ctrl('b'));
        assert!(!app.board.open, "at {w}: Ctrl+B in the board puts it away");
        let text = screen(&app, w, h).join("\n");
        assert!(!text.contains("Agents") && text.contains("Steer assistant"));
    }
}

/// At the docking threshold the board is 40 columns: the narrowest it gets.
/// Labels must read whole there, and a label and its value share a row.
#[test]
fn the_board_says_something_at_the_docking_threshold() {
    use aether_core::model::Effort;
    let mut app = quiet(loaded_app(3, 4, 2));
    let id = viewed(&app);
    app.tree
        .nodes
        .get_mut(&id)
        .expect("node")
        .agent
        .config
        .effort = Some(Effort::High);
    app.rebuild_after_tree_change();
    app.board.open = true;
    let BoardMode::Docked { width } = board_mode(100, true) else {
        panic!("the board docks at 100 columns");
    };
    let rows = screen(&app, 100, 30);
    let board: Vec<String> = rows
        .iter()
        .map(|r| r.chars().skip(usize::from(100 - width)).collect())
        .collect();
    for row in &board {
        for field in row.split_whitespace() {
            assert!(
                !(field.ends_with('…') && field.chars().count() <= 4),
                "a stub where information belongs: {field:?} in {row:?}"
            );
        }
    }
    for name in ["orchestrator", "worker 0", "worker 1"] {
        assert!(
            board.iter().any(|r| r.contains(name)),
            "{name} is missing from the board:\n{}",
            board.join("\n")
        );
    }
    let model = board
        .iter()
        .find(|r| r.trim_start().starts_with("Model"))
        .unwrap_or_else(|| panic!("no model row in:\n{}", board.join("\n")));
    assert!(
        model.contains("claude-opus-5") && model.contains("high"),
        "the model and its effort must stay on one row: {model:?}"
    );
}

/// The cleared view and the full view are both frames. The
/// cleared one drops the finished agent from the board; showing archived
/// work puts it back without drawing anything else differently.
#[test]
fn the_cleared_view_and_the_full_view_both_draw() {
    let finished_name = "worker 0";
    let frame = |show_archived: bool| -> Vec<String> {
        let mut app = quiet(loaded_app(3, 20, 2));
        app.board.open = true;
        app.show_archived = show_archived;
        let finished: Vec<_> = app
            .tree
            .nodes
            .values()
            .filter(|n| n.agent.name == finished_name)
            .map(|n| n.agent.id)
            .collect();
        assert_eq!(finished.len(), 1, "one agent to clear");
        // What the daemon does when the filter is off: the archived agent is
        // simply not in the tree it answers with.
        if !show_archived {
            for id in &finished {
                app.tree.nodes.remove(id);
                app.tree.roots.retain(|r| r != id);
                for node in app.tree.nodes.values_mut() {
                    node.children.retain(|c| c != id);
                }
            }
            app.rebuild_after_tree_change();
        }
        screen(&app, 120, 24)
    };

    let full = frame(true);
    assert!(
        full.iter().any(|r| r.contains(finished_name)),
        "the full view should still list it:\n{}",
        full.join("\n")
    );
    let cleared = frame(false);
    assert!(
        !cleared.iter().any(|r| r.contains(finished_name)),
        "the cleared view still lists it:\n{}",
        cleared.join("\n")
    );
    let status_line = &cleared[cleared.len() - 2];
    assert!(
        status_line.contains("claude-opus-5") && status_line.contains("orchestrator"),
        "the cleared view lost its status line: {status_line:?}"
    );
}

/// An engine's interrupt reason and a kill-switch bypass are
/// computed by the engine and must reach the screen, not a log.
#[test]
fn engine_interrupt_and_kill_switch_reasons_reach_the_transcript() {
    use aether_client::SubscriptionFrame;
    use aether_core::event::{Actor, Event, EventDraft, EventKind};
    use aether_core::id::EventId;

    let mut app = quiet(app_with_transcript(&["hello", "hello back"]));
    let agent = viewed(&app);
    for (seq, name, payload) in [
        (
            100,
            "engine.interrupted",
            serde_json::json!({"engine": "claude-code", "reason": "hit the 90s idle stall window"}),
        ),
        (
            101,
            "engine.kill_switch_bypassed",
            serde_json::json!({"engine": "codex", "tool": "subagent_spawn"}),
        ),
        (
            102,
            "engine.transient_retry",
            serde_json::json!({
                "engine": "claude-code",
                "class": "network",
                "waiting_for": "network",
                "attempt": 2,
                "delay_ms": 30_000,
            }),
        ),
    ] {
        let kind = EventKind::Custom {
            name: name.into(),
            payload,
        };
        let event = Event::from_draft(
            EventDraft::new(Actor::System, kind).agent(agent),
            EventId::new(),
            seq,
            aether_core::time::now(),
        );
        app.reduce(AppEvent::Frame(SubscriptionFrame::Event(Box::new(event))));
    }
    let text = screen(&app, 160, 30).join("\n");
    assert!(text.contains("hit the 90s idle stall window"), "{text}");
    assert!(text.contains("subagent_spawn"), "{text}");
    assert!(text.contains("retry 2 in 30s"), "{text}");
}

/// Rendering is a function of the app: the same state draws the same frame,
/// symbols and styles both, whether or not motion is reduced.
#[test]
fn an_unchanged_app_draws_the_same_frame() {
    for reduced in [false, true] {
        let mut app = app_with_transcript(&["what is in this repo", "A workspace of crates."]);
        app.theme = Theme::resolve(ThemeSpec {
            reduced_motion: reduced,
            ..app.theme.spec()
        });
        app.board.open = true;
        let mut cache = ViewCache::default();
        let first = draw_with(&app, &mut cache, 160, 40);
        let again = draw_with(&app, &mut cache, 160, 40);
        assert_eq!(
            first, again,
            "reduced={reduced}: a redraw changed the frame"
        );
        assert_eq!(
            first,
            draw(&app, 160, 40),
            "reduced={reduced}: a cold cache drew differently"
        );
    }
}

/// Nothing moves when nothing changes: with no agent busy, letting time pass
/// leaves the frame exactly as it was.
#[test]
fn a_quiet_screen_stays_still_as_time_passes() {
    let mut app = quiet(app_with_transcript(&[
        "what is in this repo",
        "A workspace of crates.",
    ]));
    set_status(&mut app, AgentStatus::Ready);
    let before = draw(&app, 120, 30);
    app.now += chrono::Duration::seconds(5);
    assert_eq!(before, draw(&app, 120, 30), "an idle frame moved");
}

/// While the viewed agent runs, the status row is what moves: its pulse and
/// its clock. Nothing above or below it may change with the time.
#[test]
fn only_the_status_row_moves_while_an_agent_runs() {
    let mut app = quiet(app_with_transcript(&[
        "what is in this repo",
        "A workspace of crates.",
    ]));
    let before = draw(&app, 120, 30);
    let status_row = rows(&before)
        .iter()
        .position(|r| r.contains("esc to interrupt"))
        .expect("a running agent has a status row");
    app.now += chrono::Duration::milliseconds(1_500);
    let after = draw(&app, 120, 30);
    for y in (0..30u16).filter(|y| usize::from(*y) != status_row) {
        for x in 0..120u16 {
            assert_eq!(
                before[(x, y)],
                after[(x, y)],
                "({x}, {y}) changed with the clock, outside the status row"
            );
        }
    }
}

#[test]
fn board_actions_remain_visible_in_the_actual_docked_and_overlay_footer() {
    for width in [80, 100] {
        for ascii in [false, true] {
            let mut app = quiet(app_with_transcript(&["hello", "hello back"]));
            app.theme = dark(ascii);
            app.size = (width, 40);
            app.reduce(ctrl('g'));
            let buffer = draw(&app, width, 40);
            let rows = rows(&buffer);
            let hint = rows.last().expect("the footer");
            assert!(
                hint.contains("enter") && hint.contains("esc"),
                "{width}: {hint}"
            );
            let actions = match (ascii, width) {
                (false, 80) => "⇧x stop · ⇧d remove · ⇧p pause · ⇧i interrupt",
                (false, _) => "⇧x/d/p/i stop/remove/pause/interrupt",
                (true, _) => "shift+x/d/p/i stop/remove/pause/interrupt",
            };
            assert!(hint.contains(actions), "{width}: {hint}");
        }
    }
}
