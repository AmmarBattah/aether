//! Hand-owned theme customizations, separate from saved UI preferences.
//! `[theme] <hue> = "#RRGGBB"` sets one hue over any theme; the older
//! `[theme.tokens.<token>]` table still loads, its colours mapped onto hues.
use serde::Deserialize;
use std::{collections::BTreeMap, path::Path};

use super::scheme::{Hue, HuePatch, parse_hex};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Overrides {
    /// Hues set by `[theme]` keys or by mapped `[theme.tokens.*] fg` entries.
    pub hues: HuePatch,
    /// Overrides the file sets that the theme does not apply.
    pub ignored: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Token {
    Ground,
    Accent,
    Text,
    Secondary,
    Dim,
    Border,
    BorderFocused,
    Running,
    Waiting,
    Blocked,
    Completed,
    Replayed,
    Failed,
    Cancelled,
    Approval,
    Person,
    Thinking,
    InlineCode,
    CodeBlock,
    Heading,
    Link,
    DiffAdd,
    DiffRemove,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Patch {
    fg: Option<String>,
    bg: Option<String>,
    modifiers: Option<Vec<String>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Table {
    tokens: BTreeMap<Token, Patch>,
    /// `accent = "#…"` and every other hue key.
    #[serde(flatten)]
    hues: BTreeMap<String, toml::Value>,
}

impl Token {
    /// The hue an old token's `fg` now sets; `None` for tokens the theme
    /// derives from the terminal (text, ground, borders, …).
    fn hue(self) -> Option<Hue> {
        Some(match self {
            Token::Accent => Hue::Accent,
            Token::Running => Hue::Running,
            Token::Waiting => Hue::Waiting,
            Token::Blocked => Hue::Blocked,
            Token::Completed => Hue::Success,
            Token::Failed => Hue::Failure,
            Token::Approval => Hue::Attention,
            Token::Person => Hue::User,
            Token::InlineCode => Hue::Code,
            Token::Heading => Hue::Heading,
            Token::Link => Hue::Link,
            Token::DiffAdd => Hue::DiffAdd,
            Token::DiffRemove => Hue::DiffRemove,
            Token::Ground
            | Token::Text
            | Token::Secondary
            | Token::Dim
            | Token::Border
            | Token::BorderFocused
            | Token::Replayed
            | Token::Cancelled
            | Token::Thinking
            | Token::CodeBlock => return None,
        })
    }
}

impl Overrides {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        use anyhow::Context;
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
        };
        #[derive(Deserialize)]
        struct Root {
            #[serde(default)]
            theme: Table,
        }
        let root: Root = toml::from_str(&text)
            .with_context(|| format!("theme configuration in {}", path.display()))?;
        Self::from_table(root.theme)
            .with_context(|| format!("theme configuration in {}", path.display()))
    }

    fn from_table(table: Table) -> anyhow::Result<Self> {
        let mut overrides = Self::default();
        for (&token, patch) in &table.tokens {
            let name = token_name(token);
            let Some(hue) = token.hue() else {
                overrides.ignored.push(name);
                continue;
            };
            // Only the accent was ever applied, so only its value may stop startup.
            match patch.fg.as_deref().map(|fg| (fg, parse_hex(fg))) {
                None => {}
                Some((_, Some(rgb))) => overrides.hues.set(hue, rgb),
                Some((fg, None)) if token == Token::Accent => {
                    anyhow::bail!("accent fg must be #RRGGBB, got {fg:?}")
                }
                Some((fg, None)) => overrides.ignored.push(format!("{name} fg {fg:?}")),
            }
            if patch.bg.is_some() || patch.modifiers.is_some() {
                overrides.ignored.push(format!("{name} bg/modifiers"));
            }
        }
        // The documented form wins over the retired token table.
        for (key, value) in &table.hues {
            let hue =
                Hue::from_key(key).ok_or_else(|| anyhow::anyhow!("unknown key theme.{key}"))?;
            let rgb = value
                .as_str()
                .and_then(parse_hex)
                .ok_or_else(|| anyhow::anyhow!("{key} must be #RRGGBB, got {value}"))?;
            overrides.hues.set(hue, rgb);
        }
        Ok(overrides)
    }

    /// One line for the flash row when part of the file was not applied.
    pub fn ignored_notice(&self) -> Option<String> {
        (!self.ignored.is_empty()).then(|| {
            format!(
                "theme overrides not applied: {} (only colours are)",
                self.ignored.join(", ")
            )
        })
    }
}

fn token_name(token: Token) -> String {
    let debug = format!("{token:?}");
    let mut name = String::new();
    for (i, c) in debug.chars().enumerate() {
        if c.is_ascii_uppercase() && i > 0 {
            name.push('_');
        }
        name.push(c.to_ascii_lowercase());
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::tokens::Rgb;

    #[test]
    fn overrides_validate_input_and_preserve_hand_owned_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        assert_eq!(Overrides::load(&path).unwrap(), Overrides::default());
        for invalid in [
            "[theme.tokens.unknown]\nfg='12'",
            "[theme.tokens.accent]\nfg='12'",
            "[theme.tokens.accent]\nfg='#12345g'",
            "[theme.tokens.accent]\nfg=12",
            "[theme.tokens.accent]\nforeground='12'",
            "[theme]\nrunnign='#ff8800'",
        ] {
            std::fs::write(&path, invalid).unwrap();
            let error = Overrides::load(&path).unwrap_err();
            assert!(format!("{error:#}").contains("config.toml"), "{invalid}");
            assert_eq!(std::fs::read_to_string(&path).unwrap(), invalid);
        }
        let text = "# retain my comments\n[model]\ndefault='codex/gpt-5.6-terra'\n[theme.tokens.accent]\nfg='#ff8800'\n[theme.tokens.code_block]\nfg='default'\n";
        std::fs::write(&path, text).unwrap();
        let overrides = Overrides::load(&path).unwrap();
        assert_eq!(overrides.hues.get(Hue::Accent), Some(Rgb(0xff, 0x88, 0x00)));
        assert_eq!(overrides.ignored, ["code_block"]);
        assert!(overrides.ignored_notice().unwrap().contains("code_block"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
    }

    #[test]
    fn retired_tokens_set_the_hues_they_meant() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let text = "[theme.tokens.running]\nfg='#ffaa00'\n[theme.tokens.failed]\nfg='#ff0000'\nmodifiers=['bold']\n[theme.tokens.approval]\nfg='#ff00ff'\n[theme.tokens.person]\nfg='yellow'\n";
        std::fs::write(&path, text).unwrap();
        let overrides = Overrides::load(&path).unwrap();
        assert_eq!(
            overrides.hues.get(Hue::Running),
            Some(Rgb(0xff, 0xaa, 0x00))
        );
        assert_eq!(
            overrides.hues.get(Hue::Failure),
            Some(Rgb(0xff, 0x00, 0x00))
        );
        assert_eq!(
            overrides.hues.get(Hue::Attention),
            Some(Rgb(0xff, 0x00, 0xff))
        );
        // A value the old table tolerated is reported, not fatal.
        assert_eq!(overrides.hues.get(Hue::User), None);
        assert_eq!(
            overrides.ignored,
            ["failed bg/modifiers", "person fg \"yellow\""]
        );
    }

    #[test]
    fn theme_hue_keys_win_over_the_token_table() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "[theme]\naccent = '#00bfff'\nshell = '#123456'\n[theme.tokens.accent]\nfg='#ff8800'\n",
        )
        .unwrap();
        let overrides = Overrides::load(&path).unwrap();
        assert_eq!(overrides.hues.get(Hue::Accent), Some(Rgb(0x00, 0xbf, 0xff)));
        assert_eq!(overrides.hues.get(Hue::Shell), Some(Rgb(0x12, 0x34, 0x56)));
        assert!(overrides.ignored.is_empty());
        std::fs::write(&path, "[theme]\naccent = 'orange'\n").unwrap();
        let error = format!("{:#}", Overrides::load(&path).unwrap_err());
        assert!(
            error.contains("config.toml") && error.contains("#RRGGBB"),
            "{error}"
        );
    }
}
