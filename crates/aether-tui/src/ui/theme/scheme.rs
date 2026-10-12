//! Colour schemes: the semantic hues a theme authors, one set per background.
//!
//! A scheme only names colours; [`super::tokens::Theme::resolve`] turns them
//! into styles for the terminal's depth and makes them readable on its ground.

use ratatui::style::Color;

use super::tokens::{Background, Rgb};

/// Every colour a theme may set. Text, quiet text, rules and the tint are not
/// here: they are blended from the terminal's own foreground and background.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Hue {
    /// Chrome: the model chip, selection marks, find hits.
    Accent,
    /// Agents and calls in progress; the pulse runs in this hue.
    Running,
    Waiting,
    /// Needs you: approvals and questions.
    Attention,
    Success,
    Failure,
    Blocked,
    /// Tool verbs: `Ran`, `Explored`, `Edited`, `Fetched`, `Called`, `Spawned`.
    Shell,
    Read,
    Edit,
    Web,
    Call,
    Agent,
    /// The `›` before what you typed.
    User,
    Code,
    Link,
    Heading,
    DiffAdd,
    DiffRemove,
}

pub const HUES: usize = Hue::ALL.len();

impl Hue {
    pub const ALL: [Hue; 19] = [
        Hue::Accent,
        Hue::Running,
        Hue::Waiting,
        Hue::Attention,
        Hue::Success,
        Hue::Failure,
        Hue::Blocked,
        Hue::Shell,
        Hue::Read,
        Hue::Edit,
        Hue::Web,
        Hue::Call,
        Hue::Agent,
        Hue::User,
        Hue::Code,
        Hue::Link,
        Hue::Heading,
        Hue::DiffAdd,
        Hue::DiffRemove,
    ];

    /// The key a theme file uses.
    pub const fn key(self) -> &'static str {
        match self {
            Hue::Accent => "accent",
            Hue::Running => "running",
            Hue::Waiting => "waiting",
            Hue::Attention => "attention",
            Hue::Success => "success",
            Hue::Failure => "failure",
            Hue::Blocked => "blocked",
            Hue::Shell => "shell",
            Hue::Read => "read",
            Hue::Edit => "edit",
            Hue::Web => "web",
            Hue::Call => "call",
            Hue::Agent => "agent",
            Hue::User => "user",
            Hue::Code => "code",
            Hue::Link => "link",
            Hue::Heading => "heading",
            Hue::DiffAdd => "diff_add",
            Hue::DiffRemove => "diff_remove",
        }
    }

    pub fn from_key(key: &str) -> Option<Hue> {
        Hue::ALL.into_iter().find(|hue| hue.key() == key)
    }

    /// The hue on a 16-colour terminal, whose own palette is the theme there;
    /// `None` leaves the terminal's foreground.
    pub(super) fn ansi16(self) -> Option<Color> {
        Some(match self {
            // §5.1: Iris has no ANSI-16 hue of its own; magenta stands in.
            Hue::Accent | Hue::Call | Hue::User | Hue::Edit => Color::Magenta,
            Hue::Running | Hue::Code => Color::Yellow,
            Hue::Waiting | Hue::Read | Hue::Link => Color::Blue,
            Hue::Attention | Hue::Agent => Color::LightMagenta,
            Hue::Success | Hue::DiffAdd => Color::Green,
            Hue::Failure | Hue::DiffRemove => Color::Red,
            Hue::Blocked => Color::LightRed,
            Hue::Shell | Hue::Web => Color::Cyan,
            Hue::Heading => return None,
        })
    }
}

/// One colour per [`Hue`], for one background.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hues([Rgb; HUES]);

impl Hues {
    pub fn get(&self, hue: Hue) -> Rgb {
        self.0[hue as usize]
    }

    fn patched(mut self, patch: &HuePatch) -> Hues {
        for hue in Hue::ALL {
            if let Some(rgb) = patch.get(hue) {
                self.0[hue as usize] = rgb;
            }
        }
        self
    }
}

