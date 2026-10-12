//! Cell family: reasoning: hidden in the compact view; `• Thought for 12s` and dim italic text in details. Owner: W2.
//! Handles `CellModel::Reasoning`.

use ratatui::text::Line;

use super::text::{Layout, plain, span, truncate};
use crate::app::transcript::CellModel;
use crate::format;
use crate::ui::markdown;
use crate::ui::theme::tokens::Theme;

/// Reasoning only shows in details; its title drives the status row instead.
pub fn display_lines(_cell: &CellModel, _width: u16, _theme: &Theme) -> Vec<Line<'static>> {
    Vec::new()
}

pub fn detail_lines(cell: &CellModel, width: u16, theme: &Theme) -> Vec<Line<'static>> {
    layout(cell, width, true, theme).into_lines()
}

pub fn layout(cell: &CellModel, width: u16, detail: bool, theme: &Theme) -> Layout {
    let CellModel::Reasoning {
        text, duration_ms, ..
    } = cell
    else {
        return Layout::default();
    };
    if !detail {
        return Layout::default();
    }
    let head = match duration_ms {
        Some(ms) => format!("Thought for {}", format::took(*ms)),
        None => "Thought".to_owned(),
    };
    let head = vec![
        span(format!("{} ", theme.glyphs().bullet), theme.secondary()),
        span(head, theme.secondary()),
    ];
    let mut rows = Layout::unmapped(vec![Line::from(truncate(head, usize::from(width), theme))]);
    let quiet = theme.reasoning();
    let mut body = markdown::render_layout(
        text,
        usize::from(width).saturating_sub(2),
        theme.text(),
        theme,
    );
    for (row, source) in body.rows_mut() {
        for s in &mut row.spans {
            s.style = s.style.patch(quiet);
        }
        if !row.spans.is_empty() {
            row.spans.insert(0, span("  ", theme.text()));
            source.prefix += 2;
        }
    }
    rows.append(body);
    rows
}

pub fn plain_lines(cell: &CellModel, width: u16) -> Vec<String> {
    plain(&display_lines(cell, width, &super::plain_theme()))
}
