use aether_core::id::AgentId;
use aether_core::model::Effort;

use super::*;
use crate::app::approval_flow::fixtures::{
    Fleet, add_child, agent, assert_style, at, column_of, draw, fleet, push_cell, rows,
    shell_approval,
};
use crate::app::transcript::{CellModel, Steer};
use crate::ui::theme::tokens::ThemeSpec;

fn ask(f: &mut Fleet, asker: AgentId, name: &str, text: &str, at_ms: i64) {
    let cell = CellModel::Question {
        asker,
        asker_name: name.to_owned(),
        text: text.to_owned(),
        state: crate::app::transcript::QuestionState::Pending,
        seq: 0,
    };
    push_cell(&mut f.app, asker, at(at_ms), cell);
}

fn needs_you_screen(f: &Fleet, width: u16) -> ratatui::Terminal<ratatui::backend::TestBackend> {
    let height = needs_you_height(&f.app);
    draw(width, height, |frame| {
        render_needs_you(frame, frame.area(), &f.app, &f.app.theme)
    })
}

#[test]
fn needs_you_matches_mock_b_and_paints_tokens() {
    let mut f = fleet();
    let tester = f.tester;
    ask(
        &mut f,
        tester,
        "tester",
        "Should --json include imported runs?\nDetails…",
        -10_000,
    );
    let terminal = needs_you_screen(&f, 120);
    assert_eq!(
        rows(&terminal),
        ["◆ tester asks: Should --json include imported runs?  ·  ctrl+g to open"]
    );
    let theme = f.app.theme;
    assert_style(&terminal, 0, 0, theme.accent());
    assert_style(&terminal, 2, 0, theme.text());
    let key = column_of(&terminal, 0, "ctrl+g");
    assert_style(&terminal, key, 0, theme.title());
    assert_style(&terminal, key + 6, 0, theme.secondary());
}

#[test]
fn needs_you_shows_two_rows_then_counts_the_rest() {
    let mut f = fleet();
    let (tester, researcher, implementer, root) = (f.tester, f.researcher, f.implementer, f.root);
    ask(&mut f, tester, "tester", "Include imported runs?", -30_000);
    ask(
        &mut f,
        researcher,
        "researcher",
        "Which crate owns RunListing?",
        -20_000,
    );
    ask(
        &mut f,
        root,
        "aether",
        "My own question stays in my transcript",
        -25_000,
    );
    let deferred = shell_approval(implementer, "cargo build --release", at(-10_000));
    f.app.bottom.approval.deferred.insert(deferred.id);
    f.app.approvals = vec![deferred];

    assert_eq!(needs_you_height(&f.app), NEEDS_YOU_MAX_ROWS);
    assert_eq!(
        rows(&needs_you_screen(&f, 120)),
        [
            "◆ tester asks: Include imported runs?  ·  ctrl+g to open",
            "◆ researcher asks: Which crate owns RunListing?  ·  +1 more  ·  ctrl+g to open",
        ]
    );
    let narrow = rows(&needs_you_screen(&f, 60));
    assert_eq!(
        narrow[1],
        "◆ researcher asks: Which cra…  ·  +1 more  ·  ctrl+g to open"
    );

    f.app.theme = Theme::resolve(ThemeSpec {
        ascii: true,
        ..ThemeSpec::default()
    });
    assert_eq!(
        rows(&needs_you_screen(&f, 80))[0],
        "! tester asks: Include imported runs?  |  ctrl+g to open"
    );
}

#[test]
fn deferred_approvals_read_as_wanting_approval() {
    let mut f = fleet();
    let extra = agent("docs", "writer", Some(f.root), "sonnet-5.5", Effort::Low);
    let docs = add_child(&mut f.app, f.root, extra);
    let deferred = shell_approval(docs, "rm -rf target/doc", at(-1_000));
    f.app.bottom.approval.deferred.insert(deferred.id);
    f.app.approvals = vec![deferred];
    assert_eq!(
        rows(&needs_you_screen(&f, 80)),
        ["◆ docs wants approval: rm -rf target/doc  ·  ctrl+g to open"]
    );
}

