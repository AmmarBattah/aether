use super::*;
use aether_tui::ui::bottom::popup::{self, PopupItem};

/// The `/` popup's static command names, in registry order.
fn palette(app: &App) -> Vec<&'static str> {
    let Some(popup) = &app.bottom.popup else {
        return Vec::new();
    };
    popup::items(popup, app)
        .into_iter()
        .filter_map(|item| match item {
            PopupItem::Command { name, .. } => Some(name),
            PopupItem::Mode { .. } => None,
            other => panic!("the / popup listed {other:?}"),
        })
        .collect()
}

#[test]
fn the_palette_filters_as_you_type() {
    assert!(
        palette(&App::new(TuiOptions::default())).is_empty(),
        "nothing typed, no palette"
    );
    let mut app = App::new(TuiOptions::default());
    type_text(&mut app, "/");
    assert_eq!(
        palette(&app).len(),
        COMMANDS.len(),
        "/ alone lists them all"
    );
    type_text(&mut app, "me");
    let rows = palette(&app);
    assert_eq!(
        rows.first(),
        Some(&"memory"),
        "the prefix match leads: {rows:?}"
    );
    assert!(
        rows.len() < COMMANDS.len(),
        "typing narrows the palette: {rows:?}"
    );
    for name in &rows {
        assert!(
            popup::fuzzy_match(name, "me").is_some(),
            "/{name} does not match what was typed"
        );
    }
}

/// A transcript whose early reply mentioning cargo is scrolled off a
/// 24-row screen, the way a long session buries it.
fn buried_cargo() -> App {
    let mut messages = vec!["how do I build this", "run cargo build"];
    messages.extend(std::iter::repeat_n("unrelated chatter", 40));
    messages.extend(["and the tests", "cargo test --workspace"]);
    aether_tui::testing::app_with_transcript(&messages)
}

/// The hit `current` points at, as plain text.
fn current_hit(app: &App) -> String {
    use aether_tui::ui::cells::ChatCell as _;

    let agent = app.selected.expect("the fixture views its agent");
    let bar = app.bottom.find.as_ref().expect("find is open");
    let id = bar.matches[bar.current.expect("a current hit")];
    let cell = app.transcripts[&agent]
        .cells
        .iter()
        .find(|c| c.id == id)
        .expect("a hit is a cell");
    cell.model.plain_lines(1_000).join("\n")
}

/// Search must find what it says (§9.5): Ctrl+F narrows as you type, Enter
/// steps through the hits, Esc closes, and a miss finds nothing.
#[test]
fn find_narrows_to_matching_replies_and_steps_through_them() {
    let mut app = buried_cargo();
    press(&mut app, KeyCode::Char('f'), KeyModifiers::CONTROL);
    let bar = app.bottom.find.as_ref().expect("Ctrl+F opens find");
    assert!(bar.matches.is_empty(), "an empty query finds nothing");

    type_text(&mut app, "cargo");
    let count = app.bottom.find.as_ref().unwrap().matches.len();
    assert_eq!(count, 2, "both replies mentioning cargo should be found");
    let mut seen = Vec::new();
    for _ in 0..count {
        let hit = current_hit(&app);
        assert!(hit.contains("cargo"), "{hit:?} is not a hit");
        seen.push(hit);
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    }
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), count, "Enter must visit every hit: {seen:?}");

    press(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(app.bottom.find.is_none(), "Esc closes find");
    press(&mut app, KeyCode::Char('f'), KeyModifiers::CONTROL);
    type_text(&mut app, "zzzz");
    let bar = app.bottom.find.as_ref().expect("find open");
    assert!(bar.matches.is_empty(), "no match, no hits");
    assert_eq!(bar.current, None);
}

/// Find must land where it says: a hit scrolled out of view is brought back.
#[test]
fn find_reveals_a_buried_hit_on_screen() {
    let mut app = buried_cargo();
    let mut cache = aether_tui::ui::transcript::ViewCache::default();
    let first = aether_tui::testing::render_with(&app, &mut cache, 100, 24);
    assert!(
        !first.contains("run cargo build"),
        "the early reply must start scrolled off: {first}"
    );
    press(&mut app, KeyCode::Char('f'), KeyModifiers::CONTROL);
    type_text(&mut app, "cargo");
    while !current_hit(&app).contains("run cargo build") {
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    }
    let shown = aether_tui::testing::render_with(&app, &mut cache, 100, 24);
    assert!(
        shown.contains("run cargo build"),
        "the current hit must be on screen: {shown}"
    );
}

#[test]
fn every_registry_command_is_reachable_by_scrolling_the_palette() {
    for width in [59, 60, 99, 100] {
        let mut app = App::new(TuiOptions::default());
        type_text(&mut app, "/");
        let names = palette(&app);
        assert_eq!(names.len(), COMMANDS.len());
        assert!(
            names.len() > usize::from(popup::POPUP_MAX_ROWS),
            "exercise an actual scrolled viewport"
        );
        for (index, name) in names.iter().enumerate() {
            let screen = aether_tui::testing::render_to(&app, width, 16);
            assert_eq!(app.bottom.popup.as_ref().map(|p| p.cursor), Some(index));
            assert!(
                screen.contains(&format!("/{name}")),
                "{width}/{name}: {screen}"
            );
            if index + 1 < names.len() {
                press(&mut app, KeyCode::Down, KeyModifiers::NONE);
            }
        }
        press(&mut app, KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(app.input.text, format!("/{} ", names.last().unwrap()));
    }
}
