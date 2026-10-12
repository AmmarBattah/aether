use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::style::Style;

use super::*;
use crate::app::approval_flow::fixtures::{
    Fleet, assert_style, at, column_of, draw, fleet, rows, shell_approval,
};
use crate::ui::theme::tokens::ThemeSpec;

const COMMAND: &str = "cargo test -p aether --test cli_runs -- --nocapture";

/// Mock c: the viewed root sees tester's (a grandchild's) approval, with
/// implementer's queued behind it.
fn mock_c() -> (Fleet, ApprovalRequest) {
    let mut f = fleet();
    let request = shell_approval(f.tester, COMMAND, at(-2_000));
    let behind = shell_approval(f.implementer, "cargo build", at(-1_000));
    f.app.approvals = vec![request.clone(), behind];
    (f, request)
}

fn paint(app: &App, request: &ApprovalRequest, width: u16) -> Terminal<TestBackend> {
    let height = desired_height(request, app, width);
    draw(width, height, |frame| {
        render(
            frame,
            frame.area(),
            request,
            &app.bottom.approval,
            app,
            &app.theme,
        )
    })
}

#[test]
fn grandchild_approval_matches_mock_c_at_120() {
    let (f, request) = mock_c();
    let terminal = paint(&f.app, &request, 120);
    assert_eq!(
        rows(&terminal),
        [
            "",
            "  Allow tester to run this command?",
            "  tester [verifier] · child of implementer · haiku-4.5 low",
            "",
            "  $ cargo test -p aether --test cli_runs -- --nocapture",
            "",
            "› 1. Yes, run it (y)",
            "  2. Yes, and allow cargo test -p aether --test cli_runs -- --nocapture for tester and its subagents (a)",
            "  3. Always allow cargo test -p aether --test cli_runs -- --nocapture everywhere (p)",
            "  4. No, and tell tester what to do instead (esc)",
            "",
            "  enter confirm · ↑↓ select · o open tester  ·  1 more waiting (implementer)",
            "  Esc deny · Ctrl+C defer",
            "",
        ]
    );
}

#[test]
fn viewed_agent_approval_drops_attribution_and_open() {
    let (mut f, request) = mock_c();
    f.app.selected = Some(f.tester);
    let terminal = paint(&f.app, &request, 120);
    let rows = rows(&terminal);
    assert_eq!(rows[1], "  Allow tester to run this command?");
    assert_eq!(rows[2], "");
    assert_eq!(
        rows[3],
        "  $ cargo test -p aether --test cli_runs -- --nocapture"
    );
    assert_eq!(
        rows[rows.len() - 3],
        "  enter confirm · ↑↓ select  ·  1 more waiting (implementer)"
    );
}

#[test]
fn repeated_worker_names_are_numbered_without_repeating_the_role() {
    use crate::app::approval_flow::fixtures::{add_child, agent};
    use aether_core::model::Effort;

    let mut f = fleet();
    let first = agent("worker", "worker", Some(f.root), "mock-1", Effort::Low);
    let mut second = first.clone();
    second.id = AgentId::new();
    second.created_at += chrono::Duration::seconds(1);
    add_child(&mut f.app, f.root, first);
    let id = add_child(&mut f.app, f.root, second);
    let request = shell_approval(id, "cargo test", at(0));
    let text = rows(&paint(&f.app, &request, 120)).join("\n");
    assert!(
        text.contains("Allow worker 2 to run this command?"),
        "{text}"
    );
    assert!(text.contains("worker 2 · child of aether"), "{text}");
    assert!(!text.contains("[worker]"), "{text}");
    assert!(text.contains("for worker 2 and its subagents"), "{text}");
}

#[test]
fn cells_paint_with_theme_tokens() {
    let (f, request) = mock_c();
    let theme = f.app.theme;
    let terminal = paint(&f.app, &request, 120);
    let on_tint = |style: Style| theme.tint().patch(style);
    let selected_code = theme.accent().patch(theme.title());

    assert_style(&terminal, 2, 1, on_tint(theme.title()));
    assert_style(&terminal, 2, 2, on_tint(theme.secondary()));
    assert_style(&terminal, 2, 4, on_tint(theme.secondary()));
    assert_style(&terminal, 4, 4, on_tint(theme.text()));
    assert_style(&terminal, 0, 6, on_tint(selected_code));
    assert_style(&terminal, 5, 6, on_tint(theme.title()));
    let y_key = column_of(&terminal, 6, "(y)") + 1;
    assert_style(&terminal, y_key, 6, on_tint(theme.title()));
    let code = column_of(&terminal, 7, "cargo");
    assert_style(&terminal, code, 7, on_tint(theme.accent()));
    assert_style(&terminal, 2, 11, on_tint(theme.title()));
    let label = column_of(&terminal, 11, "confirm");
    assert_style(&terminal, label, 11, on_tint(theme.secondary()));
}

