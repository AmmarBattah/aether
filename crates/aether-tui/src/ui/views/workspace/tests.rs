use std::fs;
use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::*;
use crate::app::TuiOptions;

/// A fake home: `aether` and `murals` checkouts, `notes` a plain directory.
fn home() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().expect("tempdir");
    for dir in [
        "aether/.git",
        "murals/.git",
        "notes",
        "projects/one",
        "projects/two",
    ] {
        fs::create_dir_all(tmp.path().join(dir)).expect("mkdir");
    }
    let home = workspace::canonical(tmp.path());
    (tmp, home)
}

fn live(path: &Path, agents: usize, running: usize) -> LiveWorkspace {
    LiveWorkspace {
        path: path.to_path_buf(),
        agents,
        running,
        active_at: aether_core::time::now(),
    }
}

fn sources(home: &Path) -> Sources {
    Sources {
        pinned: Vec::new(),
        live: vec![
            live(&home.join("murals"), 2, 1),
            live(&home.join("aether"), 1, 0),
        ],
        recent: vec![home.join("notes"), home.join("aether"), home.join("gone")],
        launch: Some(home.join("notes")),
        discovered: workspace::Discovery::new(None, None, Some(home))
            .search()
            .projects,
        current: Some(home.join("aether")),
        home: Some(home.to_path_buf()),
    }
}

fn main_branch(path: &Path) -> Option<String> {
    path.ends_with("aether").then(|| "main".to_owned())
}

fn view(home: &Path) -> WorkspaceView {
    let sources = sources(home);
    WorkspaceView::from_sources(&sources, "·", &main_branch)
}

fn press(view: &mut WorkspaceView, code: KeyCode) -> ViewOutcome {
    let app = App::new(TuiOptions::default());
    view.handle_key(KeyEvent::new(code, KeyModifiers::NONE), &app)
}

fn ctrl(view: &mut WorkspaceView, c: char) -> ViewOutcome {
    let app = App::new(TuiOptions::default());
    view.handle_chord(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL), &app)
}

/// The pin Ctrl+P asked the app to save.
fn pin_of(outcome: ViewOutcome) -> (PathBuf, bool) {
    match outcome {
        ViewOutcome::Act(actions) => match actions.as_slice() {
            [ViewAction::PinWorkspace { path, pinned }] => (path.clone(), *pinned),
            other => panic!("{other:?}"),
        },
        other => panic!("{other:?}"),
    }
}

fn type_text(view: &mut WorkspaceView, text: &str) {
    for c in text.chars() {
        assert!(matches!(
            press(view, KeyCode::Char(c)),
            ViewOutcome::Consumed
        ));
    }
}

fn labels(view: &WorkspaceView) -> Vec<String> {
    view.visible_rows()
        .into_iter()
        .map(|(row, _)| row.label.clone())
        .collect()
}

fn opened(outcome: ViewOutcome) -> PathBuf {
    match outcome {
        ViewOutcome::Close(actions) => match actions.as_slice() {
            [ViewAction::Command(SlashCommand::Workspace(Some(path)))] => path.clone(),
            other => panic!("{other:?}"),
        },
        other => panic!("{other:?}"),
    }
}

#[test]
fn rows_go_live_then_recent_then_launch_then_found_once_each() {
    let (_tmp, home) = home();
    let rows = rows(&sources(&home), "·", &main_branch);
    let shown: Vec<(&str, &str)> = rows
        .iter()
        .map(|(row, _)| (row.label.as_str(), row.detail.as_str()))
        .collect();
    assert_eq!(
        shown,
        [
            ("murals", "~/murals · 2 agents, 1 running"),
            ("aether", "~/aether · main · 1 agent · current"),
            ("notes", "~/notes · recent"),
        ],
        "a recent that is gone is skipped; the launch dir and discovered repos already listed"
    );

    let mut more = sources(&home);
    more.live.clear();
    more.recent.clear();
    let rows = super::rows(&more, "·", &main_branch);
    let shown: Vec<(&str, &str)> = rows
        .iter()
        .map(|(row, _)| (row.label.as_str(), row.detail.as_str()))
        .collect();
    assert_eq!(
        shown,
        [
            ("notes", "~/notes · launched here"),
            ("aether", "~/aether · current"),
            ("murals", "~/murals"),
        ],
        "discovered rows show no branch"
    );
}

#[test]
fn the_cursor_starts_on_the_current_workspace() {
    let (_tmp, home) = home();
    let view = view(&home);
    assert_eq!(view.highlighted(), Some(home.join("aether").as_path()));
}

