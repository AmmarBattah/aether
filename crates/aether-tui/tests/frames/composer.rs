use super::*;

use aether_tui::ui::layout::COMPOSER_MAX_INPUT_ROWS;
use ratatui::layout::Position;

/// One frame, with where the terminal cursor was left.
fn draw_cursor(app: &App, w: u16, h: u16) -> (Buffer, Position) {
    let mut terminal = Terminal::new(TestBackend::new(w, h)).expect("terminal");
    terminal
        .draw(|frame| ui::render(frame, app, &mut ViewCache::default()))
        .expect("draw");
    let cursor = terminal.get_cursor_position().expect("cursor");
    (terminal.backend().buffer().clone(), cursor)
}

/// Rows of the composer's input, from its prompt row to the row above its
/// bottom padding (the status line sits under that).
fn input_rows(rows: &[String], prompt: &str) -> std::ops::Range<usize> {
    let first = rows
        .iter()
        .position(|r| r.starts_with(prompt))
        .unwrap_or_else(|| panic!("no composer prompt in\n{}", rows.join("\n")));
    first..rows.len() - 3
}

/// SPEC §3.2: idle reads `Message <agent>`, a running agent `Steer <agent>`;
/// the prompt is bold while the composer has focus, on a tinted block.
#[test]
fn the_composer_says_who_the_message_goes_to() {
    for ascii in [false, true] {
        for width in [80u16, 120] {
            let mut app = quiet(app_with_transcript(&["hello", "hello back"]));
            app.theme = dark(ascii);
            let theme = app.theme;
            let prompt = theme.glyphs().prompt;
            let buffer = draw(&app, width, 24);
            let rows = rows(&buffer);
            let y = row_with(&rows, &format!("{prompt} Steer assistant"))
                .unwrap_or_else(|| panic!("a running agent is steered:\n{}", rows.join("\n")));
            assert!(rows[y].starts_with(prompt), "ascii={ascii}: {:?}", rows[y]);
            let row = y as u16;
            assert!(
                shows(&buffer[(0, row)], theme.title()),
                "the focused prompt is not bold"
            );
            assert!(
                shows(&buffer[(2, row)], theme.secondary()),
                "the placeholder is not dim"
            );
            let tint = theme.tint().bg.expect("a dark palette tints the composer");
            for padding in [row - 1, row + 1] {
                assert_eq!(
                    buffer[(width - 1, padding)].bg,
                    tint,
                    "row {padding} is not tinted"
                );
            }

            set_status(&mut app, AgentStatus::Ready);
            let rows = screen(&app, width, 24);
            assert!(
                row_with(&rows, &format!("{prompt} Message assistant")).is_some(),
                "an idle agent is messaged:\n{}",
                rows.join("\n")
            );
        }
    }
}

/// The draft wraps instead of running off the edge, the block grows to
/// eight input rows, then scrolls to keep the cursor in view - and wrapping
/// never edits what will be sent.
#[test]
fn the_composer_wraps_grows_and_scrolls_to_the_cursor() {
    let (w, h) = (30u16, 30u16);
    let mut app = quiet(app_with_transcript(&[]));
    set_status(&mut app, AgentStatus::Ready);
    app.size = (w, h);
    let prompt = app.theme.glyphs().prompt;

    app.input.set_text("a".repeat(40));
    let (buffer, cursor) = draw_cursor(&app, w, h);
    let screen = rows(&buffer);
    let input = input_rows(&screen, prompt);
    assert_eq!(
        input.len(),
        2,
        "forty columns wrap once at {w}:\n{}",
        screen.join("\n")
    );
    assert_eq!(
        usize::from(cursor.y),
        input.end - 1,
        "the cursor follows the text"
    );
    assert!(
        screen[input.clone()]
            .iter()
            .all(|r| r.trim_end().chars().count() < usize::from(w)),
        "the draft runs into the edge:\n{}",
        screen.join("\n")
    );

    let text = format!("{}END", "a".repeat(400));
    app.input.set_text(text.clone());
    let (buffer, cursor) = draw_cursor(&app, w, h);
    let screen = rows(&buffer);
    // The prompt row has scrolled away: the block is the rows over its padding.
    let end = screen.len() - 3;
    let input = end - usize::from(COMPOSER_MAX_INPUT_ROWS)..end;
    let blank = |y: usize| screen[y].trim().is_empty();
    assert!(
        !input.clone().any(blank) && blank(input.start - 1),
        "the block stops growing at {COMPOSER_MAX_INPUT_ROWS} rows:\n{}",
        screen.join("\n")
    );
    assert_eq!(
        usize::from(cursor.y),
        input.end - 1,
        "the cursor row stays in view"
    );
    assert_eq!(
        buffer[(cursor.x - 1, cursor.y)].symbol(),
        "D",
        "the end of the draft shows"
    );
    assert_eq!(
        app.input.text, text,
        "visual wrapping must not edit submitted text"
    );

    app.input.cursor = 0;
    let (_, cursor) = draw_cursor(&app, w, h);
    assert_eq!(
        (cursor.x, usize::from(cursor.y)),
        (2, input.start),
        "the window scrolls back to the start"
    );
}

