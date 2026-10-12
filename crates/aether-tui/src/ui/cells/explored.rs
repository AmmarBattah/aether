//! Cell family: explored: merged `• Exploring`/`Explored` with `└ Read a, b` / `Search q in path` / `List dir`. Owner: W2.
//! Handles `CellModel::Explored`.

use ratatui::text::Line;

use super::text::{Spans, child_prefix, clean, plain, span, truncate, wrap};
use super::{Tone, bullet};
use crate::app::transcript::{CellModel, ExploreItem, ExploreVerb};
use crate::ui::theme::tokens::{Theme, ToolKind};

pub fn display_lines(cell: &CellModel, width: u16, theme: &Theme) -> Vec<Line<'static>> {
    lines(cell, usize::from(width), theme, false)
}

/// Details wrap every row instead of cutting it, so no name is hidden.
pub fn detail_lines(cell: &CellModel, width: u16, theme: &Theme) -> Vec<Line<'static>> {
    lines(cell, usize::from(width), theme, true)
}

pub fn plain_lines(cell: &CellModel, width: u16) -> Vec<String> {
    plain(&display_lines(cell, width, &super::plain_theme()))
}

fn lines(cell: &CellModel, width: usize, theme: &Theme, detail: bool) -> Vec<Line<'static>> {
    let CellModel::Explored { items, active } = cell else {
        return Vec::new();
    };
    let (tone, word) = if *active {
        (Tone::Active, "Exploring")
    } else {
        (Tone::Neutral, "Explored")
    };
    let mut rows = vec![Line::from(vec![
        bullet(theme, tone),
        span(" ", theme.text()),
        span(word, theme.tool(ToolKind::Read).patch(theme.title())),
    ])];
    for (i, item) in items.iter().enumerate() {
        let lead = vec![child_prefix(theme, i == 0)];
        let body = item_spans(item, theme);
        if detail {
            rows.extend(wrap(body, width, lead, vec![child_prefix(theme, false)]));
        } else {
            let mut row = lead;
            row.extend(body);
            rows.push(Line::from(truncate(row, width, theme)));
        }
    }
    rows
}

/// `Read a.rs, b.rs` / `Search q in path` / `List dir`, the verb in the read hue.
fn item_spans(item: &ExploreItem, theme: &Theme) -> Spans {
    let verb = match item.verb {
        ExploreVerb::Read => "Read",
        ExploreVerb::Search => "Search",
        ExploreVerb::List => "List",
    };
    let mut spans = vec![
        span(verb, theme.tool(ToolKind::Read)),
        span(" ", theme.text()),
    ];
    spans.push(span(clean(&item.targets.join(", ")), theme.command()));
    if let Some(scope) = &item.scope {
        spans.push(span(" in ", theme.secondary()));
        spans.push(span(clean(scope), theme.command()));
    }
    spans
}
