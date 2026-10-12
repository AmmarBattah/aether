//! `/artifacts`: list; Enter opens the pager. Owner: W7.

use crossterm::event::KeyEvent;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Span;

use aether_core::artifact::Artifact;
use aether_core::id::ArtifactId;

use super::picker::{Picker, PickerEvent, PickerRow};
use super::{Hint, ViewKind, clean, render_empty, sep, unhandled};
use crate::app::App;
use crate::format;
use crate::ui::bottom::{BottomView, ViewAction, ViewOutcome};
use crate::ui::theme::tokens::Theme;

const HINTS: &[Hint] = &[
    ("↑↓", "choose"),
    ("type", "to filter"),
    ("enter", "open"),
    ("esc", "back"),
];

#[derive(Debug)]
pub struct ArtifactsView {
    picker: Picker<ArtifactId>,
}

impl Default for ArtifactsView {
    fn default() -> Self {
        ArtifactsView {
            picker: Picker::new("Artifacts", Vec::new()),
        }
    }
}

impl ArtifactsView {
    fn picker(&self, app: &App) -> Picker<ArtifactId> {
        self.picker.with_items(rows(app))
    }

    /// `Artifacts · worker`, the agent dim.
    fn title(&self, app: &App, theme: &Theme) -> Vec<Span<'static>> {
        let mut title = vec![Span::styled("Artifacts", theme.title())];
        if let Some(agent) = app.selected_agent() {
            title.push(Span::styled(
                format!("{}{}", sep(theme), clean(&app.agent_display_name(agent.id))),
                theme.secondary(),
            ));
        }
        title
    }
}

impl BottomView for ArtifactsView {
    fn wheel(&mut self, up: bool, app: &App) -> ViewOutcome {
        self.handle_key(super::wheel_key(up), app)
    }

    fn kind(&self) -> Option<super::ViewKind> {
        Some(super::ViewKind::Artifacts)
    }

    fn handle_key(&mut self, key: KeyEvent, app: &App) -> ViewOutcome {
        self.picker = self.picker(app);
        let event = self.picker.handle_key(key);
        match event {
            PickerEvent::Chosen(index) => open(&self.picker.items[index].1, app),
            PickerEvent::Moved | PickerEvent::Filtered => ViewOutcome::Consumed,
            PickerEvent::Cancelled => ViewOutcome::Close(Vec::new()),
            PickerEvent::Ignored => unhandled(key),
        }
    }

    fn desired_height(&self, width: u16, app: &App) -> u16 {
        self.picker(app).desired_height(width)
    }

    fn render(&self, frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
        let picker = self.picker(app);
        let mut title = self.title(app, theme);
        if picker.items.is_empty() {
            let text = format!(
                "No artifacts yet{}files and output agents keep appear here",
                sep(theme)
            );
            return render_empty(frame, area, title, &text, theme);
        }
        title.extend(picker.filter_spans(theme));
        picker.render_titled(frame, area, title, theme, HINTS);
    }
}

/// Bytes retention already dropped are reported, not fetched; the daemon's
/// reply still has the last word for everything else.
fn open(id: &ArtifactId, app: &App) -> ViewOutcome {
    let id = *id;
    if let Some(artifact) = app.artifacts.list.iter().find(|a| a.id == id)
        && !artifact.has_bytes()
    {
        return ViewOutcome::Act(vec![ViewAction::Flash(
            app.artifact_eviction_notice(artifact),
        )]);
    }
    ViewOutcome::Act(vec![ViewAction::Open(ViewKind::ArtifactPager(id))])
}

fn rows(app: &App) -> Vec<(PickerRow, ArtifactId)> {
    let theme = &app.theme;
    app.artifacts
        .list
        .iter()
        .map(|artifact| (row(artifact, theme), artifact.id))
        .collect()
}

fn row(artifact: &Artifact, theme: &Theme) -> PickerRow {
    let dot = sep(theme);
    let kind = super::artifact_kind(&artifact.kind);
    let mut detail = format!("{kind}{dot}{}", format::bytes(artifact.size_bytes));
    if !artifact.has_bytes() {
        detail.push_str(&format!("{dot}bytes dropped"));
    } else if !artifact.summary.is_empty() {
        detail.push_str(&format!("{dot}{}", clean(&artifact.summary)));
    }
    PickerRow::new(clean(&artifact.name), detail)
}
