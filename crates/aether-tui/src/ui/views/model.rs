//! `/model`: model and effort picker, two columns. Owner: W7.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use aether_core::model::{Effort, ModelChain, ModelRef};

use super::picker::{MAX_VISIBLE_ROWS, Picker, PickerEvent, PickerRow, clip_line};
use super::{Hint, body_room, frame_height, indent, render_frame, sep, title, unhandled};
use crate::app::{App, EFFORTS};
use crate::commands::SlashCommand;
use crate::ui::bottom::{BottomView, ViewAction, ViewOutcome};
use crate::ui::theme::tokens::Theme;

/// Columns between the model and effort lists.
const COLUMN_GAP: usize = 4;
/// Narrowest effort column: marker, `medium`, and the start of its note.
/// The model column takes the rest it needs, so its notes stay whole.
const EFFORT_MIN: usize = 22;
const HINTS: &[Hint] = &[
    ("←→", "model/effort"),
    ("↑↓", "choose"),
    ("type", "to filter"),
    ("enter", "apply"),
    ("esc", "back"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Column {
    Model,
    Effort,
}

/// Enter applies whichever of model and effort differ from the agent's
/// current ones; nothing is sent when neither changed.
#[derive(Debug)]
pub struct ModelView {
    models: Picker<ModelRef>,
    efforts: Picker<Effort>,
    column: Column,
}

impl ModelView {
    pub fn new(app: &App) -> Self {
        let mut models = Picker::new("Model", model_rows(app)).label_first();
        let current = current_model(app);
        if let Some(at) = models.items.iter().position(|(_, m)| Some(m) == current) {
            models.cursor = at;
        }
        let mut efforts = Picker::new("Effort", effort_rows(&app.theme));
        let effort = app.effort_for_views();
        efforts.cursor = EFFORTS.iter().position(|(e, _)| *e == effort).unwrap_or(0);
        ModelView {
            models,
            efforts,
            column: Column::Model,
        }
    }

    fn apply(&self, app: &App) -> ViewOutcome {
        let mut actions = Vec::new();
        if self.models.selected_row().is_some_and(|row| row.disabled) {
            return ViewOutcome::Act(vec![ViewAction::Flash(self.sign_in_hint(app))]);
        }
        if let Some(model) = self.models.selected()
            && Some(model) != current_model(app)
        {
            actions.push(ViewAction::Command(SlashCommand::Model {
                chain: ModelChain::single(model.clone()),
                default_only: false,
            }));
        }
        if let Some(&effort) = self.efforts.selected()
            && effort != app.effort_for_views()
        {
            actions.push(ViewAction::Command(SlashCommand::Effort(effort)));
        }
        ViewOutcome::Close(actions)
    }

    fn sign_in_hint(&self, app: &App) -> String {
        let info = self
            .models
            .selected()
            .and_then(|model| app.model_info(model));
        match info {
            Some(info) => format!(
                "{} is not signed in{}/login {}",
                info.provider_name,
                sep(&app.theme),
                info.model.provider
            ),
            None => "that model is not signed in".to_owned(),
        }
    }

    fn title(&self, app: &App, theme: &Theme) -> Vec<Span<'static>> {
        let target = app
            .model_target()
            .and_then(|id| app.tree.nodes.get(&id))
            .map(|node| app.agent_display_name(node.agent.id));
        let scope = match target {
            Some(name) => format!("for {name}{}from the next turn", sep(theme)),
            None => "for the next agent".to_owned(),
        };
        title("Model and effort", scope, theme)
    }

    /// Both lists side by side: models in a column as wide as their longest
    /// label and note (leaving `EFFORT_MIN`), efforts four columns after it.
    fn body(
        &self,
        models: &Picker<ModelRef>,
        width: u16,
        room: usize,
        theme: &Theme,
    ) -> Vec<Line<'static>> {
        let content = usize::from(width).saturating_sub(4);
        let widest = |width: fn(&PickerRow) -> usize| {
            models
                .items
                .iter()
                .map(|(row, _)| width(row))
                .max()
                .unwrap_or(0)
        };
        let wanted = 2 + widest(|row| row.label.width()) + 2 + widest(|row| row.detail.width());
        let left_width = wanted.min(content.saturating_sub(COLUMN_GAP + EFFORT_MIN));
        let left = models.lines(
            left_width as u16 + 2,
            room,
            theme,
            self.column == Column::Model,
            false,
        );
        let right = self.efforts.lines(
            content.saturating_sub(left_width + COLUMN_GAP) as u16 + 2,
            room,
            theme,
            self.column == Column::Effort,
            false,
        );
        let rows = left.len().max(right.len());
        let mut left = left.into_iter();
        let mut right = right.into_iter();
        (0..rows)
            .map(|_| {
                let mut line = clip_line(left.next().unwrap_or_default(), left_width as u16, theme);
                let pad = left_width.saturating_sub(line.width()) + COLUMN_GAP;
                line.spans.push(Span::raw(" ".repeat(pad)));
                line.spans.extend(right.next().unwrap_or_default().spans);
                line
            })
            .collect()
    }

    /// Body rows at `width`: the taller list, then the selected model's rows.
    fn body_len(&self, models: &Picker<ModelRef>, width: u16, theme: &Theme) -> usize {
        let selected = selected_lines(models, width, theme).len();
        models.body_len().max(self.efforts.body_len()) + selected
    }
}

impl BottomView for ModelView {
    fn wheel(&mut self, up: bool, app: &App) -> ViewOutcome {
        self.handle_key(super::wheel_key(up), app)
    }