#[test]
fn typing_filters_and_enter_opens_the_row() {
    let (_tmp, home) = home();
    let mut v = view(&home);
    type_text(&mut v, "mur");
    assert_eq!(labels(&v), ["murals"]);
    assert_eq!(opened(press(&mut v, KeyCode::Enter)), home.join("murals"));
}

#[test]
fn a_digit_picks_by_number_on_a_short_list_and_is_typed_after_a_filter() {
    let (_tmp, home) = home();
    let mut v = view(&home);
    assert_eq!(
        opened(press(&mut v, KeyCode::Char('3'))),
        home.join("notes")
    );
    let mut v = view(&home);
    type_text(&mut v, "n3");
    assert_eq!(v.query(), "n3");
}

#[test]
fn a_typed_path_lists_the_directory_and_tab_goes_one_level_down() {
    let (_tmp, home) = home();
    let mut v = view(&home);
    type_text(&mut v, "~/pro");
    assert_eq!(labels(&v), ["projects"]);
    assert!(matches!(press(&mut v, KeyCode::Tab), ViewOutcome::Consumed));
    assert_eq!(v.query(), "~/projects/");
    assert_eq!(labels(&v), ["projects", "one", "two"]);
    press(&mut v, KeyCode::Down);
    assert!(matches!(press(&mut v, KeyCode::Tab), ViewOutcome::Consumed));
    assert_eq!(v.query(), "~/projects/one/");
    assert_eq!(
        opened(press(&mut v, KeyCode::Enter)),
        home.join("projects/one")
    );
}

/// R5: a typed path listing a directory whose name is not UTF-8 refuses Tab
/// on it out loud; its lossy spelling would name the sibling beside it.
#[cfg(unix)]
#[test]
fn tab_on_a_non_utf8_directory_is_refused_and_keeps_the_query() {
    use std::os::unix::ffi::OsStringExt;
    let (_tmp, home) = home();
    let raw = home.join(std::ffi::OsString::from_vec(b"proj-\xff".to_vec()));
    fs::create_dir(&raw).expect("mkdir raw");
    fs::create_dir(home.join("proj-\u{FFFD}")).expect("mkdir lookalike");
    let mut v = view(&home);
    type_text(&mut v, "~/proj-");
    let at = v
        .visible_rows()
        .iter()
        .position(|(_, path)| *path == raw)
        .expect("listed");
    for _ in 0..at {
        press(&mut v, KeyCode::Down);
    }
    assert_eq!(v.highlighted(), Some(raw.as_path()));
    match press(&mut v, KeyCode::Tab) {
        ViewOutcome::Act(actions) => assert!(
            matches!(actions.as_slice(), [ViewAction::Flash(text)] if text.contains("not UTF-8")),
            "{actions:?}"
        ),
        other => panic!("{other:?}"),
    }
    assert_eq!(v.query(), "~/proj-");
    assert_eq!(v.highlighted(), Some(raw.as_path()));
}

#[test]
fn a_path_to_nowhere_is_one_row_that_cannot_be_chosen() {
    let (_tmp, home) = home();
    let mut v = view(&home);
    type_text(&mut v, "~/nowhere");
    let rows = v.visible_rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0.detail, "no such directory");
    assert!(rows[0].0.disabled);
    assert!(matches!(
        press(&mut v, KeyCode::Enter),
        ViewOutcome::Consumed
    ));
    assert!(matches!(press(&mut v, KeyCode::Tab), ViewOutcome::Consumed));
    assert_eq!(v.query(), "~/nowhere", "nothing to complete");
}

#[test]
fn esc_clears_the_query_first_then_closes() {
    let (_tmp, home) = home();
    let mut v = view(&home);
    type_text(&mut v, "~/no");
    press(&mut v, KeyCode::Backspace);
    assert_eq!(v.query(), "~/n");
    press(&mut v, KeyCode::Esc);
    assert_eq!(v.query(), "");
    assert_eq!(labels(&v).len(), 3);
    assert!(matches!(press(&mut v, KeyCode::Esc), ViewOutcome::Close(a) if a.is_empty()));
}

#[test]
fn chords_pass_through_so_ctrl_o_closes_it() {
    let (_tmp, home) = home();
    let mut v = view(&home);
    let app = App::new(TuiOptions::default());
    let ctrl_o = KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL);
    assert!(matches!(v.handle_key(ctrl_o, &app), ViewOutcome::Ignored));
    assert!(matches!(v.handle_chord(ctrl_o, &app), ViewOutcome::Ignored));
    assert!(matches!(ctrl(&mut v, 'c'), ViewOutcome::Ignored));
}