/// Some hues set, the rest inherited: a theme file or the overrides file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HuePatch([Option<Rgb>; HUES]);

impl HuePatch {
    pub fn get(&self, hue: Hue) -> Option<Rgb> {
        self.0[hue as usize]
    }

    pub fn set(&mut self, hue: Hue, rgb: Rgb) {
        self.0[hue as usize] = Some(rgb);
    }

    pub fn is_empty(&self) -> bool {
        self.0.iter().all(Option::is_none)
    }
}

/// A theme's hues for a dark and for a light terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scheme {
    pub dark: Hues,
    pub light: Hues,
}

impl Default for Scheme {
    fn default() -> Self {
        Builtin::Aether.scheme()
    }
}

impl Scheme {
    pub fn hues(&self, background: Background) -> Hues {
        match background {
            Background::Dark => self.dark,
            Background::Light => self.light,
        }
    }

    /// `both` on either background, then each background's own patch.
    pub fn patched(self, both: &HuePatch, dark: &HuePatch, light: &HuePatch) -> Scheme {
        Scheme {
            dark: self.dark.patched(both).patched(dark),
            light: self.light.patched(both).patched(light),
        }
    }
}

/// The themes Aether ships.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Builtin {
    Aether,
    Nord,
    Catppuccin,
    Gruvbox,
    TokyoNight,
}

impl Builtin {
    pub const ALL: [Builtin; 5] = [
        Builtin::Aether,
        Builtin::Nord,
        Builtin::Catppuccin,
        Builtin::Gruvbox,
        Builtin::TokyoNight,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Builtin::Aether => "aether",
            Builtin::Nord => "nord",
            Builtin::Catppuccin => "catppuccin",
            Builtin::Gruvbox => "gruvbox",
            Builtin::TokyoNight => "tokyo-night",
        }
    }

    pub fn from_name(name: &str) -> Option<Builtin> {
        Builtin::ALL.into_iter().find(|b| b.name() == name)
    }

    pub fn scheme(self) -> Scheme {
        let (dark, light) = match self {
            Builtin::Aether => (AETHER_DARK, AETHER_LIGHT),
            Builtin::Nord => (NORD_DARK, NORD_LIGHT),
            Builtin::Catppuccin => (CATPPUCCIN_MOCHA, CATPPUCCIN_LATTE),
            Builtin::Gruvbox => (GRUVBOX_DARK, GRUVBOX_LIGHT),
            Builtin::TokyoNight => (TOKYO_NIGHT, TOKYO_NIGHT_DAY),
        };
        Scheme { dark, light }
    }

    /// The background each variant is tuned for: every hue is AA on it.
    pub fn ground(self, background: Background) -> Rgb {
        let hex = match (self, background) {
            (Builtin::Aether, Background::Dark) => 0x0D1117,
            (Builtin::Aether, Background::Light) => 0xFFFFFF,
            (Builtin::Nord, Background::Dark) => 0x2E3440,
            (Builtin::Nord, Background::Light) => 0xECEFF4,
            (Builtin::Catppuccin, Background::Dark) => 0x1E1E2E,
            (Builtin::Catppuccin, Background::Light) => 0xEFF1F5,
            (Builtin::Gruvbox, Background::Dark) => 0x282828,
            (Builtin::Gruvbox, Background::Light) => 0xFBF1C7,
            (Builtin::TokyoNight, Background::Dark) => 0x1A1B26,
            (Builtin::TokyoNight, Background::Light) => 0xE1E2E7,
        };
        rgb(hex)
    }
}

/// `#RRGGBB`, as theme files write a colour.
pub fn parse_hex(text: &str) -> Option<Rgb> {
    let digits = text.strip_prefix('#')?;
    if digits.len() != 6 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(digits, 16).ok().map(rgb)
}

