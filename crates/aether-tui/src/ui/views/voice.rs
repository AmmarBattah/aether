//! `/voice`: voice setup and toggles. Owner: W7.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Line;

use super::picker::{Picker, PickerEvent, PickerRow};
use super::{
    Hint, ViewKind, Window, body_room, frame_height, render_frame, row, sep, title, unhandled,
};
use crate::app::App;
use crate::commands::SlashCommand;
use crate::ui::bottom::{BottomView, ViewAction, ViewOutcome};
use crate::ui::theme::tokens::Theme;
use crate::voice::Action;
use crate::voice::setup::{self, ModelKind};

const HINTS: &[Hint] = &[
    ("↑↓", "choose"),
    ("enter", "toggle or install"),
    ("esc", "back"),
];
const CONFIRM_HINTS: &[Hint] = &[("enter", "install"), ("↑↓", "scroll"), ("esc", "back")];
const BACK_HINTS: &[Hint] = &[("esc", "back")];
const INSTALL_HINTS: &[Hint] = &[
    ("c", "cancel the install"),
    ("esc", "back, it keeps running"),
];
/// The blank row and the status row under the list.
const STATUS_ROWS: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Choice {
    Questions,
    Replies,
    Check,
    Install(ModelKind),
}

/// Settings first, then one row per installable backend. Toggles run the
/// `/voice` subcommand a person would type and keep the view open.
#[derive(Debug)]
pub struct VoiceView {
    picker: Picker<Choice>,
    /// The newest missing-backend check this view has already followed.
    led: u64,
}

impl Default for VoiceView {
    fn default() -> Self {
        VoiceView {
            picker: Picker::new("Voice", Vec::new()),
            led: 0,
        }
    }
}

impl VoiceView {
    /// A failure from before the popup opened was answered by its notice;
    /// only one that happens while it is open moves the selection.
    pub fn new(app: &App) -> Self {
        VoiceView {
            led: app.voice.missing_serial(),
            ..Self::default()
        }
    }

    /// The rows from fresh app data, selecting the install row of a backend a
    /// check found missing since this view last followed one.
    fn picker(&self, app: &App) -> Picker<Choice> {
        let mut picker = self.picker.with_items(rows(app));
        let Some(model) = app.voice.install_lead(self.led) else {
            return picker;
        };
        let row = picker
            .visible()
            .into_iter()
            .position(|index| picker.items[index].1 == Choice::Install(model));
        // A typed filter may hide the row; the notice still names it.
        if let Some(row) = row {
            picker.cursor = row;
        }
        picker
    }
}

impl BottomView for VoiceView {
    fn wheel(&mut self, up: bool, app: &App) -> ViewOutcome {
        self.handle_key(super::wheel_key(up), app)
    }

    fn kind(&self) -> Option<ViewKind> {
        Some(ViewKind::Voice)
    }

    fn handle_key(&mut self, key: KeyEvent, app: &App) -> ViewOutcome {
        self.picker = self.picker(app);
        // Followed once: from here the selection is the person's again.
        self.led = app.voice.missing_serial();
        match self.picker.handle_key(key) {
            PickerEvent::Chosen(index) => choose(self.picker.items[index].1, app),
            PickerEvent::Moved | PickerEvent::Filtered => ViewOutcome::Consumed,
            PickerEvent::Cancelled => ViewOutcome::Close(Vec::new()),
            PickerEvent::Ignored => unhandled(key),
        }
    }

    fn desired_height(&self, _width: u16, app: &App) -> u16 {
        frame_height(self.picker(app).body_len() + STATUS_ROWS)
    }

    fn render(&self, frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
        let picker = self.picker(app);
        let room = body_room(area);
        // The status row gives way first on a short pane.
        let status = room > STATUS_ROWS;
        let list_room = if status { room - STATUS_ROWS } else { room };
        let numbered = picker.items.len() <= 9;
        let mut body = picker.lines(area.width, list_room, theme, true, numbered);
        if status {
            body.push(Line::default());
            body.push(status_line(app, theme));
        }
        render_frame(frame, area, picker.title_spans(theme), body, HINTS, theme);
    }
}

fn choose(choice: Choice, app: &App) -> ViewOutcome {
    let voice = |action| ViewAction::Command(SlashCommand::Voice(action));
    match choice {
        Choice::Questions if app.voice.enabled => ViewOutcome::Act(vec![voice(Action::Off)]),
        Choice::Questions => ViewOutcome::Act(vec![voice(Action::On)]),
        Choice::Replies if app.voice.saved.replies => {
            ViewOutcome::Act(vec![voice(Action::RepliesOff)])
        }
        Choice::Replies => ViewOutcome::Act(vec![voice(Action::RepliesOn)]),
        Choice::Check => ViewOutcome::Act(vec![voice(Action::Check)]),
        // Nothing is downloaded until the disclosure has been read and confirmed.
        Choice::Install(model) => ViewOutcome::Act(vec![ViewAction::Push(Box::new(
            VoiceConfirmView::new(model),
        ))]),
    }
}

fn rows(app: &App) -> Vec<(PickerRow, Choice)> {
    let theme = &app.theme;
    let dot = sep(theme);
    let on_off = |on: bool| if on { "on" } else { "off" };
    let questions = format!(
        "{}{dot}speak new agent questions aloud",
        on_off(app.voice.enabled)
    );
    let replies = format!(
        "{}{dot}a global shortcut records and sends a reply without review",
        on_off(app.voice.saved.replies)
    );
    let mut rows = vec![
        (
            PickerRow::new("Speak questions", questions),
            Choice::Questions,
        ),
        (PickerRow::new("Voice replies", replies), Choice::Replies),
        (
            PickerRow::new("Check", "find the speech backends again"),
            Choice::Check,
        ),
    ];
    rows.extend(ModelKind::ALL.map(|model| {
        let label = format!("Install {}", model.label());
        (
            PickerRow::new(label, blurb(model, theme)),
            Choice::Install(model),
        )
    }));
    rows
}

