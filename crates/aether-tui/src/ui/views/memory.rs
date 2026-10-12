//! `/memory`: records, pin, forget (two-press), restore. Owner: W7.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Line;

use aether_core::id::MemoryId;
use aether_core::memory::{MemoryRecord, MemoryScope};
use aether_protocol::Command;

use super::picker::{Picker, PickerEvent, PickerRow};
use super::{
    Hint, body_room, clean, frame_height, render_empty, render_frame, row, sep, short_id, unhandled,
};
use crate::app::{App, Effect, MemoryAction, Then};
use crate::ui::bottom::{BottomView, ViewAction, ViewOutcome};
use crate::ui::theme::tokens::Theme;

const HINTS: &[Hint] = &[
    ("↑↓", "choose"),
    ("enter", "details"),
    ("p", "pin"),
    ("f", "forget"),
    ("u", "restore"),
    ("esc", "back"),
];
/// The blank row and the detail row under the list.
const DETAIL_ROWS: usize = 2;

#[derive(Debug)]
pub struct MemoryView {
    picker: Picker<MemoryId>,
    /// Armed by the first `f`; a second `f` on the same record forgets it.
    pending_forget: Option<MemoryId>,
}

impl Default for MemoryView {
    fn default() -> Self {
        MemoryView {
            picker: Picker::new("Memory", Vec::new()),
            pending_forget: None,
        }
    }
}

impl MemoryView {
    fn picker(&self, app: &App) -> Picker<MemoryId> {
        self.picker.with_items(rows(app))
    }

    fn selected<'a>(&self, app: &'a App) -> Option<&'a MemoryRecord> {
        let id = self.picker.selected()?;
        app.memories.iter().find(|record| record.id == *id)
    }

    fn pin(&self, app: &App) -> ViewOutcome {
        let Some(record) = self.selected(app) else {
            return flash("no memory highlighted");
        };
        let action = if record.pinned {
            MemoryAction::Unpinned
        } else {
            MemoryAction::Pinned
        };
        request(
            Command::PinMemory {
                id: record.id,
                pinned: !record.pinned,
            },
            action,
        )
    }

    /// Asks first: the cursor moves on refresh, so one keystroke is thin
    /// protection for a decision that then has to be undone deliberately.
    fn forget(&mut self, app: &App) -> ViewOutcome {
        let Some(record) = self.selected(app) else {
            return flash("no memory highlighted");
        };
        let id = record.id;
        if self.pending_forget != Some(id) {
            self.pending_forget = Some(id);
            let theme = &app.theme;
            return flash(format!(
                "forget this memory? f again to confirm{}u restores it later",
                sep(theme)
            ));
        }
        self.pending_forget = None;
        request(
            Command::ForgetMemory { id, reason: None },
            MemoryAction::Forgotten,
        )
    }

    fn restore(&self, app: &App) -> ViewOutcome {
        let Some(record) = self.selected(app) else {
            return flash("no memory highlighted");
        };
        if record.forgotten_at.is_none() {
            return flash("that memory is not forgotten");
        }
        request(
            Command::RestoreMemory { id: record.id },
            MemoryAction::Restored,
        )
    }
}

impl BottomView for MemoryView {
    fn wheel(&mut self, up: bool, app: &App) -> ViewOutcome {
        self.handle_key(super::wheel_key(up), app)
    }

    fn kind(&self) -> Option<super::ViewKind> {
        Some(super::ViewKind::Memory)
    }

    fn handle_key(&mut self, key: KeyEvent, app: &App) -> ViewOutcome {
        self.picker = self.picker(app);
        if key.code != KeyCode::Char('f') {
            self.pending_forget = None;
        }
        match key.code {
            KeyCode::Enter if key.modifiers.is_empty() => {
                return self.selected(app).map_or(ViewOutcome::Consumed, |record| {
                    ViewOutcome::Act(vec![ViewAction::Push(Box::new(
                        super::pager::PagerView::new(super::pager::PagerSource::Memory(record.id)),
                    ))])
                });
            }
            KeyCode::Char('p') => return self.pin(app),
            KeyCode::Char('f') => return self.forget(app),
            KeyCode::Char('u') => return self.restore(app),
            // Letters are hotkeys here, so they never reach the filter.
            KeyCode::Char(c) if !c.is_ascii_digit() => return unhandled(key),
            _ => {}
        }
        match self.picker.handle_key(key) {
            PickerEvent::Cancelled => ViewOutcome::Close(Vec::new()),
            PickerEvent::Ignored => unhandled(key),
            _ => ViewOutcome::Consumed,
        }
    }

    fn desired_height(&self, _width: u16, app: &App) -> u16 {
        let picker = self.picker(app);
        if picker.items.is_empty() {
            return frame_height(1);
        }
        frame_height(picker.body_len() + DETAIL_ROWS)
    }

    fn render(&self, frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
        let picker = self.picker(app);
        if picker.items.is_empty() {
            let text = format!("No memories yet{}/remember <fact> saves one", sep(theme));
            return render_empty(frame, area, picker.title_spans(theme), &text, theme);
        }
        let record = picker
            .selected()
            .and_then(|id| app.memories.iter().find(|record| record.id == *id));
        let detail = record.and_then(|record| detail_line(record, theme));
        let room = body_room(area);
        // The detail row gives way first on a short pane.
        let detail = detail.filter(|_| room > DETAIL_ROWS);
        let list_room = room.saturating_sub(if detail.is_some() { DETAIL_ROWS } else { 0 });
        let numbered = picker.items.len() <= 9;
        let mut body = picker.lines(area.width, list_room, theme, true, numbered);
        if let Some(detail) = detail {
            body.push(Line::default());
            body.push(detail);
        }
        render_frame(frame, area, picker.title_spans(theme), body, HINTS, theme);
    }
}

