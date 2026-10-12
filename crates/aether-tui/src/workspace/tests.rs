use std::fs;
use std::path::{Path, PathBuf};

use super::*;

fn mkdirs(root: &Path, dirs: &[&str]) {
    for dir in dirs {
        fs::create_dir_all(root.join(dir)).expect("mkdir");
    }
}

/// Discovery as it runs with nothing configured: home, two levels down.
fn under_home(home: &Path) -> Discovery {
    Discovery::new(None, None, Some(home))
}

/// Found paths relative to `root`, in the order found.
fn names(found: &[PathBuf], root: &Path) -> Vec<String> {
    found
        .iter()
        .map(|p| p.strip_prefix(root).unwrap().display().to_string())
        .collect()
}

#[test]
fn shorthand_is_the_name_with_home_and_root_spelled_out() {
    let home = Path::new("/home/me");
    assert_eq!(
        shorthand(Path::new("/home/me/src/aether"), Some(home)),
        "aether"
    );
    assert_eq!(shorthand(home, Some(home)), "~");
    assert_eq!(shorthand(Path::new("/"), Some(home)), "/");
    assert_eq!(
        display(Path::new("/home/me/src/aether"), Some(home)),
        "~/src/aether"
    );
    assert_eq!(display(Path::new("/opt/x"), Some(home)), "/opt/x");
    assert_eq!(display(home, Some(home)), "~");
}

#[test]
fn shared_names_get_the_shortest_suffix_that_tells_them_apart() {
    let home = Path::new("/home/me");
    let paths: Vec<PathBuf> = [
        "/home/me/aether",
        "/home/me/aether-wt/aether",
        "/home/me/murals",
        "/srv/a/x/tool",
        "/srv/b/x/tool",
        "/home/me",
    ]
    .iter()
    .map(PathBuf::from)
    .collect();
    let labels = shorthands(&paths, Some(home));
    let label = |p: &str| labels[Path::new(p)].as_str();
    assert_eq!(label("/home/me/aether"), "~/aether");
    assert_eq!(label("/home/me/aether-wt/aether"), "aether-wt/aether");
    assert_eq!(label("/home/me/murals"), "murals");
    assert_eq!(label("/srv/a/x/tool"), "a/x/tool");
    assert_eq!(label("/srv/b/x/tool"), "b/x/tool");
    assert_eq!(label("/home/me"), "~");

    // One path the suffix of another: the shorter one is written in full.
    let nested: Vec<PathBuf> = ["/tool", "/x/tool"].iter().map(PathBuf::from).collect();
    let labels = shorthands(&nested, None);
    assert_eq!(labels[Path::new("/tool")], "/tool");
    assert_eq!(labels[Path::new("/x/tool")], "x/tool");
}

#[test]
fn discovery_finds_repositories_one_and_two_levels_down_only() {
    let home = tempfile::tempdir().expect("tempdir");
    let root = home.path();
    mkdirs(
        root,
        &[
            "alpha/.git",
            "code/beta/.git",
            "code/gamma/src",
            "code/deep/deeper/.git",
            ".hidden/.git",
            "code/.secret/.git",
            "alpha/nested/.git",
        ],
    );
    // A worktree's `.git` is a file.
    mkdirs(root, &["code/wt"]);
    fs::write(root.join("code/wt/.git"), "gitdir: /elsewhere\n").expect("write");
    std::os::unix::fs::symlink(root.join("alpha"), root.join("link")).expect("symlink");

    let found = under_home(root).search().projects;
    assert_eq!(names(&found, root), ["alpha", "code/beta", "code/wt"]);
}

#[test]
fn discovery_stops_at_its_caps() {
    let home = tempfile::tempdir().expect("tempdir");
    for n in 0..(MAX_FOUND + 5) {
        fs::create_dir_all(home.path().join(format!("p{n:03}/.git"))).expect("mkdir");
    }
    assert_eq!(under_home(home.path()).search().projects.len(), MAX_FOUND);
}