const fn rgb(hex: u32) -> Rgb {
    Rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// A table listing every hue exactly once, checked at compile time.
const fn hues(table: [(Hue, u32); HUES]) -> Hues {
    let mut out = [Rgb(0, 0, 0); HUES];
    let mut seen = [false; HUES];
    let mut i = 0;
    while i < HUES {
        let (hue, hex) = table[i];
        assert!(!seen[hue as usize], "a hue is listed twice");
        seen[hue as usize] = true;
        out[hue as usize] = rgb(hex);
        i += 1;
    }
    Hues(out)
}

use Hue::*;

// Iris (SPEC §5.1) with Tailwind's 300 tones on dark and 600-700 on light.
const AETHER_DARK: Hues = hues([
    (Accent, 0xA5B4FC),
    (Running, 0xFCD34D),
    (Waiting, 0x7DD3FC),
    (Attention, 0xF0ABFC),
    (Success, 0x86EFAC),
    (Failure, 0xF87171),
    (Blocked, 0xFDBA74),
    (Shell, 0x5EEAD4),
    (Read, 0x93C5FD),
    (Edit, 0xC4B5FD),
    (Web, 0x67E8F9),
    (Call, 0xA5B4FC),
    (Agent, 0xF9A8D4),
    (User, 0xA5B4FC),
    (Code, 0xFDBA74),
    (Link, 0x7DD3FC),
    (Heading, 0xC7D2FE),
    (DiffAdd, 0x86EFAC),
    (DiffRemove, 0xF87171),
]);

const AETHER_LIGHT: Hues = hues([
    (Accent, 0x4F46E5),
    (Running, 0xB45309),
    (Waiting, 0x0369A1),
    (Attention, 0xA21CAF),
    (Success, 0x15803D),
    (Failure, 0xDC2626),
    (Blocked, 0xC2410C),
    (Shell, 0x0F766E),
    (Read, 0x1D4ED8),
    (Edit, 0x6D28D9),
    (Web, 0x0E7490),
    (Call, 0x4F46E5),
    (Agent, 0xBE185D),
    (User, 0x4F46E5),
    (Code, 0xC2410C),
    (Link, 0x0369A1),
    (Heading, 0x3730A3),
    (DiffAdd, 0x15803D),
    (DiffRemove, 0xDC2626),
]);

// Frost and Aurora; red and purple lifted a step to reach AA on nord0.
const NORD_DARK: Hues = hues([
    (Accent, 0x88C0D0),
    (Running, 0xEBCB8B),
    (Waiting, 0x81A1C1),
    (Attention, 0xD18B74),
    (Success, 0xA3BE8C),
    (Failure, 0xCF888F),
    (Blocked, 0xB692B0),
    (Shell, 0x8FBCBB),
    (Read, 0x81A1C1),
    (Edit, 0xB692B0),
    (Web, 0x88C0D0),
    (Call, 0x88C0D0),
    (Agent, 0xB692B0),
    (User, 0x88C0D0),
    (Code, 0xD18B74),
    (Link, 0x88C0D0),
    (Heading, 0x88C0D0),
    (DiffAdd, 0xA3BE8C),
    (DiffRemove, 0xCF888F),
]);

// Nord has no light variant; these are its hues deepened for Snow Storm.
const NORD_LIGHT: Hues = hues([
    (Accent, 0x4C6E98),
    (Running, 0x8A6500),
    (Waiting, 0x4C6E98),
    (Attention, 0xA0522D),
    (Success, 0x4C7638),
    (Failure, 0xA4535B),
    (Blocked, 0x8A5A84),
    (Shell, 0x3E7472),
    (Read, 0x4C6E98),
    (Edit, 0x8A5A84),
    (Web, 0x4C6E98),
    (Call, 0x4C6E98),
    (Agent, 0x8A5A84),
    (User, 0x4C6E98),
    (Code, 0xA0522D),
    (Link, 0x4C6E98),
    (Heading, 0x4C6E98),
    (DiffAdd, 0x4C7638),
    (DiffRemove, 0xA4535B),
]);

const CATPPUCCIN_MOCHA: Hues = hues([
    (Accent, 0xCBA6F7),
    (Running, 0xF9E2AF),
    (Waiting, 0x74C7EC),
    (Attention, 0xF5C2E7),
    (Success, 0xA6E3A1),
    (Failure, 0xF38BA8),
    (Blocked, 0xEBA0AC),
    (Shell, 0x94E2D5),
    (Read, 0x89B4FA),
    (Edit, 0xB4BEFE),
    (Web, 0x89DCEB),
    (Call, 0xCBA6F7),
    (Agent, 0xF2CDCD),
    (User, 0xCBA6F7),
    (Code, 0xFAB387),
    (Link, 0x89B4FA),
    (Heading, 0xCBA6F7),
    (DiffAdd, 0xA6E3A1),
    (DiffRemove, 0xF38BA8),
]);

// Latte's pastels deepened just enough for AA on its base.
const CATPPUCCIN_LATTE: Hues = hues([
    (Accent, 0x8839EF),
    (Running, 0x996214),
    (Waiting, 0x187788),
    (Attention, 0xA1518C),
    (Success, 0x327D22),
    (Failure, 0xD20F39),
    (Blocked, 0xC63B47),
    (Shell, 0x13797F),
    (Read, 0x1D63ED),
    (Edit, 0x5767C2),
    (Web, 0x0374A1),
    (Call, 0x8839EF),
    (Agent, 0xA25858),
    (User, 0x8839EF),
    (Code, 0xBB4908),
    (Link, 0x1D63ED),
    (Heading, 0x8839EF),
    (DiffAdd, 0x327D22),
    (DiffRemove, 0xD20F39),
]);

const GRUVBOX_DARK: Hues = hues([
    (Accent, 0xFE8019),
    (Running, 0xFABD2F),
    (Waiting, 0x83A598),
    (Attention, 0xD3869B),
    (Success, 0xB8BB26),
    (Failure, 0xFB5441),
    (Blocked, 0xFE8019),
    (Shell, 0x8EC07C),
    (Read, 0x83A598),
    (Edit, 0xD3869B),
    (Web, 0x83A598),
    (Call, 0xFE8019),
    (Agent, 0xD3869B),
    (User, 0xFE8019),
    (Code, 0x8EC07C),
    (Link, 0x83A598),
    (Heading, 0xFABD2F),
    (DiffAdd, 0xB8BB26),
    (DiffRemove, 0xFB5441),
]);

const GRUVBOX_LIGHT: Hues = hues([
    (Accent, 0xAF3A03),
    (Running, 0x966211),
    (Waiting, 0x076678),
    (Attention, 0x8F3F71),
    (Success, 0x75700E),
    (Failure, 0x9D0006),
    (Blocked, 0xAF3A03),
    (Shell, 0x417957),
    (Read, 0x076678),
    (Edit, 0x8F3F71),
    (Web, 0x076678),
    (Call, 0xAF3A03),
    (Agent, 0x8F3F71),
    (User, 0xAF3A03),
    (Code, 0x417957),
    (Link, 0x076678),
    (Heading, 0x966211),
    (DiffAdd, 0x75700E),
    (DiffRemove, 0x9D0006),
]);

const TOKYO_NIGHT: Hues = hues([
    (Accent, 0x7AA2F7),
    (Running, 0xE0AF68),
    (Waiting, 0x7DCFFF),
    (Attention, 0xFF9E64),
    (Success, 0x9ECE6A),
    (Failure, 0xF7768E),
    (Blocked, 0xDD5656),
    (Shell, 0x73DACA),
    (Read, 0x7AA2F7),
    (Edit, 0x9D7CD8),
    (Web, 0x2AC3DE),
    (Call, 0x7AA2F7),
    (Agent, 0xBB9AF7),
    (User, 0x7AA2F7),
    (Code, 0xFF9E64),
    (Link, 0x7DCFFF),
    (Heading, 0xBB9AF7),
    (DiffAdd, 0x9ECE6A),
    (DiffRemove, 0xF7768E),
]);

// Tokyo Night Day, deepened for AA on its base.
const TOKYO_NIGHT_DAY: Hues = hues([
    (Accent, 0x2462B6),
    (Running, 0x7D6037),
    (Waiting, 0x006D92),
    (Attention, 0x9B5000),
    (Success, 0x516C35),
    (Failure, 0xBF214F),
    (Blocked, 0xB03C3C),
    (Shell, 0x376E66),
    (Read, 0x2462B6),
    (Edit, 0x7847BD),
    (Web, 0x156E7D),
    (Call, 0x2462B6),
    (Agent, 0x7E46C8),
    (User, 0x2462B6),
    (Code, 0x9B5000),
    (Link, 0x006D92),
    (Heading, 0x7E46C8),
    (DiffAdd, 0x516C35),
    (DiffRemove, 0xBF214F),
]);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::probe::contrast_ratio;
    use crate::ui::theme::tokens::MIN_TEXT_CONTRAST;

    #[test]
    fn every_builtin_hue_reads_at_aa_on_its_own_ground_unadjusted() {
        for builtin in Builtin::ALL {
            for background in [Background::Dark, Background::Light] {
                let ground = builtin.ground(background);
                let hues = builtin.scheme().hues(background);
                for hue in Hue::ALL {
                    let ratio = contrast_ratio(hues.get(hue), ground);
                    assert!(
                        ratio >= MIN_TEXT_CONTRAST,
                        "{} {background:?} {}: {ratio:.2}",
                        builtin.name(),
                        hue.key()
                    );
                }
            }
        }
    }

    #[test]
    fn hue_keys_round_trip_and_are_distinct() {
        for hue in Hue::ALL {
            assert_eq!(Hue::from_key(hue.key()), Some(hue));
            assert_eq!(Hue::ALL[hue as usize], hue, "ALL is in declaration order");
        }
        assert_eq!(Hue::from_key("Accent"), None);
        for builtin in Builtin::ALL {
            assert_eq!(Builtin::from_name(builtin.name()), Some(builtin));
        }
    }

    #[test]
    fn the_status_hues_stay_apart_in_every_builtin() {
        // Running, success and failure carry meaning side by side.
        for builtin in Builtin::ALL {
            let scheme = builtin.scheme();
            for hues in [scheme.dark, scheme.light] {
                let status = [Running, Success, Failure, Attention].map(|h| hues.get(h));
                for (i, a) in status.iter().enumerate() {
                    for b in &status[i + 1..] {
                        assert_ne!(a, b, "{}", builtin.name());
                    }
                }
            }
        }
    }

    #[test]
    fn patches_apply_both_then_per_background() {
        let mut both = HuePatch::default();
        both.set(Hue::Accent, Rgb(1, 2, 3));
        both.set(Hue::Code, Rgb(4, 5, 6));
        let mut dark = HuePatch::default();
        dark.set(Hue::Code, Rgb(7, 8, 9));
        let scheme = Scheme::default().patched(&both, &dark, &HuePatch::default());
        assert_eq!(scheme.dark.get(Hue::Accent), Rgb(1, 2, 3));
        assert_eq!(scheme.light.get(Hue::Accent), Rgb(1, 2, 3));
        assert_eq!(scheme.dark.get(Hue::Code), Rgb(7, 8, 9));
        assert_eq!(scheme.light.get(Hue::Code), Rgb(4, 5, 6));
        assert_eq!(scheme.dark.get(Hue::Running), AETHER_DARK.get(Hue::Running));
        assert!(HuePatch::default().is_empty() && !both.is_empty());
    }

    #[test]
    fn hex_parsing_is_strict() {
        assert_eq!(parse_hex("#A5b4fC"), Some(Rgb(0xA5, 0xB4, 0xFC)));
        for bad in ["A5B4FC", "#A5B4F", "#A5B4FCC", "#12345g", "", "#"] {
            assert_eq!(parse_hex(bad), None, "{bad}");
        }
    }
}
