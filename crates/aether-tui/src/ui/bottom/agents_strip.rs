//! The agents strip: one line above the status row while the viewed agent has
//! live descendants (`● researcher 41s · ◌ docs 12s · +2`). Owner: W5.

use std::collections::BTreeSet;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_width::UnicodeWidthStr;

use aether_core::id::AgentId;
use aether_core::time::Timestamp;
use aether_protocol::{AgentTree, TreeNode};

use super::status_row::{PULSE_PERIOD_MS, epoch_ms, pulse_phase};
use crate::app::App;
use crate::app::board::{self, BoardStatus};
use crate::format;
use crate::ui::theme::tokens::Theme;

#[derive(Debug, Clone, PartialEq)]
pub struct StripItem {
    pub id: AgentId,
    pub name: String,
    pub status: BoardStatus,
    pub since: Option<Timestamp>,
}

/// Live descendants of the viewed agent, running before waiting, ties by
/// creation order. Agents that need you are the needs-you strip's, and an
/// approval card on screen hides the strip altogether (SPEC §1.1).
pub fn items(app: &App) -> Vec<StripItem> {
    let Some(viewed) = app.selected else {
        return Vec::new();
    };
    if app.shown_approval().is_some() {
        return Vec::new();
    }
    let needs_you = app.needs_you_agents();
    let mut nodes: Vec<(&TreeNode, BoardStatus)> = descendants(&app.tree, viewed)
        .into_iter()
        .filter_map(|n| strip_status(n, &app.tree, &needs_you).map(|s| (n, s)))
        .filter(|(_, status)| *status != BoardStatus::NeedsYou)
        .collect();
    nodes.sort_by_key(|(n, status)| (rank(*status), n.agent.created_at, n.agent.id));
    let names = crate::app::sibling_names(&app.tree);
    nodes
        .into_iter()
        .map(|(n, status)| StripItem {
            id: n.agent.id,
            name: names
                .get(&n.agent.id)
                .cloned()
                .unwrap_or_else(|| n.agent.name.clone()),
            status,
            since: n.agent.status_since,
        })
        .collect()
}

/// Fit `items` into `width` columns, collapsing the tail into `+N`.
pub fn fit(items: &[StripItem], width: u16, now: Timestamp, theme: &Theme) -> Line<'static> {
    let width = usize::from(width);
    let parts: Vec<Vec<Span<'static>>> = items.iter().map(|i| item_spans(i, now, theme)).collect();
    let sep = format!(" {} ", theme.glyphs().dot);
    // Most items first; each step folds one more into the `+N` tail.
    for shown in (0..=parts.len()).rev() {
        let hidden = parts.len() - shown;
        let mut spans: Vec<Span<'static>> = Vec::new();
        for part in &parts[..shown] {
            if !spans.is_empty() {
                spans.push(Span::styled(sep.clone(), theme.secondary()));
            }
            spans.extend(part.iter().cloned());
        }
        if hidden > 0 {
            if !spans.is_empty() {
                spans.push(Span::styled(sep.clone(), theme.secondary()));
            }
            spans.push(Span::styled(format!("+{hidden}"), theme.secondary()));
        }
        let used: usize = spans.iter().map(|s| s.content.width()).sum();
        if used <= width {
            return Line::from(spans);
        }
    }
    Line::default()
}

/// 1 when the viewed agent has live descendants, else 0.
pub fn height(app: &App) -> u16 {
    u16::from(!items(app).is_empty())
}

pub fn render(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let line = fit(&items(app), area.width, app.now, theme);
    frame.render_widget(Paragraph::new(line), area);
}

/// Every node below `root`, depth first; `root` itself excluded.
pub(crate) fn descendants(tree: &AgentTree, root: AgentId) -> Vec<&TreeNode> {
    let mut out = Vec::new();
    let mut stack: Vec<AgentId> = tree
        .nodes
        .get(&root)
        .map(|n| n.children.iter().rev().copied().collect())
        .unwrap_or_default();
    while let Some(id) = stack.pop() {
        let Some(node) = tree.nodes.get(&id) else {
            continue;
        };
        out.push(node);
        stack.extend(node.children.iter().rev().copied());
    }
    out
}

/// Board status of a live agent; `None` for finished or archived ones.
fn strip_status(
    node: &TreeNode,
    tree: &AgentTree,
    needs_you: &BTreeSet<AgentId>,
) -> Option<BoardStatus> {
    let agent = &node.agent;
    if agent.archived || agent.status.is_terminal() {
        return None;
    }
    let live_child = node.children.iter().any(|id| {
        tree.nodes
            .get(id)
            .is_some_and(|child| child.agent.is_live())
    });
    Some(board::status_in_tree(
        agent,
        needs_you.contains(&agent.id),
        live_child,
    ))
}

fn rank(status: BoardStatus) -> u8 {
    match status {
        BoardStatus::NeedsYou => 0,
        BoardStatus::Running => 1,
        BoardStatus::Waiting => 2,
        BoardStatus::Blocked { .. } => 3,
        BoardStatus::Paused => 4,
        _ => 5,
    }
}

/// `● researcher 41s`, `◌ writer blocked`, `◌ tester blocked · quota` (red).
fn item_spans(item: &StripItem, now: Timestamp, theme: &Theme) -> Vec<Span<'static>> {
    let glyphs = theme.glyphs();
    let since = || format::elapsed_compact(elapsed_ms(item.since, now));
    let (glyph, glyph_style, detail, detail_style) = match item.status {
        BoardStatus::NeedsYou => (
            glyphs.needs_you,
            theme.attention(),
            "needs you".to_owned(),
            theme.attention(),
        ),
        BoardStatus::Running => (
            glyphs.running,
            running_style(now, theme),
            since(),
            theme.secondary(),
        ),
        BoardStatus::Waiting => (glyphs.waiting, theme.waiting(), since(), theme.secondary()),
        // Never quiet, and worded as the board words it.
        BoardStatus::Blocked { quota: true } => (
            glyphs.blocked,
            theme.failure(),
            format!("blocked {} quota", glyphs.dot),
            theme.failure(),
        ),
        BoardStatus::Blocked { quota: false } => (
            glyphs.blocked,
            theme.blocked(),
            "blocked".to_owned(),
            theme.secondary(),
        ),
        BoardStatus::Paused => (
            glyphs.paused,
            theme.paused(),
            "paused".to_owned(),
            theme.secondary(),
        ),
        _ => (
            glyphs.idle,
            theme.secondary(),
            "idle".to_owned(),
            theme.secondary(),
        ),
    };
    let name_style = match item.status {
        BoardStatus::Running => theme.running(),
        _ => theme.text(),
    };
    vec![
        Span::styled(glyph.to_owned(), glyph_style),
        Span::styled(format!(" {} ", item.name), name_style),
        Span::styled(detail, detail_style),
    ]
}

fn running_style(now: Timestamp, theme: &Theme) -> ratatui::style::Style {
    theme.pulse(pulse_phase(epoch_ms(now), PULSE_PERIOD_MS))
}

fn elapsed_ms(since: Option<Timestamp>, now: Timestamp) -> u64 {
    since.map_or(0, |s| (now - s).num_milliseconds().max(0) as u64)
}

#[cfg(test)]
mod tests;