#[test]
fn the_cursor_moves_the_prompt_and_bold() {
    let (mut f, request) = mock_c();
    f.app.bottom.approval.cursor = 3;
    f.app.bottom.approval.cursor_on = Some(request.id);
    let terminal = paint(&f.app, &request, 120);
    let rows = rows(&terminal);
    assert!(rows[6].starts_with("  1. "));
    assert!(rows[9].starts_with("› 4. "));
}

#[test]
fn ascii_mode_swaps_every_glyph() {
    let (mut f, request) = mock_c();
    f.app.theme = Theme::resolve(ThemeSpec {
        ascii: true,
        ..ThemeSpec::default()
    });
    let terminal = paint(&f.app, &request, 120);
    let rows = rows(&terminal);
    assert_eq!(
        rows[2],
        "  tester [verifier] | child of implementer | haiku-4.5 low"
    );
    assert_eq!(rows[6], "> 1. Yes, run it (y)");
    assert_eq!(
        rows[7],
        "  2. Yes, and allow cargo test -p aether --test cli_runs -- --nocapture for tester and its subagents (a)"
    );
    assert_eq!(
        rows[11],
        "  enter confirm | up/down select | o open tester  |  1 more waiting (implementer)"
    );
    assert!(rows.iter().all(|r| r.is_ascii()), "{rows:#?}");
}

#[test]
fn long_commands_wrap_to_three_rows_at_80() {
    let (mut f, _) = mock_c();
    let long = [COMMAND; 6].join(" && ");
    let request = shell_approval(f.tester, &long, at(-3_000));
    f.app.approvals.push(request.clone());
    let terminal = paint(&f.app, &request, 80);
    let rows = rows(&terminal);
    assert_eq!(
        usize::from(desired_height(&request, &f.app, 80)),
        rows.len()
    );
    assert!(rows[4].starts_with("  $ cargo test"));
    assert!(
        rows[5].starts_with("    "),
        "continuation aligns under the command"
    );
    assert!(rows[6].ends_with('…'), "{:?}", rows[6]);
    assert_eq!(rows[7], "");
    assert!(rows.iter().all(|r| r.width() <= 80));
    assert_eq!(
        rows[13], "  enter confirm · ↑↓ select · e show all · o open tester  ·  2 more waiting",
        "names drop first when the footer is short of room"
    );
}

#[test]
fn e_unfolds_a_cut_command_in_the_card_without_answering_it() {
    let (mut f, _) = mock_c();
    f.app.size = (80, 40);
    f.app.approvals.clear();
    let long = [COMMAND; 6].join(" && ");
    let request = shell_approval(f.tester, &long, at(-3_000));
    f.app.approvals.push(request.clone());
    crate::app::approval_flow::fixtures::read_card(&mut f.app);
    let folded = rows(&paint(&f.app, &request, 80));

    let effects = f.app.dispatch_key(crate::app::approval_flow::fixtures::key(
        crossterm::event::KeyCode::Char('e'),
    ));
    assert!(effects.is_empty(), "e only unfolds: {effects:?}");
    assert!(f.app.resolving.is_empty());
    let unfolded = rows(&paint(&f.app, &request, 80));
    assert!(unfolded.len() > folded.len());
    let squash = |text: &str| text.split_whitespace().collect::<String>();
    let body: String = unfolded[4..]
        .iter()
        .take_while(|r| !r.is_empty())
        .map(|r| squash(r))
        .collect();
    assert_eq!(body, format!("${}", squash(&long)), "{unfolded:#?}");
    assert!(!unfolded.iter().any(|r| r.ends_with('…')), "{unfolded:#?}");
    let footer = unfolded
        .iter()
        .find(|r| r.contains("enter confirm"))
        .unwrap();
    assert!(footer.contains("e fold"), "{footer}");

    f.app.dispatch_key(crate::app::approval_flow::fixtures::key(
        crossterm::event::KeyCode::Char('e'),
    ));
    assert_eq!(rows(&paint(&f.app, &request, 80)), folded);
}

