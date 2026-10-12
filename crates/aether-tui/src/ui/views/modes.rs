use super::{ViewKind, Window, body_room, frame_height, render_frame, row, title, unhandled};
use crate::app::App;
use crate::commands::SlashCommand;
use crate::ui::bottom::{BottomView, ViewAction, ViewOutcome};
use crate::ui::theme::Theme;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{Frame, layout::Rect, text::Line};

#[derive(Debug, Default)]
pub struct ModesView {
    cursor: usize,
}

pub fn body(app: &App, theme: &Theme) -> Vec<Line<'static>> {
    rows(app, theme).into_iter().map(|(_, line)| line).collect()
}

fn rows(app: &App, theme: &Theme) -> Vec<(Option<String>, Line<'static>)> {
    let mut lines = Vec::new();
    let dot = theme.glyphs().dot;
    for mode in app.view_modes.ordered() {
        let selected = app.view_mode == mode.name;
        let marker = if selected { theme.glyphs().prompt } else { " " };
        let scope = mode
            .definition
            .scope
            .as_ref()
            .map(|scope| format!("name={:?} role={:?}", scope.name, scope.role))
            .unwrap_or_default();
        let inactive = if selected
            && !app.details
            && app.policy(app.selected).definition.is_none()
            && mode.name != "standard"
        {
            " scope unmatched, using standard"
        } else {
            ""
        };
        let style = if selected {
            theme.accent()
        } else {
            theme.text()
        };
        lines.push((
            Some(mode.name.clone()),
            row(
                format!(
                    "{marker} {} ({}) {dot} {}",
                    mode.name,
                    mode.definition.label.as_deref().unwrap_or(&mode.name),
                    mode.definition.description.as_deref().unwrap_or(""),
                ),
                style,
            ),
        ));
        lines.push((
            Some(mode.name.clone()),
            row(
                format!("  source: {}", super::clean(&mode.source)),
                theme.secondary(),
            ),
        ));
        if !scope.is_empty() {
            lines.push((
                Some(mode.name.clone()),
                row(format!("  scope: {scope}{inactive}"), theme.secondary()),
            ));
        }
    }
    for problem in &app.view_modes.diagnostics {
        lines.push((
            None,
            row(
                super::clean(&format!("Unavailable {dot} {}", problem.source)),
                theme.failure(),
            ),
        ));
        lines.extend(
            problem
                .reason
                .lines()
                .map(|reason| (None, row(super::clean(reason), theme.failure()))),
        );
    }
    lines
}

impl BottomView for ModesView {
    fn kind(&self) -> Option<ViewKind> {
        Some(ViewKind::Modes)
    }
    fn wheel(&mut self, up: bool, app: &App) -> ViewOutcome {
        self.handle_key(super::wheel_key(up), app)
    }
    fn handle_key(&mut self, key: KeyEvent, app: &App) -> ViewOutcome {
        let count = body(app, &app.theme).len();
        self.cursor = self.cursor.min(count.saturating_sub(1));
        match key.code {
            KeyCode::Esc => ViewOutcome::Close(Vec::new()),
            KeyCode::Up => {
                self.cursor = self.cursor.saturating_sub(1);
                ViewOutcome::Consumed
            }
            KeyCode::Down => {
                self.cursor = (self.cursor + 1).min(count.saturating_sub(1));
                ViewOutcome::Consumed
            }
            KeyCode::Char('r') => ViewOutcome::Act(vec![ViewAction::Command(
                SlashCommand::ViewMode(Some("reload".into())),
            )]),
            KeyCode::Enter => match rows(app, &app.theme)
                .get(self.cursor)
                .and_then(|(name, _)| name.as_ref())
            {
                Some(name) => ViewOutcome::Close(vec![ViewAction::Command(
                    SlashCommand::ViewMode(Some(name.clone())),
                )]),
                None => ViewOutcome::Consumed,
            },
            _ => unhandled(key),
        }
    }
    fn desired_height(&self, _: u16, app: &App) -> u16 {
        frame_height(body(app, &app.theme).len())
    }
    fn render(&self, frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
        let mut lines = body(app, theme);
        let cursor = self.cursor.min(lines.len().saturating_sub(1));
        if let Some(line) = lines.get_mut(cursor) {
            line.style = theme.tint().patch(theme.title());
        }
        let window = Window::following(lines.len(), body_room(area), cursor);
        render_frame(
            frame,
            area,
            title(
                "View modes",
                app.mode_label().unwrap_or_else(|| "standard".into()),
                theme,
            ),
            window.lines(lines, theme),
            &[
                ("↑↓", "choose"),
                ("enter", "apply"),
                ("r", "reload"),
                ("esc", "back"),
            ],
            theme,
        );
    }
}
