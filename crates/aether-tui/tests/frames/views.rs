use super::*;

use aether_tui::ui::views::{ViewKind, open};

/// Every slash view, the title it paints at the top of its block, and
/// something from `stocked_app` its body must show.
const VIEWS: [(ViewKind, &str, &str); 12] = [
    (ViewKind::Model, "Model and effort", "gemini"),
    (ViewKind::Usage, "Usage", "No providers configured"),
    (ViewKind::Approvals, "Approvals", "allow bash"),
    (ViewKind::Memory, "Memory", "the sqlite store"),
    (ViewKind::Schedules, "Schedules", "Read docs"),
    (ViewKind::Mcp, "MCP servers", "notes-and-issues"),
    (ViewKind::Doctor, "Doctor", "daemon"),
    (ViewKind::Report, "Report", "reading the log"),
    (ViewKind::Artifacts, "Artifacts", "cargo-test-output"),
    (ViewKind::Settings, "Settings", "Theme"),
    (ViewKind::Voice, "Voice", "Speak questions"),
    (ViewKind::Login, "Sign in", "openai cloud"),
];

fn showing(kind: ViewKind, theme: Theme) -> App {
    let mut app = stocked_app();
    app.theme = theme;
    let view = open(kind, &app);
    app.bottom.push_view(view);
    app
}

/// The row whose text, at the 2-column margin, starts with `title`.
fn title_row(rows: &[String], title: &str) -> Option<usize> {
    rows.iter()
        .position(|r| r.starts_with("  ") && r[2..].starts_with(title))
}

/// SPEC §6.1, §3.4: each view replaces the composer as the approval card's
/// padded, titled, tinted block; overflow is elided and `--ascii` stays ascii.
#[test]
fn every_view_draws_in_the_bottom_pane_at_80_and_120_columns() {
    for (kind, title, content) in VIEWS {
        for ascii in [false, true] {
            for width in [80u16, 120] {
                let app = showing(kind, dark(ascii));
                let theme = &app.theme;
                let buffer = draw(&app, width, 40);
                let rows = rows(&buffer);
                let text = rows.join("\n");
                let label = format!("{kind:?} ascii={ascii} at {width}");
                let top = title_row(&rows, title)
                    .unwrap_or_else(|| panic!("{label}: no {title:?} row:\n{text}"));
                assert!(
                    shows(&buffer[(2, top as u16)], theme.title()),
                    "{label}: the title is not in title()"
                );
                assert!(
                    text.contains(content),
                    "{label}: {content:?} is missing:\n{text}"
                );
                assert!(
                    !text.contains("Steer assistant") && !text.contains("(esc)"),
                    "{label}: the view shares the pane:\n{text}"
                );
                let status_line = rows.len() - 2;
                assert!(
                    rows[status_line].contains("claude-opus-5"),
                    "{label}: the status line is gone:\n{text}"
                );
                let tint = theme.tint().bg.expect("a probed dark theme has a tint");
                for y in top - 1..status_line {
                    assert!(
                        (0..width).all(|x| buffer[(x, y as u16)].bg == tint),
                        "{label}: row {y} is outside the tinted block:\n{text}"
                    );
                }
                assert!(rows[top - 1].trim().is_empty(), "{label}: no top padding");
                assert!(
                    rows[status_line - 1].trim().is_empty(),
                    "{label}: the block runs into the status line:\n{text}"
                );
                assert!(
                    rows[status_line - 2].contains("esc"),
                    "{label}: the block's last row is not its keys:\n{text}"
                );
                // A row reaching the edge either ends in the ellipsis or fits exactly,
                // in which case a wide frame draws the very same row.
                let ellipsis = theme.glyphs().ellipsis;
                let wide = screen(&app, 220, 40);
                for y in top..status_line {
                    let edge = buffer[(width - 1, y as u16)].symbol();
                    let row = rows[y].trim_end();
                    assert!(
                        edge == " "
                            || ellipsis.ends_with(edge)
                            || wide.iter().any(|r| r.trim_end() == row),
                        "{label}: row {y} is cut at the edge: {row:?}"
                    );
                }
                // The hint row's ascii is `every_open_view_reads_in_ascii`'s.
                let above = rows[..rows.len() - 1].join("\n");
                if ascii {
                    assert!(
                        above.is_ascii(),
                        "{:?} {label}:\n{above}",
                        non_ascii(&above)
                    );
                }
            }
        }
    }
}