#[test]
fn a_command_that_fits_offers_no_unfold_key() {
    let (f, request) = mock_c();
    let rows = rows(&paint(&f.app, &request, 120));
    assert!(!rows.iter().any(|r| r.contains("show all")), "{rows:#?}");
}

#[test]
fn file_and_network_requests_get_their_own_titles() {
    let mut f = fleet();
    let mut write = shell_approval(f.tester, "src/lib.rs", at(0));
    write.tool = "write_file".to_owned();
    write.resources = vec![aether_core::tool::ResourceKey::file("src/lib.rs")];
    let mut fetch = shell_approval(f.tester, "https://example.com", at(0));
    fetch.tool = "web_fetch".to_owned();
    f.app.approvals = vec![write.clone()];

    let rows_of = |app: &App, r: &ApprovalRequest| rows(&paint(app, r, 120));
    let write_rows = rows_of(&f.app, &write);
    assert_eq!(write_rows[1], "  Allow tester to change this file?");
    assert_eq!(write_rows[4], "  src/lib.rs");
    assert_eq!(write_rows[6], "› 1. Yes, allow it (y)");
    assert_eq!(
        rows_of(&f.app, &fetch)[1],
        "  Allow tester to reach https://example.com?"
    );
}

/// Rule subjects use the row's room, not a fixed 24 columns, and a URL
/// spends it on host and path rather than its scheme.
#[test]
fn rule_subjects_use_the_room_the_row_has() {
    let mut f = fleet();
    let url = "https://example.com/changelog/2026/october";
    let mut fetch = shell_approval(f.tester, url, at(0));
    fetch.tool = "web_fetch".to_owned();
    f.app.approvals = vec![fetch.clone()];
    let wide = rows(&paint(&f.app, &fetch, 120));
    assert_eq!(
        wide[7],
        "  2. Yes, and allow example.com/changelog/2026/october for tester and its subagents (a)"
    );
    assert_eq!(
        wide[8],
        "  3. Always allow example.com/changelog/2026/october everywhere (p)"
    );

    let narrow = rows(&paint(&f.app, &fetch, 70));
    assert_eq!(
        narrow[7],
        "  2. Yes, and allow example.com/chan… for tester and its subagents (a)"
    );
    assert!(narrow.iter().all(|r| r.width() <= 70), "{narrow:#?}");
}

#[test]
fn guard_and_cursor_bookkeeping() {
    let mut view = ApprovalView::default();
    let id = aether_core::id::ApprovalId::new();
    assert!(view.guard_allows(at(0), at(0)), "no typing yet");
    view.note_key(at(0));
    assert!(!view.guard_allows(at(100), at(999)));
    assert!(
        view.guard_allows(at(-1), at(500)),
        "arrived before the burst"
    );
    assert_eq!(view.cursor_for(id), 0);
    view.cursor = 9;
    view.cursor_on = Some(id);
    assert_eq!(view.cursor_for(id), OPTION_COUNT - 1);
}

#[test]
fn wrap_rows_marks_the_cut_and_keeps_short_text() {
    assert_eq!(wrap_rows("abc", 10, 3, '…'), ["abc"]);
    assert_eq!(wrap_rows("", 10, 3, '…'), [""]);
    assert_eq!(wrap_rows("abcdefgh", 3, 2, '…'), ["abc", "de…"]);
    assert_eq!(wrap_rows("a\nb\nc\nd", 5, 3, '…'), ["a", "b", "c…"]);
}

#[test]
fn approval_decision_hint_is_visible_and_distinct_at_narrow_widths() {
    let (mut f, request) = mock_c();
    for ascii in [false, true] {
        f.app.theme = Theme::resolve(ThemeSpec {
            ascii,
            ..ThemeSpec::default()
        });
        for width in [59, 80, 120] {
            let terminal = paint(&f.app, &request, width);
            let text = rows(&terminal);
            let y = text
                .iter()
                .position(|row| row.contains("Esc deny"))
                .unwrap() as u16;
            assert!(text[y as usize].contains("Ctrl+C defer"));
            assert_style(
                &terminal,
                2,
                y,
                f.app
                    .theme
                    .tint()
                    .patch(f.app.theme.failure().patch(f.app.theme.title())),
            );
        }
    }
}
