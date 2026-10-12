//! full-screen pager for artifacts and reports (the one full-screen view). Owner: W7.

use std::cell::Cell;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Span;

use aether_core::id::{ArtifactId, MemoryId};
use aether_protocol::Command;

use super::report::report_lines;
use super::{Hint, body_room, clean, render_frame, row, sep, title, unhandled};
use crate::app::{App, Effect, Then};
use crate::format;
use crate::ui::bottom::{BottomView, ViewAction, ViewOutcome};
use crate::ui::theme::tokens::Theme;

const HINTS: &[Hint] = &[("↑↓", "scroll"), ("pgup pgdn", "page"), ("esc", "back")];
const REPORT_HINTS: &[Hint] = &[
    ("↑↓", "scroll"),
    ("pgup pgdn", "page"),
    ("r", "refresh"),
    ("esc", "back"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PagerSource {
    /// `app.report`: the viewed agent's run report.
    Report,
    Memory(MemoryId),
    /// `app.artifacts.open`, once the bytes for this id have arrived.
    Artifact(ArtifactId),
}

/// Rows and wrapped line count from the last render. Keys clamp against it,
/// since only render knows the width the text wraps at.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Extent {
    rows: usize,
    total: usize,
}

#[derive(Debug)]
pub struct PagerView {
    source: PagerSource,
    offset: usize,
    extent: Cell<Extent>,
}

impl PagerView {
    pub fn new(source: PagerSource) -> Self {
        PagerView {
            source,
            offset: 0,
            extent: Cell::new(Extent::default()),
        }
    }

    pub fn source(&self) -> PagerSource {
        self.source
    }

    fn scroll(&mut self, key: KeyEvent) -> bool {
        let Extent { rows, total } = self.extent.get();
        let page = rows.saturating_sub(1).max(1);
        let max = total.saturating_sub(rows);
        self.offset = match key.code {
            KeyCode::Up => self.offset.saturating_sub(1),
            KeyCode::Down => self.offset.saturating_add(1),
            KeyCode::PageUp => self.offset.saturating_sub(page),
            KeyCode::PageDown | KeyCode::Char(' ') => self.offset.saturating_add(page),
            KeyCode::Home => 0,
            KeyCode::End => max,
            _ => return false,
        }
        .min(max);
        true
    }
}

impl BottomView for PagerView {
    fn wheel(&mut self, up: bool, app: &App) -> ViewOutcome {
        self.handle_key(super::wheel_key(up), app)
    }

    fn kind(&self) -> Option<super::ViewKind> {
        match self.source {
            PagerSource::Artifact(id) => Some(super::ViewKind::ArtifactPager(id)),
            PagerSource::Report | PagerSource::Memory(_) => None,
        }
    }

    fn handle_key(&mut self, key: KeyEvent, app: &App) -> ViewOutcome {
        let chord = key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
        match key.code {
            _ if chord => unhandled(key),
            KeyCode::Esc => ViewOutcome::Close(Vec::new()),
            KeyCode::Char('r') if self.source == PagerSource::Report => report_refresh(app),
            _ if self.scroll(key) => ViewOutcome::Consumed,
            _ => unhandled(key),
        }
    }

    fn desired_height(&self, _width: u16, _app: &App) -> u16 {
        u16::MAX
    }

    fn render(&self, frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
        let rows = body_room(area);
        // Text wraps inside the margin on both sides.
        let width = area.width.saturating_sub(4);
        let (title, lines, hints, size) = match self.source {
            PagerSource::Report => {
                let (title, lines) = report_lines(app, theme);
                (title, lines, REPORT_HINTS, None)
            }
            PagerSource::Memory(id) => {
                let text = app
                    .memories
                    .iter()
                    .find(|record| record.id == id)
                    .map(super::memory::details)
                    .unwrap_or_else(|| {
                        "This memory is no longer loaded; reopen /memory to refresh.".into()
                    });
                let lines = wrap(&text, width)
                    .into_iter()
                    .map(|line| row(line, theme.text()))
                    .collect();
                (title("Memory details", "", theme), lines, HINTS, None)
            }
            PagerSource::Artifact(id) => {
                let (title, text, size) = artifact_text(app, id, theme);
                let lines = wrap(&text, width)
                    .into_iter()
                    .map(|line| row(line, theme.text()))
                    .collect();
                (title, lines, HINTS, size)
            }
        };
        let total = lines.len();
        self.extent.set(Extent { rows, total });
        let start = self.offset.min(total.saturating_sub(rows));
        let mut title = title;
        let mut meta: Vec<String> = size.into_iter().collect();
        if total > rows {
            let end = (start + rows).min(total);
            let dash = if theme.ascii() { "-" } else { "–" };
            meta.push(format!(
                "lines {}{dash}{} of {}",
                thousands(start + 1),
                thousands(end),
                thousands(total)
            ));
        }
        if !meta.is_empty() {
            title.push(Span::styled(
                format!("  {}", meta.join(&sep(theme))),
                theme.secondary(),
            ));
        }
        let shown = lines.into_iter().skip(start).take(rows).collect();
        render_frame(frame, area, title, shown, hints, theme);
    }

    fn fullscreen(&self) -> bool {
        true
    }
}

/// `4,000`.
fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Re-read the report for the run on screen; nothing to do without one.
pub(super) fn report_refresh(app: &App) -> ViewOutcome {
    match app.report.run {
        Some(run) if !app.report.loading => {
            ViewOutcome::Act(vec![ViewAction::Effect(Box::new(Effect::Request {
                command: Command::GetRunReport { run },
                then: Then::RunReport(run),
            }))])
        }
        _ => ViewOutcome::Consumed,
    }
}

/// The artifact's title, its text, and its size for the title's meta.
fn artifact_text(
    app: &App,
    id: ArtifactId,
    theme: &Theme,
) -> (Vec<Span<'static>>, String, Option<String>) {
    let dot = sep(theme);
    let open = app
        .artifacts
        .open
        .as_ref()
        .filter(|open| open.artifact.id == id);
    let Some(open) = open else {
        let listed = app.artifacts.list.iter().find(|a| a.id == id);
        let name = listed.map_or_else(|| "Artifact".to_owned(), |a| clean(&a.name));
        let text = match listed {
            Some(artifact) if !artifact.has_bytes() => app.artifact_eviction_notice(artifact),
            _ => format!("downloading{}", theme.glyphs().ellipsis),
        };
        return (title(name, "", theme), text, None);
    };
    let artifact = &open.artifact;
    let mut size = format::bytes(artifact.size_bytes);
    if open.truncated {
        size.push_str(&format!(
            "{dot}first {} shown",
            format::bytes(open.text.len() as u64)
        ));
    }
    let mut heading = title(clean(&artifact.name), "", theme);
    heading.push(Span::styled(
        format!("{dot}{}{dot}{size}", super::artifact_kind(&artifact.kind)),
        theme.secondary(),
    ));
    (heading, open.text.clone(), None)
}

/// Hard-wraps every line to `width`, keeping blank lines; control bytes are
/// stripped so artifact content cannot move the cursor.
fn wrap(text: &str, width: u16) -> Vec<String> {
    let width = usize::from(width.max(1));
    let text = aether_core::text::strip_controls(text);
    text.lines()
        .flat_map(|line| {
            if line.is_empty() {
                return vec![String::new()];
            }
            textwrap::wrap(line, width)
                .into_iter()
                .map(|part| part.into_owned())
                .collect()
        })
        .collect()
}