fn flash(text: impl Into<String>) -> ViewOutcome {
    ViewOutcome::Act(vec![ViewAction::Flash(text.into())])
}

fn request(command: Command, action: MemoryAction) -> ViewOutcome {
    ViewOutcome::Act(vec![ViewAction::Effect(Box::new(Effect::Request {
        command,
        then: Then::MemoryMutated(action),
    }))])
}

/// Forgotten and superseded records stay inspectable; dim rows distinguish them.
fn rows(app: &App) -> Vec<(PickerRow, MemoryId)> {
    let theme = &app.theme;
    app.memories
        .iter()
        .map(|record| {
            let row = PickerRow::new(clean(&record.text), summary(record, theme));
            let row = if record.is_current() {
                row
            } else {
                row.disabled()
            };
            (row, record.id)
        })
        .collect()
}

fn summary(record: &MemoryRecord, theme: &Theme) -> String {
    let dot = sep(theme);
    let mut parts = vec![kind_label(record), scope_label(&record.scope, theme)];
    if record.pinned {
        parts.push("pinned".into());
    }
    if record.forgotten_at.is_some() {
        parts.push("forgotten".into());
    } else if record.superseded_by.is_some() {
        parts.push("superseded".into());
    }
    parts.join(&dot)
}

/// Tags and where the highlighted record came from; nothing when neither
/// is known.
fn detail_line(record: &MemoryRecord, theme: &Theme) -> Option<Line<'static>> {
    let mut parts = Vec::new();
    if !record.tags.is_empty() {
        parts.push(clean(&record.tags.join(", ")));
    }
    if let Some(actor) = &record.provenance.actor {
        parts.push(format!("from {}", clean(&actor.label())));
    }
    if let Some(source) = &record.provenance.source {
        parts.push(format!("from {}", clean(&source.label())));
    }
    (!parts.is_empty()).then(|| row(parts.join(&sep(theme)), theme.secondary()))
}

fn kind_label(record: &MemoryRecord) -> String {
    format!("{:?}", record.kind).to_lowercase()
}

pub(crate) fn scope_label(scope: &MemoryScope, theme: &Theme) -> String {
    match scope {
        MemoryScope::Global => "global".to_owned(),
        MemoryScope::Repository { id } => format!("repo {}", clean(id)),
        MemoryScope::Role { role } => format!("role {}", clean(role)),
        MemoryScope::Agent { id } => format!("agent {}", short_id(theme, id)),
        MemoryScope::RepositoryRole { id, role } => {
            format!("role {} in {}", clean(role), clean(id))
        }
        MemoryScope::Subtree { root } => format!("subtree {}", short_id(theme, root)),
    }
}

/// Full record contents and provenance, read from the same record as the list.
pub(super) fn details(record: &MemoryRecord) -> String {
    let mut lines = vec![
        record.text.clone(),
        String::new(),
        format!("ID: {}", record.id),
        format!("Kind: {}", kind_label(record)),
        format!(
            "Scope: {}",
            scope_label(&record.scope, &crate::ui::cells::plain_theme())
        ),
        format!(
            "Importance: {:.2} · Confidence: {:.2}",
            record.importance, record.confidence
        ),
        format!("Created: {}", record.created_at),
        format!("Retrievals: {}", record.retrieval_count),
        format!("Pinned: {}", record.pinned),
    ];
    if !record.tags.is_empty() {
        lines.push(format!("Tags: {}", record.tags.join(", ")));
    }
    if let Some(at) = record.last_retrieved_at {
        lines.push(format!("Last retrieved: {at}"));
    }
    if let Some(at) = record.forgotten_at {
        lines.push(format!("Forgotten: {at}"));
    }
    if let Some(id) = record.supersedes {
        lines.push(format!("Supersedes: {id}"));
    }
    if let Some(id) = record.superseded_by {
        lines.push(format!("Superseded by: {id}"));
    }
    let provenance = &record.provenance;
    if let Some(actor) = &provenance.actor {
        lines.push(format!("Actor: {}", actor.label()));
    }
    if let Some(id) = provenance.task {
        lines.push(format!("Task: {id}"));
    }
    if let Some(id) = provenance.event {
        lines.push(format!("Event: {id}"));
    }
    if let Some(id) = provenance.artifact {
        lines.push(format!("Artifact: {id}"));
    }
    if let Some(id) = provenance.agent {
        lines.push(format!("Agent: {id}"));
    }
    if let Some(source) = &provenance.source {
        lines.push(format!("Source: {}", source.file));
        if let Some(commit) = &source.commit {
            lines.push(format!("Commit: {commit}"));
        }
    }
    if let Some(retracted) = &record.retracted {
        lines.push(format!(
            "Retracted: {} at {} ({})",
            retracted.file, retracted.commit, retracted.at
        ));
    }
    if let Some(structured) = &record.structured {
        lines.push(format!("Structured: {structured}"));
    }
    lines.join("\n")
}
