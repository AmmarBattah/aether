//! The approval view (SPEC §3.4, mock c): replaces the composer on a tinted
//! block; the draft is kept underneath. Owner: W5.

use std::collections::{BTreeSet, HashMap};

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use aether_core::id::{AgentId, ApprovalId};
use aether_core::time::Timestamp;
use aether_core::tool::ApprovalRequest;

use crate::app::App;
use crate::app::approval_flow::{MIN_VISIBLE_MS, TYPING_GUARD_MS};
use crate::app::approvals::{ApprovalKind as Kind, kind, wants};
use crate::format;
use crate::ui::theme::tokens::Theme;

/// Option rows, in order: y, a, p, esc.
pub const OPTION_COUNT: usize = 4;
/// The command or target wraps to at most this many rows.
const BODY_MAX_ROWS: usize = 3;
/// A rule's subject keeps at least this many columns, even on a narrow card.
const SUBJECT_MIN_COLS: usize = 16;
/// Footer names at most this many other waiting agents.
const WAITING_NAMES_MAX: usize = 3;
const INDENT: &str = "  ";

/// Approval view state. Which approval is due is `App::due_approval`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ApprovalView {
    /// Highlighted option, `0..OPTION_COUNT`, for the approval in `cursor_on`.
    pub cursor: usize,
    /// The approval the cursor was moved on; any other opens on option 1.
    pub cursor_on: Option<ApprovalId>,
    /// Approvals closed with Ctrl+C (or `o`): shown in the needs-you strip instead.
    pub deferred: BTreeSet<ApprovalId>,
    /// When this client first held each pending approval: the typing guard
    /// times arrival from here, not from the daemon's `requested_at`.
    pub first_seen: HashMap<ApprovalId, Timestamp>,
    /// Whether a view or popup covered the approval slot at the last check.
    pub covered: bool,
    /// When the slot last came free; an approval waiting behind it arrives then.
    pub uncovered_at: Option<Timestamp>,
    /// The approval on screen and when it was first drawn: answers wait out [`MIN_VISIBLE_MS`].
    pub shown: Option<(ApprovalId, Timestamp)>,
    /// The approval whose body shows in full (`e`).
    pub expanded: Option<ApprovalId>,
    /// Last keystroke, for the 1 s typing guard. Write it through [`ApprovalView::note_key`].
    pub last_key_at: Option<Timestamp>,
    /// First keystroke after a full guard of idle. Approvals older than it
    /// were already on screen, so the guard no longer hides them.
    pub burst_started_at: Option<Timestamp>,
    /// Whose approval was just denied: the composer offers `Tell <name> what to do instead`.
    pub deny_target: Option<AgentId>,
    /// Approval waits already answered in each agent's current turn.
    pub paused: HashMap<AgentId, TurnPause>,
}

/// Banked approval wait for one turn; the status-row clock leaves it out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TurnPause {
    pub turn_started: Timestamp,
    pub paused_ms: u64,
}

impl ApprovalView {
    /// Record a keystroke (W6 calls this for every key).
    pub fn note_key(&mut self, at: Timestamp) {
        if self.last_key_at.is_none_or(|last| idle_for_guard(last, at)) {
            self.burst_started_at = Some(at);
        }
        self.last_key_at = Some(at);
    }

    /// Whether an approval that arrived at `arrived` may show at `now`: the
    /// operator is idle, or it arrived before the current burst of typing.
    pub fn guard_allows(&self, arrived: Timestamp, now: Timestamp) -> bool {
        let Some(last) = self.last_key_at else {
            return true;
        };
        idle_for_guard(last, now) || self.burst_started_at.is_some_and(|s| arrived < s)
    }

    /// When approval `id` could first have appeared: first held, or the slot came free.
    pub fn arrived_at(&self, id: ApprovalId, now: Timestamp) -> Timestamp {
        let seen = self.first_seen.get(&id).copied().unwrap_or(now);
        self.uncovered_at.map_or(seen, |free| seen.max(free))
    }