/// ↑ and ↓ walk the rows a person sees, not the lines in the text.
#[test]
fn arrow_keys_follow_the_wrapped_rows() {
    let (w, h) = (12u16, 30u16);
    let mut app = quiet(app_with_transcript(&[]));
    set_status(&mut app, AgentStatus::Ready);
    app.size = (w, h);
    // Eight columns of text at this width: two painted rows, one text line.
    app.input.set_text("abcdefghijk");
    let rows = screen(&app, w, h);
    assert_eq!(input_rows(&rows, app.theme.glyphs().prompt).len(), 2);
    app.reduce(key(KeyCode::Up));
    assert!(app.input.cursor < 11, "↑ stayed on the last row");
    let up = app.input.cursor;
    app.reduce(key(KeyCode::Down));
    assert_eq!(app.input.cursor, 11, "↓ came back to the end from {up}");
}

/// SPEC §3.4: a due approval takes the composer's place with its choices
/// and keys; the draft underneath survives the answer.
#[test]
fn an_approval_takes_the_composers_place_and_gives_it_back() {
    for ascii in [false, true] {
        for width in [80u16, 120] {
            let mut app = quiet(populated_app());
            app.theme = dark(ascii);
            app.input.set_text("half a thought");
            let theme = app.theme;
            let buffer = draw(&app, width, 30);
            let rows = rows(&buffer);
            let text = rows.join("\n");
            let title = row_with(&rows, "Allow assistant to run this command?")
                .unwrap_or_else(|| panic!("ascii={ascii} at {width}: no approval:\n{text}"));
            let once = row_with(&rows, "(y)").expect("approve once");
            let deny = row_with(&rows, "(esc)").expect("deny");
            assert!(title < once && once < deny, "{text}");
            assert!(
                deny < rows.len() - 2,
                "the choices sit above the status line:\n{text}"
            );
            assert!(
                !text.contains("half a thought"),
                "the draft shows under the approval"
            );
            let x = column_of(&rows[title], "Allow").expect("found");
            assert!(
                shows(&buffer[(x, title as u16)], theme.title()),
                "the title is not bold"
            );
            assert!(
                rows[once].starts_with(theme.glyphs().prompt),
                "the first choice is selected: {:?}",
                rows[once]
            );
            assert!(
                shows(&buffer[(0, once as u16)], theme.accent()),
                "the marker is not accent"
            );
            if ascii {
                assert!(text.is_ascii(), "{:?} {text}", non_ascii(&text));
            }

            read_card(&mut app);
            app.reduce(key(KeyCode::Char('y')));
            let text = screen(&app, width, 30).join("\n");
            assert!(
                !text.contains("(esc)"),
                "the answered approval stayed:\n{text}"
            );
            assert!(
                text.contains("half a thought"),
                "the draft did not come back:\n{text}"
            );
        }
    }
}

