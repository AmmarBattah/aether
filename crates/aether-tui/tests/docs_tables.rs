#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

//! docs/tui.md's key and command tables are rendered from the registry, so
//! the guide cannot drift from the `?` panel and the `/` popup. Regenerate with
//! `AETHER_BLESS_DOCS=1 cargo test -p aether-tui --test docs_tables`.

use std::path::PathBuf;

use aether_tui::commands::{COMMANDS, KEYS, KeyScope, help_commands};

/// The help's grouping, in the help's order.
const SCOPES: [KeyScope; 4] = [
    KeyScope::Global,
    KeyScope::Input,
    KeyScope::Panel,
    KeyScope::Overlay,
];

fn guide_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/tui.md")
}

/// GitHub tables split on `|` even inside a code span.
fn cell(text: &str) -> String {
    text.replace('|', "\\|")
}

fn code(text: &str) -> String {
    format!("`{}`", cell(text))
}

fn sentence_case(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

fn keys_table() -> String {
    let mut out = String::new();
    for (i, scope) in SCOPES.into_iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&format!("**{}**\n\n", sentence_case(scope.title())));
        out.push_str("| Keys | Action |\n| --- | --- |\n");
        for spec in KEYS.iter().filter(|k| k.scope == scope) {
            out.push_str(&format!(
                "| {} | {} |\n",
                code(spec.display_keys(false)),
                cell(spec.display_action(false))
            ));
        }
    }
    out
}

/// Listed commands only: hidden aliases still parse but the popup never shows them.
fn commands_table() -> String {
    let mut out = String::from("| Command | What it does | Key |\n| --- | --- | --- |\n");
    for ((name, about), spec) in help_commands().zip(COMMANDS) {
        let key = if spec.key.is_empty() {
            String::new()
        } else {
            code(spec.key)
        };
        out.push_str(&format!("| {} | {} | {key} |\n", code(&name), cell(about)));
    }
    out
}

/// Byte range of the text strictly between the `name` markers' lines.
fn section(doc: &str, name: &str) -> std::ops::Range<usize> {
    let begin = format!("<!-- BEGIN GENERATED: {name} -->\n");
    let end = format!("<!-- END GENERATED: {name} -->");
    assert_eq!(
        doc.matches(&begin).count(),
        1,
        "docs/tui.md needs one {begin:?}"
    );
    assert_eq!(
        doc.matches(&end).count(),
        1,
        "docs/tui.md needs one {end:?}"
    );
    let start = doc.find(&begin).unwrap() + begin.len();
    let stop = doc.find(&end).unwrap();
    assert!(start <= stop, "{name}: the END marker comes before BEGIN");
    start..stop
}

#[test]
fn the_guide_tables_match_the_registry() {
    let path = guide_path();
    let mut doc = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    let bless = std::env::var_os("AETHER_BLESS_DOCS").is_some_and(|v| v == "1");
    let mut stale = Vec::new();
    for (name, table) in [("keys", keys_table()), ("commands", commands_table())] {
        let range = section(&doc, name);
        if doc[range.clone()] == table {
            continue;
        }
        if bless {
            doc.replace_range(range, &table);
        } else {
            stale.push(name);
        }
    }
    if bless {
        std::fs::write(&path, &doc).unwrap_or_else(|e| panic!("writing {}: {e}", path.display()));
    }
    assert!(
        stale.is_empty(),
        "docs/tui.md is out of date for {stale:?}; regenerate with \
         AETHER_BLESS_DOCS=1 cargo test -p aether-tui --test docs_tables"
    );
}

/// The generator's own invariants, so a blessed table is a complete one.
#[test]
fn the_tables_list_every_key_and_every_listed_command_once() {
    let key_rows = keys_table()
        .lines()
        .filter(|l| l.starts_with("| `"))
        .count();
    assert_eq!(
        key_rows,
        KEYS.len(),
        "a key row went missing or was grouped twice"
    );
    let listed: Vec<String> = commands_table()
        .lines()
        .filter_map(|l| l.strip_prefix("| `/"))
        .map(|rest| rest.split([' ', '`']).next().unwrap_or_default().to_owned())
        .collect();
    let names: Vec<&str> = COMMANDS.iter().map(|c| c.name).collect();
    assert_eq!(
        listed, names,
        "every listed command once, in rank order, no aliases"
    );
}