    /// Whether `id` has been on screen long enough for an answer key to count.
    pub fn answerable(&self, id: ApprovalId, now: Timestamp) -> bool {
        self.shown.is_some_and(|(shown, since)| {
            shown == id && (now - since).num_milliseconds() >= MIN_VISIBLE_MS
        })
    }

    /// The highlighted option for approval `id`.
    pub fn cursor_for(&self, id: ApprovalId) -> usize {
        if self.cursor_on == Some(id) {
            self.cursor.min(OPTION_COUNT - 1)
        } else {
            0
        }
    }
}

fn idle_for_guard(last: Timestamp, now: Timestamp) -> bool {
    (now - last).num_milliseconds() >= TYPING_GUARD_MS
}

pub fn desired_height(request: &ApprovalRequest, app: &App, width: u16) -> u16 {
    let rows = lines(request, 0, app, &app.theme, width).len();
    u16::try_from(rows).unwrap_or(u16::MAX)
}

pub fn render(
    frame: &mut Frame,
    area: Rect,
    request: &ApprovalRequest,
    view: &ApprovalView,
    app: &App,
    theme: &Theme,
) {
    let cursor = view.cursor_for(request.id);
    let body = lines(request, cursor, app, theme, area.width);
    frame.render_widget(Paragraph::new(body).style(theme.tint()), area);
}

/// Every row of the view at `width`, padding rows included.
fn lines(
    request: &ApprovalRequest,
    cursor: usize,
    app: &App,
    theme: &Theme,
    width: u16,
) -> Vec<Line<'static>> {
    let name = app.agent_display_name(request.agent_id);
    let kind = kind(request);
    let title = match &kind {
        Kind::Command(_) => format!("Allow {name} to run this command?"),
        Kind::File(_) => format!("Allow {name} to change this file?"),
        Kind::Network | Kind::Other => {
            let action = wants(request);
            let action = action.strip_prefix("Wants to ").unwrap_or(&action);
            let title = format!("Allow {name} to {action}?");
            format::one_line_with_marker(
                &title,
                usize::from(width).saturating_sub(2),
                ellipsis(theme),
            )
        }
    };
    let mut out = vec![
        Line::default(),
        indented(vec![Span::styled(title, theme.title())]),
    ];
    if app.selected != Some(request.agent_id) {
        out.push(indented(vec![Span::styled(
            attribution(app, request.agent_id),
            theme.secondary(),
        )]));
    }
    out.push(Line::default());
    let (prefix, text) = body_text(request, &kind);
    let room = usize::from(width).saturating_sub(INDENT.width() + prefix.width());
    let folds = wrap_rows(&text, room, usize::MAX, ellipsis(theme)).len() > BODY_MAX_ROWS;
    let max_rows = if app.bottom.approval.expanded == Some(request.id) {
        // Room for the title, options and footer stays on screen.
        usize::from(app.size.1 / 2).max(BODY_MAX_ROWS)
    } else {
        BODY_MAX_ROWS
    };
    out.extend(body_lines(prefix, &text, room, max_rows, theme));
    out.push(Line::default());
    let labels = option_labels(request, &kind, &name, width, ellipsis(theme));
    for (index, label) in labels.iter().enumerate() {
        out.push(option_line(index, label, index == cursor, theme));
    }
    out.push(Line::default());
    out.push(footer(app, request, &name, folds, theme, width));
    out.push(indented(vec![
        Span::styled("Esc deny", theme.failure().patch(theme.title())),
        Span::styled(format!(" {} ", theme.glyphs().dot), theme.secondary()),
        Span::styled("Ctrl+C defer", theme.attention().patch(theme.title())),
    ]));
    out.push(Line::default());
    out
}

