//! `/settings`: theme, motion, mouse off/scroll/full, ascii. Owner: W7.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use super::{Hint, frame_height, home_path, render_frame, row, title, unhandled};
use crate::app::App;
use crate::commands::SlashCommand;
use crate::prefs::MouseMode;
use crate::ui::bottom::{BottomView, ViewAction, ViewOutcome};
use crate::ui::theme::ThemeChoice;
use crate::ui::theme::tokens::Theme;

const HINTS: &[Hint] = &[("↑↓", "choose"), ("←→", "change"), ("esc", "back")];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Setting {
    Theme,
    Motion,
    Mouse,
    Ascii,
    Keys,
    Mode,
}

const SETTINGS: [Setting; 6] = [
    Setting::Theme,
    Setting::Motion,
    Setting::Mouse,
    Setting::Ascii,
    Setting::Keys,
    Setting::Mode,
];

impl Setting {
    fn label(self) -> &'static str {
        match self {
            Setting::Mode => "View mode",
            Setting::Theme => "Theme",
            Setting::Motion => "Motion",
            Setting::Mouse => "Mouse",
            Setting::Ascii => "ASCII",
            Setting::Keys => "Keys",
        }
    }

    /// The theme row lists only the one in effect; `/theme` shows them all.
    fn values(self) -> &'static [&'static str] {
        match self {
            Setting::Theme | Setting::Mode | Setting::Keys => &[],
            Setting::Motion => &["full", "reduced"],
            Setting::Mouse => &["off", "scroll"],
            Setting::Ascii => &["off", "on"],
        }
    }

    /// Index into `values()` of what is in effect now.
    fn current(self, app: &App) -> usize {
        match self {
            Setting::Mode => app
                .view_modes
                .ordered()
                .position(|m| m.name == app.view_mode)
                .unwrap_or(0),
            Setting::Theme => ThemeChoice::builtins()
                .iter()
                .position(|choice| *choice == app.theme_choice)
                .unwrap_or(0),
            Setting::Motion => usize::from(app.motion.reduced()),
            Setting::Mouse => match app.mouse {
                MouseMode::Off => 0,
                MouseMode::Scroll | MouseMode::Full => 1,
            },
            Setting::Ascii => usize::from(app.options.ascii),
            Setting::Keys => 0,
        }
    }
}

/// One row per setting; ←/→ or Enter step the highlighted one. Each step
/// goes out as the slash command a person would type, and the view stays open.
#[derive(Debug, Default)]
pub struct SettingsView {
    cursor: usize,
}

impl SettingsView {
    fn step(&self, app: &App, forward: bool) -> ViewOutcome {
        let setting = SETTINGS[self.cursor];
        if setting == Setting::Mode {
            let names: Vec<_> = app.view_modes.ordered().map(|m| m.name.clone()).collect();
            let current = setting.current(app);
            let next = if forward {
                (current + 1) % names.len()
            } else {
                (current + names.len() - 1) % names.len()
            };
            return ViewOutcome::Act(vec![ViewAction::Command(SlashCommand::ViewMode(Some(
                names[next].clone(),
            )))]);
        }
        if setting == Setting::Keys {
            return ViewOutcome::Act(vec![ViewAction::Open(super::ViewKind::Keys)]);
        }
        if setting == Setting::Theme {
            return step_theme(app, forward);
        }
        if setting == Setting::Ascii {
            return ViewOutcome::Act(vec![ViewAction::Flash(
                "ASCII is set at launch: aether --ascii".into(),
            )]);
        }
        let count = setting.values().len();
        let current = setting.current(app);
        let next = if forward {
            (current + 1) % count
        } else {
            (current + count - 1) % count
        };
        let command = command(setting, next);
        ViewOutcome::Act(vec![ViewAction::Command(command)])
    }
}

impl BottomView for SettingsView {
    fn wheel(&mut self, up: bool, app: &App) -> ViewOutcome {
        self.handle_key(super::wheel_key(up), app)
    }

    fn kind(&self) -> Option<super::ViewKind> {
        Some(super::ViewKind::Settings)
    }

    fn handle_key(&mut self, key: KeyEvent, app: &App) -> ViewOutcome {
        let last = SETTINGS.len() - 1;
        match key.code {
            KeyCode::Esc => ViewOutcome::Close(Vec::new()),
            KeyCode::Up => {
                self.cursor = self.cursor.saturating_sub(1);
                ViewOutcome::Consumed
            }
            KeyCode::Down => {
                self.cursor = (self.cursor + 1).min(last);
                ViewOutcome::Consumed
            }
            KeyCode::Enter if SETTINGS[self.cursor] == Setting::Theme => {
                ViewOutcome::Act(vec![ViewAction::Command(SlashCommand::Theme)])
            }
            KeyCode::Enter if SETTINGS[self.cursor] == Setting::Mode => {
                ViewOutcome::Act(vec![ViewAction::Command(SlashCommand::ViewMode(None))])
            }
            KeyCode::Right | KeyCode::Enter | KeyCode::Char(' ') => self.step(app, true),
            KeyCode::Left => self.step(app, false),
            KeyCode::Char(c @ '1'..='6') => {
                self.cursor = usize::from(c as u8 - b'1');
                ViewOutcome::Consumed
            }
            _ => unhandled(key),
        }
    }