/// Every filesystem call discovery makes is paid from its budget, so a home
/// wider than the budget is cut short, not read to the end.
#[test]
fn discovery_work_is_bounded_by_its_budget() {
    let wide = tempfile::tempdir().expect("tempdir");
    for n in 0..300 {
        fs::create_dir_all(wide.path().join(format!("d{n:04}"))).expect("mkdir");
    }
    fs::create_dir_all(wide.path().join("zzz/.git")).expect("mkdir");
    let mut budget = Budget::new(100);
    assert!(
        under_home(wide.path())
            .within(&mut budget)
            .projects
            .is_empty(),
        "past the budget nothing more is looked at"
    );
    assert_eq!(budget.used(), 100);

    let mut ample = Budget::new(DISCOVERY_CALLS);
    let found = under_home(wide.path()).within(&mut ample).projects;
    assert_eq!(found, [wide.path().join("zzz")]);
    assert!(ample.used() < DISCOVERY_CALLS, "{}", ample.used());
}

#[test]
fn configured_roots_expand_tilde_and_skip_what_is_missing_or_relative() {
    let home = tempfile::tempdir().expect("tempdir");
    let root = home.path();
    let elsewhere = tempfile::tempdir().expect("tempdir");
    mkdirs(root, &["work/one/.git", "stray/.git"]);
    mkdirs(elsewhere.path(), &["two/.git"]);
    let roots = [
        "~/missing".to_owned(),
        "~/work".to_owned(),
        "relative/path".to_owned(),
        "~someone/x".to_owned(),
        elsewhere.path().display().to_string(),
    ];
    let found = Discovery::new(Some(&roots), None, Some(root))
        .search()
        .projects;
    assert_eq!(
        found,
        [root.join("work/one"), elsewhere.path().join("two")],
        "only the configured roots are searched, not home"
    );
    assert!(
        Discovery::new(Some(&[]), None, Some(root))
            .search()
            .projects
            .is_empty(),
        "an empty list looks nowhere"
    );
}

#[test]
fn depth_counts_levels_below_each_root_and_is_clamped() {
    let home = tempfile::tempdir().expect("tempdir");
    let root = home.path();
    mkdirs(
        root,
        &["a/.git", "x/b/.git", "x/y/c/.git", "x/y/z/w/d/.git"],
    );
    let at = |depth| {
        names(
            &Discovery::new(None, Some(depth), Some(root))
                .search()
                .projects,
            root,
        )
    };
    assert_eq!(at(1), ["a"]);
    assert_eq!(at(2), ["a", "x/b"]);
    assert_eq!(at(3), ["a", "x/b", "x/y/c"]);
    assert_eq!(at(0), at(1), "clamped up to one level");
    assert_eq!(at(9), at(MAX_DEPTH), "clamped down to the maximum");
    assert_eq!(
        at(MAX_DEPTH),
        ["a", "x/b", "x/y/c"],
        "five levels is too deep"
    );
}

#[test]
fn a_root_that_is_a_repository_is_listed_and_not_searched_but_home_never_is() {
    let home = tempfile::tempdir().expect("tempdir");
    let root = home.path();
    mkdirs(
        root,
        &[".git", "repo/.git", "repo/nested/.git", "plain/inner/.git"],
    );
    let roots = ["~/repo".to_owned(), "~/plain".to_owned()];
    let found = Discovery::new(Some(&roots), None, Some(root))
        .search()
        .projects;
    assert_eq!(names(&found, root), ["repo", "plain/inner"]);

    let found = under_home(root).search().projects;
    assert_eq!(
        names(&found, root),
        ["repo", "plain/inner"],
        "a home under git is still searched, never listed"
    );
    let explicit = ["~".to_owned()];
    assert_eq!(
        Discovery::new(Some(&explicit), None, Some(root))
            .search()
            .projects,
        found
    );
}

#[test]
fn overlapping_and_repeated_roots_find_each_project_once() {
    let home = tempfile::tempdir().expect("tempdir");
    let root = home.path();
    mkdirs(root, &["src/aether/.git", "src/murals/.git", "top/.git"]);
    let roots = [
        "~".to_owned(),
        "~/src".to_owned(),
        "~/src/".to_owned(),
        "~/src/aether".to_owned(),
    ];
    let found = Discovery::new(Some(&roots), None, Some(root))
        .search()
        .projects;
    // `src/aether` is a root itself; `top` and `src/murals` are one level down.
    assert_eq!(names(&found, root), ["src/aether", "top", "src/murals"]);
}