fn indented(mut spans: Vec<Span<'static>>) -> Line<'static> {
    spans.insert(0, Span::raw(INDENT));
    Line::from(spans)
}

/// `tester [verifier] · child of implementer · haiku-4.5 low`.
fn attribution(app: &App, agent: AgentId) -> String {
    let Some(node) = app.tree.nodes.get(&agent) else {
        return String::new();
    };
    let dot = app.theme.glyphs().dot;
    let a = &node.agent;
    let name = app.agent_display_name(agent);
    let mut parts = vec![if a.role.0 == a.name {
        name
    } else {
        format!("{name} [{}]", a.role.0)
    }];
    if let Some(parent) = a.parent {
        parts.push(format!("child of {}", app.agent_display_name(parent)));
    }
    let mut model = a.model.model.clone();
    if let Some(effort) = a.config.effort {
        model.push_str(&format!(" {effort}"));
    }
    parts.push(model);
    parts.join(&format!(" {dot} "))
}

/// The body's dim lead (`$ ` for a command) and the text it introduces.
fn body_text(request: &ApprovalRequest, kind: &Kind<'_>) -> (&'static str, String) {
    match kind {
        Kind::Command(command) => ("$ ", (*command).to_owned()),
        Kind::File(path) => ("", (*path).to_owned()),
        Kind::Network | Kind::Other => (
            "",
            request
                .argument
                .clone()
                .unwrap_or_else(|| request.description.clone()),
        ),
    }
}

fn body_lines(
    prefix: &str,
    text: &str,
    room: usize,
    max_rows: usize,
    theme: &Theme,
) -> Vec<Line<'static>> {
    wrap_rows(text, room, max_rows, ellipsis(theme))
        .into_iter()
        .enumerate()
        .map(|(row, chunk)| {
            let lead = if row == 0 { prefix } else { "" };
            let pad = " ".repeat(prefix.width() - lead.width());
            indented(vec![
                Span::styled(format!("{lead}{pad}"), theme.secondary()),
                Span::styled(chunk, theme.text()),
            ])
        })
        .collect()
}

/// Hard-wrap `text` to `width` columns, at most `max_rows`; a cut ends in `marker`.
fn wrap_rows(text: &str, width: usize, max_rows: usize, marker: char) -> Vec<String> {
    let width = width.max(1);
    let mut rows: Vec<String> = Vec::new();
    'lines: for source in text.lines() {
        let mut current = String::new();
        let mut used = 0usize;
        for c in source.chars() {
            let w = c.width().unwrap_or(0);
            if used + w > width && !current.is_empty() {
                rows.push(std::mem::take(&mut current));
                used = 0;
                if rows.len() > max_rows {
                    break 'lines;
                }
            }
            current.push(c);
            used += w;
        }
        rows.push(current);
        if rows.len() > max_rows {
            break;
        }
    }
    if rows.is_empty() {
        rows.push(String::new());
    }
    if rows.len() > max_rows {
        rows.truncate(max_rows);
        if let Some(last) = rows.last_mut() {
            *last = format::truncate_with_marker(&format!("{last}{marker}"), width, marker);
        }
    }
    rows
}

/// The theme's ellipsis as a truncation marker (`~` in ASCII mode).
pub(crate) fn ellipsis(theme: &Theme) -> char {
    theme.glyphs().ellipsis.chars().next().unwrap_or('~')
}

/// What option 2 and 3 would remember: the exact argument, else the path,
/// cut to `room`. A URL drops its scheme so host and path get the room.
fn rule_subject(request: &ApprovalRequest, room: usize, marker: char) -> String {
    let subject = request
        .argument
        .clone()
        .or_else(|| {
            request.resources.first().map(|r| {
                let key = &r.0;
                key.split_once(':')
                    .map_or(key.as_str(), |(_, path)| path)
                    .to_owned()
            })
        })
        .unwrap_or_else(|| request.tool.clone());
    let subject = ["https://", "http://"]
        .iter()
        .find_map(|scheme| subject.strip_prefix(scheme))
        .unwrap_or(&subject);
    format::one_line_with_marker(subject, room.max(SUBJECT_MIN_COLS), marker)
}

