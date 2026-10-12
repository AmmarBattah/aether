use ratatui::style::Color;

/// What the terminal can actually show. Every token is authored once with a
/// 256-colour index and, where VISION gives one, a truecolor hex; the depth
/// decides which is used and folds the rest down to the sixteen ANSI colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorDepth {
    Truecolor,
    #[default]
    Indexed256,
    Basic16,
}

impl ColorDepth {
    /// Read the terminal's own claims. `COLORTERM` is the most reliable
    /// truecolor signal; a `TERM` without `256color` means sixteen.
    pub fn detect() -> ColorDepth {
        let var = |k: &str| std::env::var(k).unwrap_or_default();
        ColorDepth::from_env(&var("COLORTERM"), &var("TERM"))
    }

    /// The decision on its own, so it is testable without the environment.
    pub fn from_env(colorterm: &str, term: &str) -> ColorDepth {
        let colorterm = colorterm.to_ascii_lowercase();
        if colorterm.contains("truecolor") || colorterm.contains("24bit") {
            return ColorDepth::Truecolor;
        }
        // `xterm-direct` and friends are the direct-colour TERMs: truecolor.
        let term = term.to_ascii_lowercase();
        if term.contains("direct") {
            return ColorDepth::Truecolor;
        }
        if term.contains("256color") {
            return ColorDepth::Indexed256;
        }
        ColorDepth::Basic16
    }
}

/// Fold a 256-colour index onto the sixteen ANSI colours. A channel counts
/// towards the hue only when it is within one step of the strongest, so a
/// blue with some green stays blue rather than turning cyan.
pub(super) fn basic16(idx: u8) -> Color {
    if idx < 16 {
        return ansi(idx);
    }
    if idx >= 232 {
        return match idx {
            232..=240 => Color::Black,
            241..=250 => Color::DarkGray,
            251..=253 => Color::Gray,
            _ => Color::White,
        };
    }
    let c = idx - 16;
    let (r, g, b) = (c / 36, (c % 36) / 6, c % 6);
    let peak = r.max(g).max(b);
    if peak == 0 {
        return Color::Black;
    }
    let near = |v: u8| u8::from(v + 1 >= peak);
    let base = match near(r) | (near(g) << 1) | (near(b) << 2) {
        0b001 => 1, // red
        0b010 => 2, // green
        0b011 => 3, // yellow
        0b100 => 4, // blue
        0b101 => 5, // magenta
        0b110 => 6, // cyan
        _ => 7,     // white: no channel stands out
    };
    ansi(if peak >= 3 { base + 8 } else { base })
}

fn ansi(n: u8) -> Color {
    match n {
        0 => Color::Black,
        1 => Color::Red,
        2 => Color::Green,
        3 => Color::Yellow,
        4 => Color::Blue,
        5 => Color::Magenta,
        6 => Color::Cyan,
        7 => Color::Gray,
        8 => Color::DarkGray,
        9 => Color::LightRed,
        10 => Color::LightGreen,
        11 => Color::LightYellow,
        12 => Color::LightBlue,
        13 => Color::LightMagenta,
        14 => Color::LightCyan,
        _ => Color::White,
    }
}