/// Missing roots are found by the search itself, under its budget: a repeated
/// spelling is checked once, and past the budget nothing more is checked.
#[test]
fn missing_roots_are_checked_once_each_within_the_budget() {
    let home = tempfile::tempdir().expect("tempdir");
    let root = home.path();
    mkdirs(root, &["here/one/.git"]);
    fs::write(root.join("file"), "").expect("write");
    let mut roots = vec!["~/nope".to_owned(); 5000];
    roots.extend([
        "relative".into(),
        "~/file".into(),
        "~/here".into(),
        "relative".into(),
    ]);
    let discovery = Discovery::new(Some(&roots), Some(1), Some(root));
    let mut budget = Budget::new(DISCOVERY_CALLS);
    let found = discovery.within(&mut budget);
    assert_eq!(names(&found.projects, root), ["here/one"]);
    assert_eq!(found.missing, ["~/nope", "~/file", "relative"]);
    assert!(budget.used() < 20, "{}", budget.used());

    let distinct: Vec<String> = (0..5000).map(|n| format!("~/gone{n}")).collect();
    let mut budget = Budget::new(DISCOVERY_CALLS);
    let found = Discovery::new(Some(&distinct), None, Some(root)).within(&mut budget);
    assert_eq!(budget.used(), DISCOVERY_CALLS);
    assert_eq!(
        found.missing.len(),
        (DISCOVERY_CALLS - 1) / 2,
        "home, then two calls a root"
    );

    assert!(
        under_home(&root.join("gone")).search().missing.is_empty(),
        "home by default is never flagged"
    );
}

/// A symlinked root is the directory it names: its projects are found once,
/// leaving room under the cap for a later root's.
#[test]
fn a_symlinked_root_is_searched_once_with_the_root_it_aliases() {
    let home = tempfile::tempdir().expect("tempdir");
    let root = home.path();
    for n in 0..25 {
        mkdirs(root, &[&format!("a/p{n:02}/.git")]);
    }
    mkdirs(root, &["c/last/.git"]);
    std::os::unix::fs::symlink(root.join("a"), root.join("b")).expect("symlink");
    let roots = ["~/a".to_owned(), "~/b".to_owned(), "~/c".to_owned()];
    let found = Discovery::new(Some(&roots), Some(1), Some(root))
        .search()
        .projects;
    assert_eq!(found.len(), 26, "{:?}", names(&found, root));
    assert_eq!(found.last(), Some(&root.join("c/last")));
}

/// Home is never a project however it is reached: under a root above it, or
/// through a symlink. Searching goes on below it.
#[test]
fn home_is_searched_never_listed_from_a_root_above_it_or_a_link_to_it() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let base = tmp.path();
    let home = base.join("users/me");
    mkdirs(
        base,
        &["users/me/.git", "users/me/project/.git", "users/other/.git"],
    );
    let above = [base.join("users").display().to_string()];
    let found = Discovery::new(Some(&above), Some(2), Some(&home))
        .search()
        .projects;
    assert_eq!(names(&found, base), ["users/other", "users/me/project"]);

    std::os::unix::fs::symlink(&home, base.join("alias")).expect("symlink");
    let alias = [base.join("alias").display().to_string()];
    let found = Discovery::new(Some(&alias), Some(1), Some(&home))
        .search()
        .projects;
    assert_eq!(names(&found, base), ["users/me/project"]);
}

/// One budget across every root, spent level by level: a shallow project
/// under the second root wins over a deep one under the first.
#[test]
fn discovery_is_breadth_first_across_roots_under_one_budget() {
    let home = tempfile::tempdir().expect("tempdir");
    let root = home.path();
    for n in 0..50 {
        mkdirs(root, &[&format!("wide/d{n:02}")]);
    }
    mkdirs(root, &["wide/zz/deep/.git", "narrow/shallow/.git"]);
    let roots = ["~/wide".to_owned(), "~/narrow".to_owned()];
    let discovery = Discovery::new(Some(&roots), None, Some(root));

    let found = discovery.search().projects;
    assert_eq!(names(&found, root), ["narrow/shallow", "wide/zz/deep"]);

    // Home, both roots and level one cost 167 calls; level two of `wide` far more.
    let mut tight = Budget::new(200);
    let found = discovery.within(&mut tight).projects;
    assert_eq!(names(&found, root), ["narrow/shallow"]);
    assert_eq!(tight.used(), 200, "the budget is shared, then spent");
}