/// Option labels; text in backticks renders as inline code.
fn option_labels(
    request: &ApprovalRequest,
    kind: &Kind<'_>,
    name: &str,
    width: u16,
    marker: char,
) -> [(String, &'static str); OPTION_COUNT] {
    // Option 2 is the longest row; its subject gets whatever it leaves free.
    let around = format!("{INDENT}2. Yes, and allow  for {name} and its subagents (a)");
    let room = usize::from(width).saturating_sub(around.width());
    let subject = rule_subject(request, room, marker);
    let once = match kind {
        Kind::Command(_) => "Yes, run it".to_owned(),
        _ => "Yes, allow it".to_owned(),
    };
    [
        (once, "y"),
        (
            format!("Yes, and allow `{subject}` for {name} and its subagents"),
            "a",
        ),
        (format!("Always allow `{subject}` everywhere"), "p"),
        (format!("No, and tell {name} what to do instead"), "esc"),
    ]
}

fn option_line(
    index: usize,
    (label, key): &(String, &'static str),
    selected: bool,
    theme: &Theme,
) -> Line<'static> {
    let glyphs = theme.glyphs();
    // The highlighted option reads in title weight; inline code keeps its accent.
    let (plain, code) = if selected {
        (theme.title(), theme.accent().patch(theme.title()))
    } else {
        (theme.text(), theme.accent())
    };
    let mut spans = vec![if selected {
        Span::styled(format!("{} ", glyphs.prompt), code)
    } else {
        Span::raw(INDENT)
    }];
    spans.push(Span::styled(format!("{}. ", index + 1), theme.secondary()));
    for (i, part) in label.split('`').enumerate() {
        if part.is_empty() {
            continue;
        }
        let style = if i % 2 == 1 { code } else { plain };
        spans.push(Span::styled(part.to_owned(), style));
    }
    spans.push(Span::styled(" (", theme.secondary()));
    spans.push(Span::styled(*key, theme.title()));
    spans.push(Span::styled(")", theme.secondary()));
    Line::from(spans)
}

/// `enter confirm · ↑↓ select · e show all · o open tester  ·  1 more waiting (implementer)`.
/// The names go first when the row is short of room; `e` only when the body folds.
fn footer(
    app: &App,
    request: &ApprovalRequest,
    name: &str,
    folds: bool,
    theme: &Theme,
    width: u16,
) -> Line<'static> {
    let dot = theme.glyphs().dot;
    let arrows = if theme.ascii() { "up/down" } else { "↑↓" };
    let mut keys = vec![
        ("enter", "confirm".to_owned()),
        (arrows, "select".to_owned()),
    ];
    if folds {
        let expanded = app.bottom.approval.expanded == Some(request.id);
        keys.push(("e", if expanded { "fold" } else { "show all" }.to_owned()));
    }
    if app.selected != Some(request.agent_id) {
        keys.push(("o", format!("open {name}")));
    }
    let mut spans = vec![Span::raw(INDENT)];
    for (i, (key, label)) in keys.into_iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(format!(" {dot} "), theme.secondary()));
        }
        spans.push(Span::styled(key, theme.title()));
        spans.push(Span::styled(format!(" {label}"), theme.secondary()));
    }
    if let Some((count, names)) = waiting(app, request, theme) {
        let used: usize = spans.iter().map(|s| s.content.width()).sum();
        let full = format!("  {dot}  {count} more waiting ({names})");
        let text = if used + full.width() <= usize::from(width) {
            full
        } else {
            format!("  {dot}  {count} more waiting")
        };
        spans.push(Span::styled(text, theme.secondary()));
    }
    Line::from(spans)
}

/// How many others wait (deferred ones included) and up to three of their names.
fn waiting(app: &App, request: &ApprovalRequest, theme: &Theme) -> Option<(usize, String)> {
    let others: Vec<&ApprovalRequest> = app
        .approvals
        .iter()
        .filter(|a| a.id != request.id && !app.resolving.contains(&a.id))
        .collect();
    if others.is_empty() {
        return None;
    }
    let mut names: Vec<String> = Vec::new();
    for other in &others {
        let name = app.agent_display_name(other.agent_id);
        if !names.contains(&name) {
            names.push(name);
        }
    }
    let shown = names.len().min(WAITING_NAMES_MAX);
    let mut list = names[..shown].join(", ");
    if names.len() > shown {
        list.push_str(&format!(", {}", theme.glyphs().ellipsis));
    }
    Some((others.len(), list))
}

#[cfg(test)]
mod tests;