fn status_line(app: &App, theme: &Theme) -> Line<'static> {
    let text = if app.voice.available {
        app.voice_key_hint(app.voice.status().unwrap_or("voice off"))
    } else {
        "local voice needs the interactive TUI on Linux or macOS; see docs/voice.md".to_owned()
    };
    row(text, theme.secondary())
}

/// The backend's size and language note, with the theme's separator.
fn blurb(model: ModelKind, theme: &Theme) -> String {
    model.blurb().replace('·', theme.glyphs().dot)
}

/// The one explicit gate before any download: the backend's own disclosure,
/// verbatim, or the local tools it still needs.
#[derive(Debug)]
pub struct VoiceConfirmView {
    model: ModelKind,
    /// Probed once when the view opens; the PATH does not change under it.
    missing: Vec<setup::Prerequisite>,
    offset: usize,
}

impl VoiceConfirmView {
    pub fn new(model: ModelKind) -> Self {
        VoiceConfirmView {
            model,
            missing: setup::missing_prerequisites(model),
            offset: 0,
        }
    }

    fn body(&self, width: u16, theme: &Theme) -> Vec<Line<'static>> {
        let mut text = Vec::new();
        if self.missing.is_empty() {
            text.push((blurb(self.model, theme), theme.text()));
            text.push((self.model.notice().to_owned(), theme.secondary()));
            text.push((
                "Your saved voice settings are left alone.".to_owned(),
                theme.secondary(),
            ));
        } else {
            text.push((
                "This backend needs these local tools. Aether never runs a package manager for you."
                    .to_owned(),
                theme.text(),
            ));
            for item in &self.missing {
                text.push((format!("{}: {}", item.name, item.hint), theme.secondary()));
            }
        }
        let room = usize::from(width.saturating_sub(4).max(1));
        // `GPL-3.0` and the like stay whole.
        let options =
            textwrap::Options::new(room).word_splitter(textwrap::WordSplitter::NoHyphenation);
        text.into_iter()
            .flat_map(|(text, style)| {
                textwrap::wrap(&text, &options)
                    .into_iter()
                    .map(move |part| row(part.into_owned(), style))
                    .collect::<Vec<_>>()
            })
            .collect()
    }
}

impl BottomView for VoiceConfirmView {
    fn kind(&self) -> Option<ViewKind> {
        Some(ViewKind::Voice)
    }

    fn handle_key(&mut self, key: KeyEvent, _app: &App) -> ViewOutcome {
        match key.code {
            KeyCode::Esc | KeyCode::Char('n') => ViewOutcome::Close(Vec::new()),
            KeyCode::Enter | KeyCode::Char('y') if self.missing.is_empty() => {
                ViewOutcome::Close(vec![ViewAction::Open(ViewKind::VoiceInstall(self.model))])
            }
            KeyCode::Up => {
                self.offset = self.offset.saturating_sub(1);
                ViewOutcome::Consumed
            }
            // Clamped at render, where the wrapped length is known.
            KeyCode::Down => {
                self.offset = self.offset.saturating_add(1);
                ViewOutcome::Consumed
            }
            _ => unhandled(key),
        }
    }

    fn desired_height(&self, width: u16, app: &App) -> u16 {
        frame_height(self.body(width, &app.theme).len())
    }

    fn render(&self, frame: &mut Frame, area: Rect, _app: &App, theme: &Theme) {
        let body = self.body(area.width, theme);
        let window = Window::scrolled(body.len(), body_room(area), self.offset);
        let meta = if self.missing.is_empty() {
            "downloads only after you confirm"
        } else {
            "nothing was downloaded"
        };
        let title = title(format!("Install {}", self.model.label()), meta, theme);
        let hints = if self.missing.is_empty() {
            CONFIRM_HINTS
        } else {
            BACK_HINTS
        };
        render_frame(frame, area, title, window.lines(body, theme), hints, theme);
    }
}

/// Shown while an install runs. The detailed progress stays inside the voice
/// module, so this reports `voice.status()`; the result arrives as a notice.
#[derive(Debug)]
pub struct VoiceInstallView {
    model: ModelKind,
}

impl VoiceInstallView {
    pub fn new(model: ModelKind) -> Self {
        VoiceInstallView { model }
    }
}

impl BottomView for VoiceInstallView {
    fn kind(&self) -> Option<ViewKind> {
        Some(ViewKind::VoiceInstall(self.model))
    }

    fn handle_key(&mut self, key: KeyEvent, _app: &App) -> ViewOutcome {
        match key.code {
            KeyCode::Esc => ViewOutcome::Close(Vec::new()),
            KeyCode::Char('c') => ViewOutcome::Close(vec![ViewAction::Command(
                SlashCommand::Voice(Action::Cancel),
            )]),
            _ => unhandled(key),
        }
    }

    fn desired_height(&self, _width: u16, _app: &App) -> u16 {
        frame_height(1)
    }

    fn render(&self, frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
        let title = title(format!("Install {}", self.model.label()), "", theme);
        let progress = match app.voice.install_stage(self.model) {
            Some(stage) => row(stage.to_string(), theme.secondary()),
            None => status_line(app, theme),
        };
        render_frame(frame, area, title, vec![progress], INSTALL_HINTS, theme);
    }
}
