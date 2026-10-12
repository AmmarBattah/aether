//! What `theme = "…"`, `/theme <name>` and `--theme` name.
//!
//! `auto`, `dark`, `light` and `mono` keep their old meaning (Aether's own
//! colours); a built-in name follows the terminal's background unless pinned
//! with `-dark`/`-light`; any other plain name is a file in the themes folder.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::scheme::Builtin;
use super::tokens::ThemeMode;

/// Longest name a theme file may have; also bounds what `/theme` echoes back.
const MAX_NAME: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Palette {
    Builtin(Builtin),
    /// `<config dir>/themes/<name>.toml`; the name is a safe file stem.
    User(String),
}

/// A palette and how it picks its background. Mono ignores the palette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeChoice {
    pub palette: Palette,
    pub mode: ThemeMode,
}

impl Default for ThemeChoice {
    fn default() -> Self {
        ThemeChoice::builtin(Builtin::Aether, ThemeMode::Auto)
    }
}

impl ThemeChoice {
    pub fn builtin(builtin: Builtin, mode: ThemeMode) -> ThemeChoice {
        ThemeChoice {
            palette: Palette::Builtin(builtin),
            mode,
        }
    }

    pub fn mono() -> ThemeChoice {
        ThemeChoice::builtin(Builtin::Aether, ThemeMode::Mono)
    }

    /// Every built-in choice, in the order `/theme` lists them.
    pub fn builtins() -> Vec<ThemeChoice> {
        let mut all = vec![
            ThemeChoice::default(),
            ThemeChoice::builtin(Builtin::Aether, ThemeMode::Dark),
            ThemeChoice::builtin(Builtin::Aether, ThemeMode::Light),
        ];
        all.extend(
            Builtin::ALL
                .into_iter()
                .filter(|b| *b != Builtin::Aether)
                .map(|b| ThemeChoice::builtin(b, ThemeMode::Auto)),
        );
        all.push(ThemeChoice::mono());
        all
    }
}

impl fmt::Display for ThemeChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.palette, self.mode) {
            (_, ThemeMode::Mono) => f.write_str("mono"),
            (Palette::Builtin(Builtin::Aether), mode) => f.write_str(mode.as_str()),
            (Palette::Builtin(builtin), ThemeMode::Auto) => f.write_str(builtin.name()),
            (Palette::Builtin(builtin), mode) => write!(f, "{}-{mode}", builtin.name()),
            (Palette::User(name), _) => f.write_str(name),
        }
    }
}

impl FromStr for ThemeChoice {
    type Err = String;

    fn from_str(text: &str) -> Result<ThemeChoice, String> {
        let name = text.trim().to_ascii_lowercase();
        if let Ok(mode) = name.parse::<ThemeMode>() {
            return Ok(ThemeChoice::builtin(Builtin::Aether, mode));
        }
        let pinned = match name.as_str() {
            "catppuccin-mocha" => Some((Builtin::Catppuccin, ThemeMode::Dark)),
            "catppuccin-latte" => Some((Builtin::Catppuccin, ThemeMode::Light)),
            "tokyo-night-day" => Some((Builtin::TokyoNight, ThemeMode::Light)),
            _ => None,
        };
        if let Some((builtin, mode)) = pinned {
            return Ok(ThemeChoice::builtin(builtin, mode));
        }
        if let Some(builtin) = Builtin::from_name(&name) {
            return Ok(ThemeChoice::builtin(builtin, ThemeMode::Auto));
        }
        for (suffix, mode) in [("-dark", ThemeMode::Dark), ("-light", ThemeMode::Light)] {
            if let Some(builtin) = name.strip_suffix(suffix).and_then(Builtin::from_name) {
                return Ok(ThemeChoice::builtin(builtin, mode));
            }
        }
        if is_file_stem(&name) {
            return Ok(ThemeChoice {
                palette: Palette::User(name),
                mode: ThemeMode::Auto,
            });
        }
        Err(format!(
            "{:?} is not a theme name: use letters, digits, - and _",
            text.chars().take(MAX_NAME).collect::<String>()
        ))
    }
}

/// Stored as its name, so `ui.toml` reads `theme = "nord"`.
impl Serialize for ThemeChoice {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for ThemeChoice {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<ThemeChoice, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

/// Lower-case letters, digits, `-` and `_`: never a path, never hidden.
pub fn is_file_stem(name: &str) -> bool {
    (1..=MAX_NAME).contains(&name.len())
        && !name.starts_with(['-', '_'])
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> ThemeChoice {
        text.parse().unwrap_or_else(|e| panic!("{text}: {e}"))
    }

    #[test]
    fn the_old_names_keep_their_meaning() {
        for (text, mode) in [
            ("auto", ThemeMode::Auto),
            ("dark", ThemeMode::Dark),
            ("dusk", ThemeMode::Dark),
            ("light", ThemeMode::Light),
            (" Dawn ", ThemeMode::Light),
            ("mono", ThemeMode::Mono),
            ("monochrome", ThemeMode::Mono),
        ] {
            assert_eq!(parse(text), ThemeChoice::builtin(Builtin::Aether, mode));
        }
        assert_eq!(parse("aether"), ThemeChoice::default());
        assert_eq!(parse("aether-light").to_string(), "light");
    }

    #[test]
    fn builtins_follow_the_terminal_unless_pinned() {
        assert_eq!(
            parse("nord"),
            ThemeChoice::builtin(Builtin::Nord, ThemeMode::Auto)
        );
        assert_eq!(
            parse("gruvbox-light"),
            ThemeChoice::builtin(Builtin::Gruvbox, ThemeMode::Light)
        );
        assert_eq!(
            parse("catppuccin-latte"),
            ThemeChoice::builtin(Builtin::Catppuccin, ThemeMode::Light)
        );
        assert_eq!(parse("catppuccin-mocha").to_string(), "catppuccin-dark");
        assert_eq!(parse("tokyo-night-day").to_string(), "tokyo-night-light");
    }

    #[test]
    fn every_listed_choice_round_trips_through_its_name() {
        let listed = ThemeChoice::builtins();
        assert_eq!(listed.len(), 8);
        for choice in listed {
            assert_eq!(parse(&choice.to_string()), choice);
        }
        let user = parse("solar_flare-2");
        assert_eq!(user.palette, Palette::User("solar_flare-2".into()));
        assert_eq!(user.to_string(), "solar_flare-2");
    }

    #[test]
    fn a_name_can_never_reach_outside_the_themes_folder() {
        for bad in [
            "../x",
            "a/b",
            ".hidden",
            "-x",
            "",
            "a b",
            "x.toml",
            "é",
            &"a".repeat(65),
        ] {
            let error = bad.parse::<ThemeChoice>().unwrap_err();
            assert!(error.contains("not a theme name"), "{bad}: {error}");
        }
    }
}