/// `--ascii` leaves no unicode anywhere an open view draws, its hint row
/// included, at every size.
#[test]
fn every_open_view_reads_in_ascii() {
    for (kind, ..) in VIEWS {
        for (w, h) in SIZES {
            let text = screen(&showing(kind, dark(true)), w, h).join("\n");
            assert!(
                text.is_ascii(),
                "{:?} {kind:?} at {w}x{h}:\n{text}",
                non_ascii(&text)
            );
        }
    }
    for (w, h) in SIZES {
        let text = screen(&paging(dark(true)), w, h).join("\n");
        assert!(
            text.is_ascii(),
            "{:?} pager at {w}x{h}:\n{text}",
            non_ascii(&text)
        );
    }
}

/// An open view owns the keys, so it says its keys inside its own block,
/// and the hint row stops offering the composer's.
#[test]
fn an_open_view_carries_its_own_keys() {
    for (kind, ..) in VIEWS {
        let app = showing(kind, dark(false));
        let rows = screen(&app, 120, 40);
        let hint = rows.last().expect("a hint row");
        assert!(!hint.contains("? shortcuts"), "{kind:?}: {hint:?}");
        // Above the status line: the block's padding, then its keys.
        let keys = &rows[rows.len() - 4];
        assert!(
            keys.starts_with("  ") && keys.contains("esc"),
            "{kind:?} does not say how to leave: {keys:?}"
        );
    }
}

/// Typing the command is how a person gets there: `/model` and Enter opens
/// the view, and Esc gives the composer back with nothing typed in it.
#[test]
fn a_slash_command_opens_its_view_and_esc_closes_it() {
    for (command, title) in [("/model", "Model and effort"), ("/settings", "Settings")] {
        let mut app = quiet(app_with_transcript(&["hello", "hello back"]));
        app.reduce(AppEvent::Paste(command.into()));
        app.reduce(key(KeyCode::Enter));
        let rows = screen(&app, 120, 40);
        assert!(
            title_row(&rows, title).is_some(),
            "{command} did not open {title}:\n{}",
            rows.join("\n")
        );
        app.reduce(key(KeyCode::Esc));
        let text = screen(&app, 120, 40).join("\n");
        assert!(
            !text.contains(title),
            "{command}: Esc left the view open:\n{text}"
        );
        assert!(
            text.contains("Steer assistant"),
            "{command}: no composer:\n{text}"
        );
    }
}

/// `stocked_app` reading `build.log` in the artifact pager.
fn paging(theme: Theme) -> App {
    use aether_tui::app::OpenArtifact;
    let mut app = stocked_app();
    app.theme = theme;
    let artifact = artifact("build.log");
    let id = artifact.id;
    app.artifacts.open = Some(OpenArtifact {
        artifact,
        text: "compiling aether\nfinished".into(),
        truncated: false,
    });
    let view = open(ViewKind::ArtifactPager(id), &app);
    app.bottom.push_view(view);
    app
}

/// The pager is the one full-screen view: it takes everything but the hint
/// row, at any width, framed like every other view.
#[test]
fn the_artifact_pager_takes_the_whole_frame() {
    for ascii in [false, true] {
        for width in [80u16, 120] {
            let rows = screen(&paging(dark(ascii)), width, 24);
            let text = rows.join("\n");
            assert!(rows[0].trim().is_empty(), "{text}");
            assert_eq!(
                title_row(&rows, "build.log"),
                Some(1),
                "ascii={ascii} at {width}:\n{text}"
            );
            assert!(rows[1].contains("84 KB"), "{text}");
            assert_eq!(rows[3].trim_end(), "  compiling aether", "{text}");
            assert!(rows[rows.len() - 3].contains("esc back"), "{text}");
            assert!(
                !text.contains("claude-opus-5"),
                "the status line shows through:\n{text}"
            );
            assert!(
                !rows.last().expect("a hint row").trim().is_empty(),
                "{text}"
            );
        }
    }
}
