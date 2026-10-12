use aether_core::agent::{AgentStatus, WaitCondition};
use aether_core::id::TaskId;
use aether_core::model::Effort;

use super::*;
use crate::app::approval_flow::fixtures::{
    Fleet, add_child, agent, assert_style, at, column_of, draw, fleet, node_mut, rows,
    shell_approval,
};
use crate::ui::theme::tokens::ThemeSpec;

/// The viewed root's descendants: one needs you (its approval deferred), two
/// run, two wait (task, inbox).
fn strip_fleet() -> Fleet {
    let mut f = fleet();
    let root = f.root;
    let docs = agent("docs", "writer", Some(root), "sonnet-5.5", Effort::Low);
    let docs = add_child(&mut f.app, root, docs);
    let notes = agent("notes", "writer", Some(root), "sonnet-5.5", Effort::Low);
    let notes = add_child(&mut f.app, root, notes);
    let setup = [
        (f.researcher, -700_000, -41_000),
        (f.implementer, -650_000, -120_000),
        (f.tester, -600_000, -5_000),
        (docs, -500_000, -12_000),
        (notes, -400_000, -3_000),
    ];
    for (id, created, since) in setup {
        let a = node_mut(&mut f.app, id);
        a.created_at = at(created);
        a.status_since = Some(at(since));
    }
    node_mut(&mut f.app, docs).status = AgentStatus::Waiting {
        condition: WaitCondition::Task { id: TaskId::new() },
    };
    node_mut(&mut f.app, notes).status = AgentStatus::Waiting {
        condition: WaitCondition::Inbox,
    };
    let approval = shell_approval(f.tester, "cargo test", at(-5_000));
    f.app.bottom.approval.deferred.insert(approval.id);
    f.app.approvals = vec![approval];
    f
}

fn line_text(line: &Line<'_>) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}

/// M2: the needs-you strip owns who needs you, so the agents strip never
/// repeats it, and the card on screen hides the strip altogether.
#[test]
fn needs_you_is_left_to_its_strip_and_running_ranks_before_waiting() {
    let mut f = strip_fleet();
    let order: Vec<(String, BoardStatus)> = items(&f.app)
        .into_iter()
        .map(|i| (i.name, i.status))
        .collect();
    let want = [
        ("researcher", BoardStatus::Running),
        ("implementer", BoardStatus::Running),
        ("docs", BoardStatus::Waiting),
        ("notes", BoardStatus::Idle),
    ]
    .map(|(name, status)| (name.to_owned(), status));
    assert_eq!(order, want);
    assert!(f.app.needs_you().iter().any(|row| row.agent == f.tester));

    f.app.bottom.approval.deferred.clear();
    assert!(f.app.shown_approval().is_some());
    assert!(items(&f.app).is_empty(), "the card is up");
    assert_eq!(height(&f.app), 0);
}

#[test]
fn the_strip_collapses_its_tail_at_120_50_and_4() {
    let f = strip_fleet();
    let items = items(&f.app);
    let theme = f.app.theme;
    let at_width = |w| line_text(&fit(&items, w, f.app.now, &theme));
    assert_eq!(
        at_width(120),
        "● researcher 41s · ● implementer 2m 00s · ◌ docs 12s · ○ notes idle"
    );
    assert_eq!(at_width(50), "● researcher 41s · ● implementer 2m 00s · +2");
    assert_eq!(at_width(4), "+4");
    assert_eq!(at_width(1), "");
}

#[test]
fn the_strip_paints_with_theme_tokens() {
    let f = strip_fleet();
    let terminal = draw(120, 1, |frame| {
        render(frame, frame.area(), &f.app, &f.app.theme)
    });
    let theme = f.app.theme;
    // No probed palette, so the running glyph holds still in its hue.
    assert_style(&terminal, 0, 0, theme.running());
    assert_style(&terminal, 2, 0, theme.running());
    let docs = column_of(&terminal, 0, "◌");
    assert_style(&terminal, docs, 0, theme.waiting());
    assert_style(&terminal, docs + 2, 0, theme.text());
    let age = column_of(&terminal, 0, "41s");
    assert_style(&terminal, age, 0, theme.secondary());
}

/// M10: quota exhaustion is never quiet, and reads as the board says it.
#[test]
fn a_quota_block_is_red_and_worded_as_on_the_board() {
    let mut f = strip_fleet();
    node_mut(&mut f.app, f.researcher).status = AgentStatus::Blocked {
        reason: aether_core::agent::BlockReason::Exhausted {
            model: aether_core::model::ModelRef::new("anthropic", "opus-5.5"),
            class: aether_core::agent::ExhaustionClass::QuotaExhausted,
            since: at(-60_000),
            reported_reset: None,
            next_probe: at(60_000),
            probes: 1,
            detail: String::new(),
        },
    };
    let terminal = draw(120, 1, |frame| {
        render(frame, frame.area(), &f.app, &f.app.theme)
    });
    let row = rows(&terminal).remove(0);
    assert!(row.contains("researcher blocked · quota"), "{row}");
    let theme = f.app.theme;
    let glyph = column_of(&terminal, 0, "researcher") - 2;
    assert_style(&terminal, glyph, 0, theme.failure());
    let detail = column_of(&terminal, 0, "blocked");
    assert_style(&terminal, detail, 0, theme.failure());
}

#[test]
fn ascii_mode_uses_the_ascii_glyphs() {
    let mut f = strip_fleet();
    f.app.theme = Theme::resolve(ThemeSpec {
        ascii: true,
        ..ThemeSpec::default()
    });
    let terminal = draw(80, 1, |frame| {
        render(frame, frame.area(), &f.app, &f.app.theme)
    });
    assert_eq!(
        rows(&terminal),
        ["* researcher 41s | * implementer 2m 00s | ~ docs 12s | o notes idle"]
    );
}

#[test]
fn shown_only_while_the_viewed_agent_has_live_descendants() {
    let mut f = strip_fleet();
    assert_eq!(height(&f.app), 1);
    f.app.selected = Some(f.tester);
    assert_eq!(height(&f.app), 0, "a leaf has no strip");
    f.app.selected = Some(f.root);
    let ids: Vec<_> = descendants(&f.app.tree, f.root)
        .iter()
        .map(|n| n.agent.id)
        .collect();
    for id in ids {
        node_mut(&mut f.app, id).status = AgentStatus::Completed { summary: None };
    }
    assert_eq!(height(&f.app), 0, "finished children leave the strip");
}