/// M2.4: an approval the typing guard holds back is named, and says when it comes.
#[test]
fn an_approval_held_by_typing_says_it_appears_on_a_pause() {
    let mut f = fleet();
    f.app.now = at(0);
    f.app.bottom.approval.note_key(at(0));
    f.app.approvals = vec![shell_approval(f.tester, "cargo test", at(100))];
    f.app.now = at(200);
    assert!(f.app.due_approval().is_none(), "held while typing");
    assert_eq!(
        rows(&needs_you_screen(&f, 100)),
        ["◆ tester wants approval: cargo test  ·  appears when you pause typing"]
    );
    f.app.now = at(5_000);
    assert_eq!(needs_you_height(&f.app), 0, "the card itself is up");
}

/// M2: one blocked child is one row on screen, not three.
#[test]
fn a_deferred_child_approval_shows_once_on_screen() {
    let mut f = fleet();
    f.app.size = (120, 40);
    let deferred = shell_approval(f.tester, "cargo test", at(-5_000));
    f.app.bottom.approval.deferred.insert(deferred.id);
    f.app.approvals = vec![deferred];
    let screen = crate::testing::render_to(&f.app, 120, 40);
    assert_eq!(screen.matches("tester").count(), 1, "{screen}");
    assert!(
        screen.contains("◆ tester wants approval: cargo test"),
        "{screen}"
    );
    assert!(!screen.contains("needs you"), "{screen}");

    // Too short for the strip: the status row's count is all that is left.
    let short = crate::testing::render_to(&f.app, 120, 8);
    assert!(!short.contains("wants approval"), "{short}");
    assert!(short.contains("◆ 1 needs you"), "{short}");
}

fn pending_screen(f: &Fleet, width: u16) -> ratatui::Terminal<ratatui::backend::TestBackend> {
    let height = pending_height(&f.app, width);
    draw(width, height, |frame| {
        render_pending(frame, frame.area(), &f.app, &f.app.theme)
    })
}

#[test]
fn queued_preview_matches_mock_a() {
    let mut f = fleet();
    let transcript = f.app.transcripts.entry(f.root).or_default();
    transcript
        .queue
        .push("Also add the flag to CHANGELOG.md under Unreleased.".to_owned());
    let terminal = pending_screen(&f, 120);
    assert_eq!(
        rows(&terminal),
        [
            "  ↪ Also add the flag to CHANGELOG.md under Unreleased.",
            "    queued for after this turn · ⇧← edit",
            "",
        ]
    );
    let theme = f.app.theme;
    assert_style(&terminal, 2, 0, theme.secondary());
    assert_style(&terminal, 4, 0, theme.text());
    assert_style(&terminal, 4, 1, theme.secondary());
    let key = column_of(&terminal, 1, "⇧←");
    assert_style(&terminal, key, 1, theme.title());
}

#[test]
fn steering_comes_first_and_long_groups_fold() {
    let mut f = fleet();
    let transcript = f.app.transcripts.entry(f.root).or_default();
    transcript.steering.push(Steer {
        text: "Use serde_json::to_writer, not format!".to_owned(),
        sent_at: at(-1_000),
    });
    for i in 1..=4 {
        transcript.queue.push(format!("queued {i}\nsecond line"));
    }
    assert_eq!(
        pending_height(&f.app, 80),
        2 + 4 + 1,
        "a blank row closes the previews"
    );
    assert_eq!(
        rows(&pending_screen(&f, 80)),
        [
            "  ↪ Use serde_json::to_writer, not format!",
            "    steering · lands at the next tool call",
            "  ↪ queued 1",
            "  ↪ queued 2",
            "  ↪ queued 3",
            "    +1 more · queued for after this turn · ⇧← edit",
            "",
        ]
    );
    let narrow = rows(&pending_screen(&f, 30));
    assert_eq!(narrow[0], "  ↪ Use serde_json::to_writer…");

    f.app.theme = Theme::resolve(ThemeSpec {
        ascii: true,
        ..ThemeSpec::default()
    });
    let ascii = rows(&pending_screen(&f, 80));
    assert_eq!(ascii[1], "    steering | lands at the next tool call");
    assert_eq!(
        ascii[5],
        "    +1 more | queued for after this turn | shift+left edit"
    );
    assert!(ascii.iter().all(|r| r.is_ascii()));
}

#[test]
fn nothing_pending_takes_no_rows() {
    let f = fleet();
    assert_eq!(pending_height(&f.app, 80), 0);
    assert_eq!(needs_you_height(&f.app), 0);
}
