//! Theme files: `<config dir>/themes/<name>.toml`, written by hand.
//!
//! ```toml
//! base = "nord"            # optional built-in to start from; default aether
//! running = "#FFB86C"      # any hue, on dark and light terminals alike
//! [light]
//! running = "#B45309"      # this background only
//! ```

use std::io::Read;
use std::path::{Path, PathBuf};

use super::choice::{Palette, ThemeChoice, is_file_stem};
use super::scheme::{Builtin, Hue, HuePatch, Scheme, parse_hex};

pub const DIR: &str = "themes";

/// Far above any real theme; bounds what a stray file can make `/theme` read.
const MAX_BYTES: u64 = 64 * 1024;

/// Why a theme could not be used as written. `fallback` is the base to paint
/// instead; `None` when there is nothing of the theme to fall back to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeError {
    pub message: String,
    pub fallback: Option<Builtin>,
}

pub fn dir(config_dir: &Path) -> PathBuf {
    config_dir.join(DIR)
}

/// The scheme `choice` paints with. A theme file is read now, so an edit
/// shows the next time it is chosen.
pub fn scheme(choice: &ThemeChoice, config_dir: Option<&Path>) -> Result<Scheme, ThemeError> {
    let name = match &choice.palette {
        Palette::Builtin(builtin) => return Ok(builtin.scheme()),
        Palette::User(name) => name,
    };
    let Some(config_dir) = config_dir else {
        return Err(ThemeError {
            message: format!("theme {name}: no config directory this run"),
            fallback: None,
        });
    };
    let path = dir(config_dir).join(format!("{name}.toml"));
    let text = read(&path).map_err(|message| ThemeError {
        message,
        fallback: None,
    })?;
    parse(&text).map_err(|(problem, base)| ThemeError {
        message: format!(
            "theme {}: {problem}; using {} instead",
            path.display(),
            base.name()
        ),
        fallback: Some(base),
    })
}

/// A theme file's text. Only a small regular file is opened, so a FIFO or a
/// device named `*.toml` can neither block the UI nor be read without end.
fn read(path: &Path) -> Result<String, String> {
    let name = || path.file_stem().unwrap_or_default().to_string_lossy();
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(format!(
                "no theme named {}: {} does not exist",
                name(),
                path.display()
            ));
        }
        Err(e) => return Err(format!("theme {}: {e}", path.display())),
    };
    if !metadata.is_file() {
        return Err(format!("theme {}: not a regular file", path.display()));
    }
    if metadata.len() > MAX_BYTES {
        return Err(format!(
            "theme {}: larger than {} KiB",
            path.display(),
            MAX_BYTES / 1024
        ));
    }
    let mut text = String::new();
    std::fs::File::open(path)
        .and_then(|file| file.take(MAX_BYTES).read_to_string(&mut text))
        .map_err(|e| format!("theme {}: {e}", path.display()))?;
    Ok(text)
}

/// Theme file names in `config_dir/themes`, sorted; a missing folder is none.
pub fn list(config_dir: &Path) -> Result<Vec<String>, String> {
    let folder = dir(config_dir);
    let entries = match std::fs::read_dir(&folder) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("{}: {e}", folder.display())),
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("{}: {e}", folder.display()))?;
        let file_name = entry.file_name();
        let Some(stem) = file_name
            .to_str()
            .and_then(|name| name.strip_suffix(".toml"))
        else {
            continue;
        };
        // Only names `/theme <name>` can reach; anything else is not a theme.
        if is_file_stem(stem) {
            names.push(stem.to_owned());
        }
    }
    names.sort();
    Ok(names)
}

/// The scheme a file describes, or the problem and the base to fall back to.
fn parse(text: &str) -> Result<Scheme, (String, Builtin)> {
    let table: toml::Table = text
        .parse()
        .map_err(|e: toml::de::Error| (e.message().to_owned(), Builtin::Aether))?;
    let base = match table.get("base") {
        None => Builtin::Aether,
        Some(toml::Value::String(name)) => Builtin::from_name(name).ok_or_else(|| {
            let known: Vec<_> = Builtin::ALL.iter().map(|b| b.name()).collect();
            (
                format!("base {name:?} is not one of {}", known.join(", ")),
                Builtin::Aether,
            )
        })?,
        Some(other) => {
            return Err((
                format!("base must be a theme name, got {}", other.type_str()),
                Builtin::Aether,
            ));
        }
    };
    let mut both = HuePatch::default();
    let mut dark = HuePatch::default();
    let mut light = HuePatch::default();
    for (key, value) in &table {
        let result = match key.as_str() {
            "base" => Ok(()),
            "dark" => patch_table("dark.", value, &mut dark),
            "light" => patch_table("light.", value, &mut light),
            _ => set_hue("", key, value, &mut both),
        };
        result.map_err(|problem| (problem, base))?;
    }
    Ok(base.scheme().patched(&both, &dark, &light))
}