    fn kind(&self) -> Option<super::ViewKind> {
        Some(super::ViewKind::Model)
    }

    fn handle_key(&mut self, key: KeyEvent, app: &App) -> ViewOutcome {
        self.models = self.models.with_items(model_rows(app));
        match key.code {
            KeyCode::Left | KeyCode::Right | KeyCode::Tab | KeyCode::BackTab => {
                self.column = match self.column {
                    Column::Model => Column::Effort,
                    Column::Effort => Column::Model,
                };
                return ViewOutcome::Consumed;
            }
            // Letters only filter the model list; the effort list is five rows.
            KeyCode::Char(c) if self.column == Column::Effort && !c.is_ascii_digit() => {
                return ViewOutcome::Consumed;
            }
            _ => {}
        }
        let picker_event = match self.column {
            Column::Model => self.models.handle_key(key),
            Column::Effort => self.efforts.handle_key(key),
        };
        match picker_event {
            PickerEvent::Ignored if key.code == KeyCode::Enter => self.apply(app),
            PickerEvent::Ignored => unhandled(key),
            PickerEvent::Moved | PickerEvent::Filtered => ViewOutcome::Consumed,
            PickerEvent::Chosen(_) => self.apply(app),
            PickerEvent::Cancelled => ViewOutcome::Close(Vec::new()),
        }
    }

    fn desired_height(&self, width: u16, app: &App) -> u16 {
        let models = self.models.with_items(model_rows(app));
        frame_height(self.body_len(&models, width, &app.theme))
    }

    fn render(&self, frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
        let models = self.models.with_items(model_rows(app));
        let selected = selected_lines(&models, area.width, theme);
        let room = body_room(area)
            .saturating_sub(selected.len())
            .clamp(1, MAX_VISIBLE_ROWS);
        let mut body = self.body(&models, area.width, room, theme);
        body.extend(selected);
        render_frame(frame, area, self.title(app, theme), body, HINTS, theme);
    }
}

/// The selected model's full id and detail under the lists, after a gap:
/// the columns may cut both, and Enter should never apply a guess.
fn selected_lines(models: &Picker<ModelRef>, width: u16, theme: &Theme) -> Vec<Line<'static>> {
    let Some(row) = models.selected_row() else {
        return Vec::new();
    };
    let content = usize::from(width).saturating_sub(4).max(1);
    let detail = format!("{}{}", sep(theme), row.detail);
    if row.detail.is_empty() || row.label.width() + detail.width() <= content {
        let mut spans = vec![Span::styled(row.label.clone(), theme.text())];
        if !row.detail.is_empty() {
            spans.push(Span::styled(detail, theme.secondary()));
        }
        return vec![Line::default(), indent(Line::from(spans))];
    }
    let mut lines = vec![Line::default()];
    lines.extend(
        textwrap::wrap(&row.label, content)
            .iter()
            .map(|part| super::row(part.to_string(), theme.text())),
    );
    lines.extend(
        textwrap::wrap(&row.detail, content)
            .iter()
            .map(|part| super::row(part.to_string(), theme.secondary())),
    );
    lines
}

fn current_model(app: &App) -> Option<&ModelRef> {
    let target = app.model_target()?;
    let model = &app.tree.nodes.get(&target)?.agent.model;
    Some(app.model_info(model).map_or(model, |info| &info.model))
}

/// Current model first, then ready, then signed-out (shown, not choosable),
/// each group recently chosen first, then in registry order.
fn model_rows(app: &App) -> Vec<(PickerRow, ModelRef)> {
    let theme = &app.theme;
    let current = current_model(app);
    let session_default = app
        .options
        .default_model
        .as_ref()
        .map(|model| app.model_info(model).map_or(model, |info| &info.model));
    // Saved choices may be aliases: rank each by the entry it resolves to.
    let recent: Vec<&ModelRef> = app
        .recent_models
        .iter()
        .filter_map(|name| app.model_info(&ModelRef::parse(name)?))
        .map(|info| &info.model)
        .collect();
    let mut models: Vec<_> = app.models.iter().collect();
    models.sort_by_key(|m| {
        let rank = recent.iter().position(|r| **r == m.model);
        (
            Some(&m.model) != current,
            !m.configured,
            rank.unwrap_or(usize::MAX),
        )
    });
    models
        .into_iter()
        .map(|m| {
            let budgets = match m
                .capabilities
                .extensions
                .get("aether.cost_budget")
                .and_then(|value| value.as_bool())
            {
                Some(true) => "token + dollar caps",
                Some(false) => "token caps only",
                None => "budget support unknown",
            };
            let mut detail = format!("{budgets}{}{}", sep(theme), m.provider_name);
            let note = if Some(&m.model) == current {
                Some("current")
            } else if Some(&m.model) == session_default {
                Some("new agents")
            } else if m.is_default {
                Some("configured default")
            } else if !m.configured {
                Some("needs sign-in")
            } else {
                None
            };
            if let Some(note) = note {
                detail = format!("{note}{}{detail}", sep(theme));
            }
            let row = PickerRow::new(m.model.to_string(), detail);
            let row = if m.configured { row } else { row.disabled() };
            (row, m.model.clone())
        })
        .collect()
}

fn effort_rows(theme: &Theme) -> Vec<(PickerRow, Effort)> {
    let dot = theme.glyphs().dot;
    EFFORTS
        .iter()
        .map(|(effort, about)| {
            let about = about.replace('·', dot);
            (PickerRow::new(effort.to_string(), about), *effort)
        })
        .collect()
}