#[test]
fn pinned_rows_come_first_in_pin_order_and_a_gone_pin_is_hidden() {
    let (_tmp, home) = home();
    let mut pinned = sources(&home);
    pinned.pinned = vec![
        home.join("notes"),
        home.join("gone"),
        home.join("murals"),
        home.join("projects/one"),
        home.join("notes"),
    ];
    let rows = rows(&pinned, "·", &main_branch);
    let shown: Vec<(&str, &str)> = rows
        .iter()
        .map(|(row, _)| (row.label.as_str(), row.detail.as_str()))
        .collect();
    assert_eq!(
        shown,
        [
            ("notes", "~/notes · recent · pinned"),
            ("murals", "~/murals · 2 agents, 1 running · pinned"),
            ("one", "~/projects/one · pinned"),
            ("aether", "~/aether · main · 1 agent · current"),
        ],
        "pinned before live, each once; a pin from no other list still shows"
    );
}

#[test]
fn ctrl_p_pins_and_unpins_the_highlighted_row_and_the_cursor_follows_it() {
    let (_tmp, home) = home();
    let mut v = view(&home);
    assert_eq!(labels(&v), ["murals", "aether", "notes"]);
    assert_eq!(pin_of(ctrl(&mut v, 'p')), (home.join("aether"), true));
    assert_eq!(labels(&v), ["aether", "murals", "notes"]);
    assert_eq!(v.highlighted(), Some(home.join("aether").as_path()));
    assert!(v.visible_rows()[0].0.detail.ends_with("current · pinned"));

    press(&mut v, KeyCode::End);
    assert_eq!(pin_of(ctrl(&mut v, 'p')), (home.join("notes"), true));
    assert_eq!(
        labels(&v),
        ["aether", "notes", "murals"],
        "after the first pin"
    );
    assert_eq!(v.highlighted(), Some(home.join("notes").as_path()));

    press(&mut v, KeyCode::Home);
    assert_eq!(pin_of(ctrl(&mut v, 'p')), (home.join("aether"), false));
    assert_eq!(labels(&v), ["notes", "murals", "aether"]);
    assert_eq!(v.highlighted(), Some(home.join("aether").as_path()));
    assert!(!v.visible_rows()[2].0.detail.contains("pinned"));
}

#[test]
fn ctrl_p_keeps_the_filter_and_skips_a_row_that_cannot_be_chosen() {
    let (_tmp, home) = home();
    let mut v = view(&home);
    type_text(&mut v, "mur");
    assert_eq!(pin_of(ctrl(&mut v, 'p')), (home.join("murals"), true));
    assert_eq!(v.query(), "mur");
    assert_eq!(labels(&v), ["murals"]);

    let mut v = view(&home);
    type_text(&mut v, "~/nowhere");
    assert!(matches!(ctrl(&mut v, 'p'), ViewOutcome::Consumed));
}

#[test]
fn a_typed_path_can_be_pinned_and_shows_once_the_query_is_cleared() {
    let (_tmp, home) = home();
    let mut v = view(&home);
    type_text(&mut v, "~/projects/two");
    assert_eq!(pin_of(ctrl(&mut v, 'p')), (home.join("projects/two"), true));
    assert_eq!(v.query(), "~/projects/two", "the typed path stays");
    press(&mut v, KeyCode::Esc);
    assert_eq!(labels(&v), ["two", "murals", "aether", "notes"]);
    assert_eq!(v.visible_rows()[0].0.detail, "~/projects/two · pinned");
}

#[cfg(unix)]
#[test]
fn ctrl_p_on_a_non_utf8_directory_is_refused() {
    use std::os::unix::ffi::OsStringExt;
    let (_tmp, home) = home();
    let raw = home.join(std::ffi::OsString::from_vec(b"proj-\xff".to_vec()));
    fs::create_dir(&raw).expect("mkdir raw");
    let mut v = view(&home);
    type_text(&mut v, "~/proj-");
    assert_eq!(v.highlighted(), Some(raw.as_path()));
    match ctrl(&mut v, 'p') {
        ViewOutcome::Act(actions) => assert!(
            matches!(actions.as_slice(), [ViewAction::Flash(text)] if text.contains("not UTF-8")),
            "{actions:?}"
        ),
        other => panic!("{other:?}"),
    }
    press(&mut v, KeyCode::Esc);
    assert_eq!(labels(&v), ["murals", "aether", "notes"], "nothing pinned");
}

#[test]
fn a_pasted_path_takes_its_first_line() {
    let (_tmp, home) = home();
    let mut v = view(&home);
    v.handle_paste(&format!("{}\nrm -rf /\n", home.join("notes").display()));
    assert_eq!(labels(&v), ["notes"]);
}
