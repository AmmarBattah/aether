//! Needs-you strip and the queued/steering previews (SPEC §1.1 items 1–2). Owner: W5.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_width::UnicodeWidthStr;

use super::approval::ellipsis;
use crate::app::App;
use crate::app::approval_flow::{NeedsYou, NeedsYouKind};
use crate::format;
use crate::ui::theme::tokens::Theme;

/// At most this many needs-you rows, then `+N more`.
pub const NEEDS_YOU_MAX_ROWS: u16 = 2;
/// Previews shown per group (steering, queued) before `+N more`.
const PREVIEW_MAX_ITEMS: usize = 3;
const STEERING_CAPTION: &str = "steering · lands at the next tool call";
const QUEUED_CAPTION: &str = "queued for after this turn · ";
const EDIT_KEY: &str = "⇧←";
const EDIT_LABEL: &str = " edit";

pub fn needs_you_height(app: &App) -> u16 {
    let rows = needs_you_rows(app).len();
    u16::try_from(rows)
        .unwrap_or(u16::MAX)
        .min(NEEDS_YOU_MAX_ROWS)
}

pub fn render_needs_you(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let rows = needs_you_rows(app);
    let shown = rows.len().min(usize::from(NEEDS_YOU_MAX_ROWS));
    let hidden = rows.len() - shown;
    let lines: Vec<Line<'static>> = rows[..shown]
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let more = if i + 1 == shown { hidden } else { 0 };
            needs_you_line(row, more, way_there(app, row), area.width, theme, app)
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), area);
}

/// `↪ text` + `steering · lands at the next tool call` / `queued for after this turn · ⇧← edit`.
pub fn pending_height(app: &App, width: u16) -> u16 {
    let rows = pending_lines(app, width, &app.theme).len();
    u16::try_from(rows).unwrap_or(u16::MAX)
}

pub fn render_pending(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    frame.render_widget(Paragraph::new(pending_lines(app, area.width, theme)), area);
}

/// Everyone waiting on the operator except the viewed agent's own question,
/// which its transcript already shows.
fn needs_you_rows(app: &App) -> Vec<NeedsYou> {
    // §1.1: an approval on screen is the modal, so the strip steps aside.
    if app.shown_approval().is_some() {
        return Vec::new();
    }
    app.needs_you()
        .into_iter()
        .filter(|row| {
            Some(row.agent) != app.selected || !matches!(row.what, NeedsYouKind::Question { .. })
        })
        .collect()
}

/// How a needs-you row is reached from the screen as it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WayThere {
    Resize,
    /// The board opens it.
    Board,
    /// A view or popup holds the pane; esc first.
    Esc,
    /// Held by the typing guard; the card comes up on its own.
    Pause,
}

fn way_there(app: &App, row: &NeedsYou) -> WayThere {
    if app.approval_too_short() {
        return WayThere::Resize;
    }
    if app.bottom_covered() {
        return WayThere::Esc;
    }
    match row.what {
        NeedsYouKind::Approval { id, .. } if !app.bottom.approval.deferred.contains(&id) => {
            WayThere::Pause
        }
        _ => WayThere::Board,
    }
}