fn patch_table(prefix: &str, value: &toml::Value, patch: &mut HuePatch) -> Result<(), String> {
    let toml::Value::Table(table) = value else {
        return Err(format!(
            "{} must be a table of colours",
            prefix.trim_end_matches('.')
        ));
    };
    for (key, value) in table {
        set_hue(prefix, key, value, patch)?;
    }
    Ok(())
}

fn set_hue(
    prefix: &str,
    key: &str,
    value: &toml::Value,
    patch: &mut HuePatch,
) -> Result<(), String> {
    let hue = Hue::from_key(key).ok_or_else(|| {
        format!("unknown key {prefix}{key} (colours are listed in docs/tui.md#themes)")
    })?;
    let rgb = value
        .as_str()
        .and_then(parse_hex)
        .ok_or_else(|| format!("{prefix}{key} must be \"#RRGGBB\", got {value}"))?;
    patch.set(hue, rgb);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::tokens::{Background, Rgb, ThemeMode};

    fn user(name: &str) -> ThemeChoice {
        ThemeChoice {
            palette: Palette::User(name.into()),
            mode: ThemeMode::Auto,
        }
    }

    fn write(config: &Path, name: &str, text: &str) {
        std::fs::create_dir_all(dir(config)).unwrap();
        std::fs::write(dir(config).join(format!("{name}.toml")), text).unwrap();
    }

    #[test]
    fn a_theme_file_patches_its_base_on_both_backgrounds_then_each() {
        let config = tempfile::tempdir().unwrap();
        write(
            config.path(),
            "ember",
            "base = \"gruvbox\"\nrunning = \"#FF8800\"\n[light]\nrunning = \"#884400\"\n",
        );
        let scheme = scheme(&user("ember"), Some(config.path())).unwrap();
        let gruvbox = Builtin::Gruvbox.scheme();
        assert_eq!(scheme.dark.get(Hue::Running), Rgb(0xFF, 0x88, 0x00));
        assert_eq!(scheme.light.get(Hue::Running), Rgb(0x88, 0x44, 0x00));
        assert_eq!(
            scheme.hues(Background::Dark).get(Hue::Accent),
            gruvbox.dark.get(Hue::Accent)
        );
    }

    #[test]
    fn a_broken_file_names_the_file_and_key_and_falls_back_to_its_base() {
        let config = tempfile::tempdir().unwrap();
        for (text, key, base) in [
            (
                "base = \"nord\"\nrunnign = \"#FF8800\"\n",
                "runnign",
                Builtin::Nord,
            ),
            (
                "base = \"nord\"\n[dark]\nrunning = \"orange\"\n",
                "dark.running",
                Builtin::Nord,
            ),
            ("[light]\nshell = 3\n", "light.shell", Builtin::Aether),
            ("base = \"solarized\"\n", "solarized", Builtin::Aether),
            ("running = \n", "", Builtin::Aether),
        ] {
            write(config.path(), "broken", text);
            let error = scheme(&user("broken"), Some(config.path())).unwrap_err();
            assert!(error.message.contains("broken.toml"), "{}", error.message);
            assert!(error.message.contains(key), "{key}: {}", error.message);
            assert_eq!(error.fallback, Some(base), "{text}");
        }
    }

    #[test]
    fn a_missing_theme_has_nothing_to_fall_back_to() {
        let config = tempfile::tempdir().unwrap();
        let error = scheme(&user("nope"), Some(config.path())).unwrap_err();
        assert!(
            error.message.contains("no theme named nope"),
            "{}",
            error.message
        );
        assert_eq!(error.fallback, None);
        assert!(scheme(&user("nope"), None).unwrap_err().fallback.is_none());
        let nord = ThemeChoice::builtin(Builtin::Nord, ThemeMode::Light);
        assert_eq!(scheme(&nord, None), Ok(Builtin::Nord.scheme()));
    }

    #[test]
    fn listing_keeps_only_reachable_theme_names() {
        let config = tempfile::tempdir().unwrap();
        assert_eq!(list(config.path()), Ok(Vec::new()));
        for name in ["zen", "ember", "Bad Name", ".hidden"] {
            write(config.path(), name, "");
        }
        std::fs::write(dir(config.path()).join("notes.txt"), "").unwrap();
        assert_eq!(list(config.path()).unwrap(), ["ember", "zen"]);
    }

    #[test]
    fn only_a_small_regular_file_is_read() {
        let config = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir(config.path()).join("folder.toml")).unwrap();
        write(config.path(), "huge", &"#".repeat(MAX_BYTES as usize + 1));
        for (name, problem) in [("folder", "not a regular file"), ("huge", "larger than")] {
            let error = scheme(&user(name), Some(config.path())).unwrap_err();
            assert!(error.message.contains(problem), "{}", error.message);
            assert_eq!(error.fallback, None);
        }
    }
}
