//! Stage 2: board keys while it has focus (↑↓ ←→ Home End Enter `.` Esc;
//! Ctrl+G, Ctrl+B and other keys pass to global dispatch). Owner: W6 (board model helpers: W3).

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

use super::Dispatch;
use crate::app::{App, Effect};
use crate::keys::KeyAction;

impl App {
    pub(super) fn board_key(&mut self, key: KeyEvent) -> Dispatch {
        if !self.board.focused() {
            return Dispatch::Pass;
        }
        // Legacy terminals send Shift+d as a bare 'D', same as Caps Lock; only a
        // reported CAPS_LOCK state tells them apart.
        if let KeyCode::Char(c) = key.code
            && (key.modifiers == KeyModifiers::SHIFT
                || (key.modifiers.is_empty()
                    && c.is_ascii_uppercase()
                    && !key.state.contains(KeyEventState::CAPS_LOCK)))
            && matches!(c.to_ascii_uppercase(), 'X' | 'D' | 'P' | 'I')
        {
            return Dispatch::Handled(if key.kind == KeyEventKind::Press {
                self.board_action(c.to_ascii_uppercase())
            } else {
                Vec::new()
            });
        }
        if [KeyAction::History, KeyAction::Find, KeyAction::Workspace]
            .into_iter()
            .any(|a| self.keys.matches(a, key))
        {
            self.leave_board();
            return Dispatch::Pass;
        }
        if !key.modifiers.is_empty() {
            return Dispatch::Pass;
        }
        let effects = match key.code {
            KeyCode::Up => {
                self.board_move(-1);
                Vec::new()
            }
            KeyCode::Down => {
                self.board_move(1);
                Vec::new()
            }
            KeyCode::Left => {
                self.board_fold();
                Vec::new()
            }
            KeyCode::Right => {
                self.board_unfold();
                Vec::new()
            }
            KeyCode::Home => {
                self.board_home();
                Vec::new()
            }
            KeyCode::End => {
                self.board_end();
                Vec::new()
            }
            KeyCode::Enter => self.view_board_cursor(),
            KeyCode::Char('.') => self.board_toggle_archived(),
            KeyCode::Esc => {
                self.leave_board();
                Vec::new()
            }
            _ => return Dispatch::Pass,
        };
        Dispatch::Handled(effects)
    }

    /// Enter: view the cursor's agent; docked hands focus back, the overlay closes.
    /// An agent waiting on approvals gets them shown again.
    fn view_board_cursor(&mut self) -> Vec<Effect> {
        let Some(agent) = self.board.cursor else {
            return Vec::new();
        };
        self.leave_board();
        if self
            .approvals
            .iter()
            .any(|request| request.agent_id == agent)
        {
            self.reopen_approvals(agent);
        }
        self.select(agent)
    }
}