    fn desired_height(&self, _width: u16, app: &App) -> u16 {
        frame_height(SETTINGS.len() + 1 + path_lines(app, &app.theme).len())
    }

    fn render(&self, frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
        let mut lines: Vec<_> = SETTINGS
            .iter()
            .enumerate()
            .map(|(index, setting)| setting_line(*setting, app, index, index == self.cursor, theme))
            .collect();
        lines.push(Line::default());
        lines.extend(path_lines(app, theme));
        render_frame(
            frame,
            area,
            title("Settings", "", theme),
            lines,
            HINTS,
            theme,
        );
    }
}

/// `› 1. Theme    auto · dark · light · mono`, the value in effect in the
/// accent and the rest secondary.
fn setting_line(
    setting: Setting,
    app: &App,
    index: usize,
    selected: bool,
    theme: &Theme,
) -> Line<'static> {
    let label_width = SETTINGS
        .iter()
        .map(|setting| setting.label().width())
        .max()
        .unwrap_or(0)
        + 1;
    let current = setting.current(app);
    let dot = theme.glyphs().dot;
    let (marker, label) = if selected {
        (
            theme.accent().patch(theme.title()),
            theme.text().patch(theme.title()),
        )
    } else {
        (theme.secondary(), theme.text())
    };
    let prompt = if selected { theme.glyphs().prompt } else { " " };
    let mut spans = vec![
        Span::styled(format!("{prompt} "), marker),
        Span::styled(format!("{}. ", index + 1), theme.secondary()),
        Span::styled(
            format!(
                "{}{}",
                setting.label(),
                " ".repeat(label_width - setting.label().width())
            ),
            label,
        ),
    ];
    for (at, value) in setting.values().iter().enumerate() {
        if at > 0 {
            spans.push(Span::styled(format!(" {dot} "), theme.secondary()));
        }
        let style = if at == current {
            theme.accent().patch(theme.title())
        } else {
            theme.secondary()
        };
        spans.push(Span::styled(*value, style));
    }
    if setting == Setting::Mode {
        spans.push(Span::styled(
            app.mode_label().unwrap_or_else(|| "standard".into()),
            theme.accent(),
        ));
        spans.push(Span::styled(
            "  enter for definitions and notices",
            theme.secondary(),
        ));
    }
    if setting == Setting::Theme {
        spans.push(Span::styled(
            app.theme_choice.to_string(),
            theme.accent().patch(theme.title()),
        ));
        spans.push(Span::styled("  enter for every theme", theme.secondary()));
    }
    if setting == Setting::Keys {
        spans.push(Span::styled(
            "enter to edit key bindings",
            theme.secondary(),
        ));
    }
    if setting == Setting::Ascii {
        spans.push(Span::styled("  set with --ascii", theme.secondary()));
    }
    Line::from(spans)
}

/// Where preferences are saved and colour overrides live, so a person can
/// find them; one dim row each.
fn path_lines(app: &App, theme: &Theme) -> Vec<Line<'static>> {
    let Some(dir) = &app.options.prefs_dir else {
        return vec![row("Preferences are not saved this run", theme.secondary())];
    };
    let saved = home_path(&dir.join(crate::prefs::FILE));
    let overrides = home_path(&dir.join("config.toml"));
    vec![
        row(format!("Saved to   {saved}"), theme.secondary()),
        row(format!("Overrides  {overrides}"), theme.secondary()),
    ]
}

/// ←/→ on the theme row: the next built-in; a theme file steps from `auto`.
fn step_theme(app: &App, forward: bool) -> ViewOutcome {
    let builtins = ThemeChoice::builtins();
    let count = builtins.len();
    let current = Setting::Theme.current(app);
    let next = if forward {
        (current + 1) % count
    } else {
        (current + count - 1) % count
    };
    let choice = builtins[next].clone();
    ViewOutcome::Act(vec![ViewAction::Command(SlashCommand::SetTheme(choice))])
}

/// The command for `setting` set to `values()[value]`.
fn command(setting: Setting, value: usize) -> SlashCommand {
    const MICE: [MouseMode; 2] = [MouseMode::Off, MouseMode::Scroll];
    match setting {
        Setting::Mode => unreachable!("mode is handled in step"),
        Setting::Theme => unreachable!("the theme row steps through step_theme"),
        Setting::Motion => SlashCommand::Motion(Some(value == 1)),
        Setting::Mouse => SlashCommand::Mouse(Some(MICE[value])),
        Setting::Keys => unreachable!("Keys opens a view"),
        Setting::Ascii => unreachable!("ASCII is launch-only and handled in step"),
    }
}