/// `◆ tester asks: Should --json include imported runs?  ·  ctrl+g to open`.
fn needs_you_line(
    row: &NeedsYou,
    more: usize,
    way: WayThere,
    width: u16,
    theme: &Theme,
    app: &App,
) -> Line<'static> {
    let glyphs = theme.glyphs();
    let sep = format!("  {}  ", glyphs.dot);
    let (verb, text) = match &row.what {
        NeedsYouKind::Question { first_line } => ("asks", first_line.as_str()),
        NeedsYouKind::Approval { summary, .. } => ("wants approval", summary.as_str()),
    };
    let lead = format!("{} {verb}: ", row.name);
    let mut tail = vec![Span::styled(sep.clone(), theme.secondary())];
    if more > 0 {
        tail.push(Span::styled(format!("+{more} more"), theme.secondary()));
        tail.push(Span::styled(sep, theme.secondary()));
    }
    match (&row.what, way) {
        (_, WayThere::Resize) => {
            tail.push(Span::styled("resize to show approval", theme.secondary()));
        }
        (_, WayThere::Pause) => {
            tail.push(Span::styled(
                "appears when you pause typing",
                theme.secondary(),
            ));
        }
        (NeedsYouKind::Approval { .. }, WayThere::Esc) => {
            tail.push(Span::styled("esc", theme.title()));
            tail.push(Span::styled(" to answer", theme.secondary()));
        }
        (NeedsYouKind::Question { .. }, WayThere::Esc) => {
            tail.push(Span::styled("esc", theme.title()));
            tail.push(Span::styled(", then ", theme.secondary()));
            tail.push(Span::styled(
                app.keys
                    .display(crate::keys::KeyAction::Agents, theme.ascii()),
                theme.title(),
            ));
        }
        (_, WayThere::Board) => {
            tail.push(Span::styled(
                app.keys
                    .display(crate::keys::KeyAction::Agents, theme.ascii()),
                theme.title(),
            ));
            tail.push(Span::styled(" to open", theme.secondary()));
        }
    }
    let fixed = glyphs.needs_you.width() + 1 + lead.width() + spans_width(&tail);
    let room = usize::from(width).saturating_sub(fixed).max(1);
    let mut spans = vec![
        Span::styled(glyphs.needs_you.to_owned(), theme.accent()),
        Span::raw(" "),
        Span::styled(lead, theme.text()),
        Span::styled(
            format::one_line_with_marker(text, room, ellipsis(theme)),
            theme.text(),
        ),
    ];
    spans.extend(tail);
    Line::from(spans)
}

/// Steering first (it lands sooner), then the queue; each group ends with its caption.
fn pending_lines(app: &App, width: u16, theme: &Theme) -> Vec<Line<'static>> {
    let Some(transcript) = app.selected.and_then(|a| app.transcripts.get(&a)) else {
        return Vec::new();
    };
    let mut lines = Vec::new();
    let steering: Vec<&str> = transcript
        .steering
        .iter()
        .map(|s| s.text.as_str())
        .collect();
    push_group(&mut lines, &steering, width, theme, steering_caption(theme));
    let queued: Vec<&str> = transcript.queue.iter().map(String::as_str).collect();
    push_group(
        &mut lines,
        &queued,
        width,
        theme,
        queued_caption(theme, app),
    );
    // Mock a: one blank row before the status row below.
    if !lines.is_empty() {
        lines.push(Line::default());
    }
    lines
}

fn push_group(
    lines: &mut Vec<Line<'static>>,
    texts: &[&str],
    width: u16,
    theme: &Theme,
    caption: Vec<Span<'static>>,
) {
    if texts.is_empty() {
        return;
    }
    let glyph = theme.glyphs().queued;
    let room = usize::from(width)
        .saturating_sub(2 + glyph.width() + 1)
        .max(1);
    for text in texts.iter().take(PREVIEW_MAX_ITEMS) {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(glyph.to_owned(), theme.secondary()),
            Span::raw(" "),
            Span::styled(
                format::one_line_with_marker(text, room, ellipsis(theme)),
                theme.text(),
            ),
        ]));
    }
    let mut row = vec![Span::raw("    ")];
    let hidden = texts.len().saturating_sub(PREVIEW_MAX_ITEMS);
    if hidden > 0 {
        row.push(Span::styled(
            format!("+{hidden} more {} ", theme.glyphs().dot),
            theme.secondary(),
        ));
    }
    row.extend(caption);
    lines.push(Line::from(row));
}

fn steering_caption(theme: &Theme) -> Vec<Span<'static>> {
    vec![Span::styled(
        STEERING_CAPTION.replace('·', theme.glyphs().dot),
        theme.secondary(),
    )]
}

fn queued_caption(theme: &Theme, app: &App) -> Vec<Span<'static>> {
    let key = if theme.ascii() {
        "shift+left"
    } else {
        EDIT_KEY
    };
    vec![
        Span::styled(
            QUEUED_CAPTION.replace('·', theme.glyphs().dot),
            theme.secondary(),
        ),
        Span::styled(
            if app.keys.customised(crate::keys::KeyAction::Unqueue) {
                app.keys
                    .display(crate::keys::KeyAction::Unqueue, theme.ascii())
            } else {
                key.to_owned()
            },
            theme.title(),
        ),
        Span::styled(EDIT_LABEL, theme.secondary()),
    ]
}

fn spans_width(spans: &[Span<'_>]) -> usize {
    spans.iter().map(|s| s.content.width()).sum()
}

#[cfg(test)]
mod tests;