#[test]
fn paths_resolve_tilde_relative_and_dot_dot() {
    let home = Path::new("/home/me");
    let base = Path::new("/home/me/src/aether");
    let r = |text| resolve(text, Some(home), Some(base));
    assert_eq!(r("~"), Some(PathBuf::from("/home/me")));
    assert_eq!(r("~/murals"), Some(PathBuf::from("/home/me/murals")));
    assert_eq!(r("/opt/./x/../y"), Some(PathBuf::from("/opt/y")));
    assert_eq!(r("../murals"), Some(PathBuf::from("/home/me/src/murals")));
    assert_eq!(
        r("./crates"),
        Some(PathBuf::from("/home/me/src/aether/crates"))
    );
    assert_eq!(r("~other/x"), None, "no guessing other users' homes");
    assert_eq!(resolve("~/x", None, Some(base)), None);
    assert_eq!(resolve("./x", Some(home), None), None);

    assert!(looks_like_path("/x") && looks_like_path("~") && looks_like_path("./x"));
    assert!(looks_like_path("../x"));
    assert!(!looks_like_path("aether") && !looks_like_path(".hidden"));
}

#[test]
fn completion_lists_the_directory_then_matching_siblings() {
    let home = tempfile::tempdir().expect("tempdir");
    let root = home.path();
    mkdirs(
        root,
        &["proj", "projects/one", "prose", "other", ".private"],
    );
    fs::write(root.join("profile.txt"), "").expect("write");
    let h = Some(root);

    let c = complete("~/pro", h, None, &mut None);
    assert_eq!(c.exact, None);
    assert_eq!(
        c.candidates,
        [root.join("proj"), root.join("projects"), root.join("prose")]
    );

    let c = complete("~/proj", h, None, &mut None);
    assert_eq!(c.exact, Some(root.join("proj")));
    assert_eq!(c.candidates, [root.join("projects")], "itself not repeated");

    let c = complete("~/projects/", h, None, &mut None);
    assert_eq!(c.exact, Some(root.join("projects")));
    assert_eq!(c.candidates, [root.join("projects/one")]);

    let c = complete("~/", h, None, &mut None);
    assert!(
        !c.candidates.contains(&root.join(".private")),
        "hidden only on a dot"
    );
    assert!(
        !c.candidates.contains(&root.join("profile.txt")),
        "directories only"
    );
    let c = complete("~/.p", h, None, &mut None);
    assert_eq!(c.candidates, [root.join(".private")]);

    let c = complete("~/nothing-here", h, None, &mut None);
    assert_eq!(c, Completion::default());
    let c = complete("~/profile.txt", h, None, &mut None);
    assert_eq!(c.exact, None, "a file is not somewhere to work");
}

#[test]
fn completion_is_capped() {
    let home = tempfile::tempdir().expect("tempdir");
    for n in 0..(MAX_COMPLETIONS + 7) {
        fs::create_dir_all(home.path().join(format!("d{n:03}"))).expect("mkdir");
    }
    let c = complete("~/d", Some(home.path()), None, &mut None);
    assert_eq!(c.candidates.len(), MAX_COMPLETIONS);
}

