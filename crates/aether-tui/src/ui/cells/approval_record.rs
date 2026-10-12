//! Cell family: approval record: `✓ You approved cargo test · saved for aether` / `✗ You denied …`. Owner: W2.
//! Handles `CellModel::ApprovalRecord`.

use ratatui::text::Line;

use super::text::{clean, plain, span, truncate, wrap};
use crate::app::transcript::{ApprovalScope, CellModel};
use crate::ui::theme::tokens::Theme;

pub fn display_lines(cell: &CellModel, width: u16, theme: &Theme) -> Vec<Line<'static>> {
    lines(cell, usize::from(width), theme, false)
}

pub fn detail_lines(cell: &CellModel, width: u16, theme: &Theme) -> Vec<Line<'static>> {
    lines(cell, usize::from(width), theme, true)
}

pub fn plain_lines(cell: &CellModel, width: u16) -> Vec<String> {
    plain(&display_lines(cell, width, &super::plain_theme()))
}

fn lines(cell: &CellModel, width: usize, theme: &Theme, detail: bool) -> Vec<Line<'static>> {
    let CellModel::ApprovalRecord {
        approved,
        what,
        scope,
        ..
    } = cell
    else {
        return Vec::new();
    };
    let g = theme.glyphs();
    let (glyph, verb) = if *approved {
        (span(g.done, theme.success()), "approved")
    } else {
        (span(g.failed, theme.failure()), "denied")
    };
    let scope = match scope {
        ApprovalScope::Once => String::new(),
        ApprovalScope::Agent { name } => format!(" {} saved for {name}", g.dot),
        ApprovalScope::Run { id } => format!(" {} saved for run {id}", g.dot),
        ApprovalScope::Subtree { root } => format!(" {} saved for {root} and its subagents", g.dot),
        ApprovalScope::Everywhere => format!(" {} always, everywhere", g.dot),
    };
    let what = clean(what);
    let what = if detail {
        what
    } else {
        what.lines().next().unwrap_or_default().to_owned()
    };
    let row = vec![
        glyph,
        span(" ", theme.text()),
        span(format!("You {verb} {what}{scope}"), theme.secondary()),
    ];
    if detail {
        wrap(row, width, Vec::new(), vec![span("  ", theme.text())])
    } else {
        vec![Line::from(truncate(row, width, theme))]
    }
}
