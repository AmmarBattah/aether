//! `/settings` key capture; navigation stays fixed while this view owns input.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
};

use super::{BottomView, ViewKind, Window, body_room, frame_height, render_frame, row, title};
use crate::app::App;
use crate::keys::{KeyAction, KeyChord};
use crate::ui::bottom::{ViewAction, ViewOutcome};
use crate::ui::theme::tokens::Theme;

#[derive(Debug, Default)]
pub struct KeysView {
    cursor: usize,
    capturing: bool,
    notice: Option<String>,
}

impl KeysView {
    fn input(&mut self, key: KeyEvent, app: &App) -> ViewOutcome {
        if key.kind != KeyEventKind::Press {
            return ViewOutcome::Consumed;
        }
        if self.capturing {
            if key.code == KeyCode::Esc && key.modifiers.is_empty() {
                self.capturing = false;
                self.notice = None;
                return ViewOutcome::Consumed;
            }
            let action = KeyAction::ALL[self.cursor];
            let result = KeyChord::from_event(key).and_then(|chord| {
                app.keys.clone().assign(action, chord)?;
                Ok(chord)
            });
            return match result {
                Ok(chord) => {
                    self.capturing = false;
                    self.notice = None;
                    ViewOutcome::Act(vec![ViewAction::SetKey {
                        action,
                        chord: Some(chord),
                    }])
                }
                Err(error) => {
                    self.notice = Some(error);
                    ViewOutcome::Consumed
                }
            };
        }
        if !key.modifiers.is_empty() {
            return super::unhandled(key);
        }
        self.notice = None;
        match key.code {
            KeyCode::Esc => ViewOutcome::Close(Vec::new()),
            KeyCode::Up => {
                self.cursor = self.cursor.saturating_sub(1);
                ViewOutcome::Consumed
            }
            KeyCode::Down => {
                self.cursor = (self.cursor + 1).min(KeyAction::ALL.len());
                ViewOutcome::Consumed
            }
            KeyCode::Home => {
                self.cursor = 0;
                ViewOutcome::Consumed
            }
            KeyCode::End => {
                self.cursor = KeyAction::ALL.len();
                ViewOutcome::Consumed
            }
            KeyCode::Enter if self.cursor == KeyAction::ALL.len() => {
                ViewOutcome::Act(vec![ViewAction::ResetKeys])
            }
            KeyCode::Enter => {
                self.capturing = true;
                ViewOutcome::Consumed
            }
            KeyCode::Backspace | KeyCode::Char('r') if self.cursor < KeyAction::ALL.len() => {
                let action = KeyAction::ALL[self.cursor];
                if let Err(error) = app.keys.clone().reset(action) {
                    self.notice = Some(error);
                    return ViewOutcome::Consumed;
                }
                ViewOutcome::Act(vec![ViewAction::SetKey {
                    action,
                    chord: None,
                }])
            }
            _ => super::unhandled(key),
        }
    }
}

impl BottomView for KeysView {
    fn kind(&self) -> Option<ViewKind> {
        Some(ViewKind::Keys)
    }
    fn handle_key(&mut self, key: KeyEvent, app: &App) -> ViewOutcome {
        self.input(key, app)
    }
    fn handle_chord(&mut self, key: KeyEvent, app: &App) -> ViewOutcome {
        self.input(key, app)
    }
    fn wheel(&mut self, up: bool, app: &App) -> ViewOutcome {
        if self.capturing {
            ViewOutcome::Consumed
        } else {
            self.input(super::wheel_key(up), app)
        }
    }
    fn desired_height(&self, _width: u16, _app: &App) -> u16 {
        frame_height(KeyAction::ALL.len() + 2)
    }
    fn hints(&self) -> &'static str {
        if self.capturing {
            "press the new key · esc cancels"
        } else {
            "enter rebind · r reset · esc back"
        }
    }
    fn render(&self, frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
        let status = self.notice.as_deref().unwrap_or(if self.capturing {
            "Press the new key... Esc cancels"
        } else {
            "Customised rows marked *; Ctrl+C, Ctrl+D and Esc stay fixed"
        });
        let mut body = vec![row(status, theme.secondary())];
        let mut rows: Vec<_> = KeyAction::ALL
            .iter()
            .enumerate()
            .map(|(i, action)| {
                let selected = self.cursor == i;
                let style = if selected {
                    theme.accent().patch(theme.title())
                } else {
                    theme.text()
                };
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        format!(
                            "{} {:<27} ",
                            if selected { theme.glyphs().prompt } else { " " },
                            action
                                .label()
                                .replace(" · ", if theme.ascii() { ", " } else { " · " })
                        ),
                        style,
                    ),
                    Span::styled(app.keys.display(*action, theme.ascii()), style),
                    Span::styled(
                        if app.keys.customised(*action) {
                            "  *"
                        } else {
                            ""
                        },
                        theme.secondary(),
                    ),
                ])
            })
            .collect();
        rows.push(row(
            format!(
                "{} Reset all",
                if self.cursor == KeyAction::ALL.len() {
                    theme.glyphs().prompt
                } else {
                    " "
                }
            ),
            theme.accent(),
        ));
        let window = Window::following(rows.len(), body_room(area).saturating_sub(1), self.cursor);
        body.extend(window.lines(rows, theme));
        let hints = if self.capturing {
            &[("esc", "cancel")][..]
        } else {
            &[
                ("↑↓", "choose"),
                ("enter", "rebind"),
                ("r/backspace", "reset"),
                ("esc", "back"),
            ][..]
        };
        render_frame(frame, area, title("Keys", "", theme), body, hints, theme);
    }
}

#[cfg(test)]
mod tests;
