#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

//! One registry is the source of truth for the palette, the keys, the `?`
//! panel and the docs (VISION §9.5). These tests are what keeps it one:
//! a command the parser knows but the palette does not, or a key in the
//! table that no longer does anything, fails the build.

use aether_tui::app::{App, AppEvent, TuiOptions};
use aether_tui::commands::{
    self, COMMANDS, InputAction, KEYS, KeyGroup, KeyScope, SlashCommand, parse,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[path = "registry/help.rs"]
mod help;
#[path = "registry/keys.rs"]
mod keys;
#[path = "registry/navigation.rs"]
mod navigation;
#[path = "registry/vocabulary.rs"]
mod vocabulary;

fn press(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    app.reduce(AppEvent::Key(KeyEvent::new(code, mods)));
}

fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        press(app, KeyCode::Char(c), KeyModifiers::NONE);
    }
}

/// The screen as rows, one char per terminal cell, so an index is a column.
fn screen_rows(app: &App, width: u16, height: u16) -> Vec<Vec<char>> {
    aether_tui::testing::render_to(app, width, height)
        .lines()
        .map(|row| row.chars().collect())
        .collect()
}

/// `shown` is `full`, or a non-empty prefix of it that says it was cut.
fn whole_or_elided(shown: &str, full: &str) -> bool {
    shown == full
        || shown
            .strip_suffix('\u{2026}')
            .is_some_and(|head| !head.is_empty() && full.starts_with(head))
}

/// Where `needle` starts in `row`, as a column.
fn find_in(row: &[char], needle: &str) -> Option<usize> {
    let needle: Vec<char> = needle.chars().collect();
    row.windows(needle.len())
        .position(|w| w == needle.as_slice())
}

/// A rendered `name  description` cell split at its first double space.
fn split_columns(cell: &[char]) -> (String, Option<usize>, String) {
    let text: String = cell.iter().collect();
    let Some(gap) = find_in(cell, "  ") else {
        return (text.trim_end().to_owned(), None, String::new());
    };
    let head: String = cell[..gap].iter().collect();
    let tail_at = (gap..cell.len()).find(|i| cell[*i] != ' ');
    let tail: String = tail_at
        .map(|at| cell[at..].iter().collect::<String>())
        .unwrap_or_default();
    (head, tail_at, tail.trim_end().to_owned())
}