/// Esc denies, and the composer then asks what to do instead.
#[test]
fn denying_an_approval_asks_what_to_do_instead() {
    let mut app = quiet(populated_app());
    read_card(&mut app);
    app.reduce(key(KeyCode::Esc));
    let text = screen(&app, 120, 30).join("\n");
    assert!(!text.contains("(esc)"), "{text}");
    assert!(text.contains("Tell assistant what to do instead"), "{text}");
}

/// `?` on an empty composer opens the shortcuts above it, over the
/// transcript, and Esc puts them away.
#[test]
fn question_mark_opens_the_shortcuts_above_the_composer() {
    for ascii in [false, true] {
        for width in [80u16, 120] {
            let mut app = quiet(app_with_transcript(&["hello", "hello back"]));
            app.theme = dark(ascii);
            app.reduce(key(KeyCode::Char('?')));
            assert!(app.bottom.shortcuts);
            assert!(app.input.text.is_empty(), "the ? reached the draft");
            let buffer = draw(&app, width, 40);
            let rows = rows(&buffer);
            let text = rows.join("\n");
            let title = row_with(&rows, "Keyboard shortcuts")
                .unwrap_or_else(|| panic!("ascii={ascii} at {width}: no shortcuts:\n{text}"));
            let composer = row_with(&rows, "Steer assistant").expect("the composer stays");
            assert!(title < composer, "{text}");
            let x = column_of(&rows[title], "Keyboard").expect("found");
            assert!(shows(&buffer[(x, title as u16)], app.theme.title()));
            for keys in ["Ctrl+G", "Ctrl+B", "Ctrl+F", "Ctrl+T"] {
                assert!(text.contains(keys), "{keys} is not listed:\n{text}");
            }
            if ascii {
                assert!(text.is_ascii(), "{:?} {text}", non_ascii(&text));
            }

            app.reduce(key(KeyCode::Esc));
            assert!(
                !screen(&app, width, 40)
                    .join("\n")
                    .contains("Keyboard shortcuts")
            );
        }
    }
}

#[test]
fn essential_shortcuts_are_visible_without_scrolling_at_80_by_24() {
    for ascii in [false, true] {
        let mut app = quiet(app_with_transcript(&["hello", "hello back"]));
        app.theme = dark(ascii);
        app.reduce(key(KeyCode::Char('?')));
        let text = screen(&app, 80, 24).join("\n");
        for label in [
            "quit on empty draft",
            "close, else interrupt",
            "go to the agents",
            "shortcuts (empty draft)",
        ] {
            assert!(text.contains(label), "missing {label}:\n{text}");
        }
    }
}

/// Ctrl+F swaps the hint row for a find bar that counts matches in the
/// viewed transcript; Esc gives the hint row back.
#[test]
fn ctrl_f_finds_in_the_transcript_from_the_hint_row() {
    for ascii in [false, true] {
        for width in [80u16, 120] {
            let mut app = quiet(app_with_transcript(&["hello", "hello back"]));
            app.theme = dark(ascii);
            app.reduce(ctrl('f'));
            for c in "hello".chars() {
                app.reduce(key(KeyCode::Char(c)));
            }
            assert!(app.input.text.is_empty(), "the query reached the draft");
            let dot = app.theme.glyphs().dot;
            let caret = app.theme.glyphs().caret;
            let buffer = draw(&app, width, 24);
            let rows = rows(&buffer);
            let bar = rows.last().expect("a find bar").as_str();
            assert!(
                bar.starts_with(&format!("  find hello{caret}  1 of 2 {dot} enter next")),
                "ascii={ascii} at {width}: {bar:?}"
            );
            assert!(bar.contains("esc close"), "{bar:?}");
            let y = (rows.len() - 1) as u16;
            let x = column_of(bar, "enter").expect("found");
            assert!(
                shows(&buffer[(x, y)], app.theme.title()),
                "the key is not bold"
            );

            app.reduce(key(KeyCode::Esc));
            let rows = screen(&app, width, 24);
            let hint = rows.last().expect("a hint row");
            assert!(
                hint.contains("? shortcuts"),
                "the hints did not come back: {hint:?}"
            );
        }
    }
}