/// Reading a directory wider than its budget stops at the budget, and only
/// names matching the typed prefix are checked for being directories.
#[test]
fn completion_work_is_bounded_and_filters_names_first() {
    let dir = tempfile::tempdir().expect("tempdir");
    for n in 0..300 {
        fs::write(dir.path().join(format!("a{n:03}")), "").expect("write");
    }
    mkdirs(dir.path(), &["zz1", "zz2", "zz3"]);

    let mut budget = Budget::new(100);
    let short = Listing::read(dir.path(), &mut budget);
    assert_eq!(budget.used(), 100);
    assert!(short.entries.len() < 100, "{}", short.entries.len());

    let mut listing = Listing::read(dir.path(), &mut Budget::new(LISTING_CALLS));
    assert_eq!(listing.entries.len(), 303);
    let mut probes = Budget::new(COMPLETION_PROBES);
    let found = listing.dirs("zz", MAX_COMPLETIONS, &mut probes);
    assert_eq!(found.len(), 3);
    assert_eq!(probes.used(), 3, "only matching names are checked");

    // Checked once: the next keystroke reuses what is known.
    let mut again = Budget::new(COMPLETION_PROBES);
    assert_eq!(listing.dirs("zz", MAX_COMPLETIONS, &mut again), found);
    assert_eq!(again.used(), 0);

    // A prefix matching only files spends its probes and stops.
    let mut few = Budget::new(20);
    assert!(listing.dirs("a", MAX_COMPLETIONS, &mut few).is_empty());
    assert_eq!(few.used(), 20);
}

/// Keystrokes within one directory reuse its listing; another directory
/// replaces it.
#[test]
fn completion_reads_a_directory_once_while_typing_stays_in_it() {
    let home = tempfile::tempdir().expect("tempdir");
    mkdirs(home.path(), &["d1", "d2", "other/inner"]);
    let h = Some(home.path());
    let mut listing = None;
    assert_eq!(complete("~/d", h, None, &mut listing).candidates.len(), 2);
    fs::create_dir(home.path().join("d3")).expect("mkdir");
    assert_eq!(
        complete("~/d", h, None, &mut listing).candidates.len(),
        2,
        "the cached listing is used"
    );
    let c = complete("~/other/", h, None, &mut listing);
    assert_eq!(c.candidates, [home.path().join("other/inner")]);
    assert_eq!(complete("~/d", h, None, &mut listing).candidates.len(), 3);
}

#[test]
fn tab_completion_keeps_the_typed_spelling_and_ends_in_a_slash() {
    let home = Path::new("/home/me");
    let base = Path::new("/home/me/src");
    let done = |typed, chosen: &str| {
        completed_text(typed, Path::new(chosen), Some(home), Some(base)).expect("UTF-8 path")
    };
    assert_eq!(done("~/pro", "/home/me/projects"), "~/projects/");
    assert_eq!(done("~/proj", "/home/me/proj"), "~/proj/");
    assert_eq!(done("~/projects/", "/home/me/projects"), "~/projects/");
    assert_eq!(
        done("~/projects/", "/home/me/projects/one"),
        "~/projects/one/"
    );
    assert_eq!(done("~", "/home/me/murals"), "~/murals/");
    assert_eq!(done("../mu", "/home/me/murals"), "../murals/");
    assert_eq!(done("/opt/t", "/opt/tools"), "/opt/tools/");
    // A filter, not a path: Tab writes the row's own path.
    assert_eq!(done("aeth", "/home/me/aether"), "~/aether/");
    assert_eq!(done("x", "/opt/x"), "/opt/x/");
}

/// A lossy spelling of a non-UTF-8 name could resolve to a sibling, so both
/// the typed-path and the filter branch refuse it.
#[test]
fn tab_completion_refuses_a_path_text_cannot_spell() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    let home = Path::new("/home/me");
    let chosen = home.join(OsStr::from_bytes(b"proj-\xFF"));
    for typed in ["~/pro", "proj"] {
        assert_eq!(
            completed_text(typed, &chosen, Some(home), Some(home)),
            Err(NotUtf8(chosen.clone())),
            "{typed}"
        );
    }
}

#[test]
fn canonical_merges_spellings_and_keeps_paths_it_cannot_resolve() {
    let dir = tempfile::tempdir().expect("tempdir");
    mkdirs(dir.path(), &["real"]);
    std::os::unix::fs::symlink(dir.path().join("real"), dir.path().join("link")).expect("symlink");
    assert_eq!(
        canonical(&dir.path().join("link")),
        canonical(&dir.path().join("real"))
    );
    let gone = dir.path().join("gone");
    assert_eq!(canonical(&gone), gone);
}
