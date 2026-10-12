use super::*;

#[test]
fn the_working_directory_decides_what_a_relative_path_means() {
    let owned = vec![PathBuf::from("/ws/src/mine")];
    // The same relative path is inside from one directory and outside
    // from another.
    assert!(refusal("git restore -- a.py", Path::new("/ws/src/mine"), &owned).is_none());
    assert!(refusal("git restore -- a.py", Path::new("/ws/src"), &owned).is_some());
    // `-C` moves what the pathspecs are relative to, and repeats compose.
    assert!(refusal("git -C src/mine restore -- a.py", Path::new(CWD), &owned).is_none());
    assert!(refusal("git -C src restore -- a.py", Path::new(CWD), &owned).is_some());
    assert!(refusal("git -C src -C mine restore -- a.py", Path::new(CWD), &owned).is_none());

    // `git clean` with no pathspec deletes from the working directory
    // down, so inside an owned directory it stays inside the boundary.
    assert!(refusal("git clean -fd", Path::new("/ws/src/mine"), &owned).is_none());
    assert!(refusal("git clean -fd", Path::new("/ws/src"), &owned).is_some());
    // A stash is repository-wide wherever it is run from.
    assert!(refusal("git stash", Path::new("/ws/src/mine"), &owned).is_some());
}

#[test]
fn literal_directory_changes_update_subsequent_git_paths() {
    for change in ["cd", "pushd"] {
        refused(&format!("{change} theirs && git restore -- src/mine/a.py"));
        permitted(&format!("{change} src/mine && git restore -- a.py"));
        permitted(&format!("{change} 'src/mine' && git clean -fd"));
        permitted(&format!("{change} src && git -C mine restore -- a.py"));
        refused(&format!(
            "{change} src/mine && git restore -- ../theirs/a.py"
        ));
    }
    permitted("cd src; cd mine; git restore -- a.py");
    permitted("(cd src/mine && git restore -- a.py); git restore -- notes.md");
    refused("(cd src/mine); git restore -- a.py");
    refused("cd src/mine | git restore -- a.py");
    refused("cd src/mine & git restore -- a.py");
    refused("false && cd src/mine; git restore -- a.py");
    refused("true || cd /ws/src/mine; git restore -- a.py");
    refused("true || cd /ws/src/mine && git restore -- a.py");
    permitted("true && cd src/mine && git restore -- a.py");
}

#[test]
fn timed_directory_changes_move_the_parent_shell() {
    let owned = vec![PathBuf::from("/ws/src/mine")];
    for prefix in ["time", "time -p"] {
        let outside = format!("{prefix} cd ../theirs; git restore -- file");
        assert!(
            refusal(&outside, Path::new("/ws/src/mine"), &owned).is_some(),
            "{outside}"
        );
        let inside = format!("{prefix} cd src/mine; git restore -- file");
        assert!(
            refusal(&inside, Path::new(CWD), &owned).is_none(),
            "{inside}"
        );
    }
}

#[test]
fn coprocess_directory_changes_do_not_move_the_parent_shell() {
    let owned = vec![PathBuf::from("/ws/src/mine")];
    for prefix in ["coproc", "coproc WORKER"] {
        let outside = format!("{prefix} cd src/mine; git restore -- file");
        assert!(
            refusal(&outside, Path::new(CWD), &owned).is_some(),
            "{outside}"
        );
        let inside = format!("{prefix} cd ../theirs; git restore -- file");
        assert!(
            refusal(&inside, Path::new("/ws/src/mine"), &owned).is_none(),
            "{inside}"
        );
    }
}

#[test]
fn pipelines_and_background_jobs_preserve_parent_directory() {
    let roots = vec![PathBuf::from("/ws/mine")];
    for command in [
        "git status | head && git restore -- mine/file",
        "cd mine; git status | head; git restore -- file",
        "ls | wc -l && git restore -- mine/file",
        "make build & git restore -- mine/file",
        "git status | head\ngit restore -- mine/file",
        "cd theirs | true; git restore -- mine/file",
        "cd theirs | cd elsewhere | true; git restore -- mine/file",
        "true | cd theirs; git restore -- mine/file",
        "cd $DIR | true; git restore -- mine/file",
        "cd theirs & git restore -- mine/file",
        "cd $DIR & git restore -- mine/file",
        "cd theirs && true & git restore -- mine/file",
        "true || cd theirs & git restore -- mine/file",
        "cd mine; cd ../theirs && true & git restore -- file",
        "(cd theirs) | true; git restore -- mine/file",
        "(cd theirs | true); git restore -- mine/file",
        "cd mine && true | cd ../theirs && git restore -- file",
    ] {
        let reason = refusal(command, Path::new(CWD), &roots);
        assert!(reason.is_none(), "`{command}` should run: {reason:?}");
    }
    for command in [
        "(cd theirs && git restore -- mine/file)",
        "cd mine | git restore -- file",
        "true | (cd theirs && git restore -- mine/file)",
        "cd theirs && git restore -- mine/file &",
        "false && cd mine | true; git restore -- file",
        "true || cd mine | true; git restore -- file",
    ] {
        assert!(
            refusal(command, Path::new(CWD), &roots).is_some(),
            "{command}"
        );
    }
}

#[test]
fn or_lists_preserve_unchanged_directories() {
    let roots = vec![PathBuf::from("/ws/mine")];
    for command in [
        "mkdir -p mine/out || exit 1\ngit restore -- mine/file",
        "cd mine || exit 1; git restore -- file",
        "cargo build || git restore -- mine/src/lib.rs",
        "git fetch || true\ncd mine\ngit restore -- file",
        "git status || true; git stash push -- mine/file",
        "set -e\ngit status || true\ngit clean -fd mine",
        "git restore -- mine/file || git restore -- mine/other",
    ] {
        let reason = refusal(command, Path::new(CWD), &roots);
        assert!(reason.is_none(), "`{command}` should run: {reason:?}");
    }
    for command in [
        "true || cd /ws/src/mine; git restore -- a.py",
        "true || cd mine | true; git restore -- file",
        "cargo test || git checkout -- .",
        "cd theirs || true; git restore -- mine/file",
    ] {
        assert!(
            refusal(command, Path::new(CWD), &roots).is_some(),
            "{command}"
        );
    }
}

#[test]
fn only_a_literal_terminating_exit_resolves_the_left_branch() {
    for command in [
        "cd src/mine || exit; git restore -- file",
        "true && cd src/mine || exit 1; git restore -- file",
        "cd src/mine || exit 0\ngit restore -- file",
    ] {
        permitted(command);
    }
    for command in [
        "cd src/mine || exit 1 | true; git restore -- file",
        "cd src/mine || exit 1 & git restore -- file",
        "cd src/mine || exit 1 && true; git restore -- file",
        "cd src/mine || exit 1 2; git restore -- file",
        "cd src/mine || exit $STATUS; git restore -- file",
        "cd theirs || exit 1; git restore -- src/mine/file",
        "true || cd src/mine || exit 1; git restore -- file",
    ] {
        refused(command);
    }
}

#[test]
fn compound_directory_changes_cannot_hide_outside_writes() {
    let roots = vec![PathBuf::from("/ws/mine")];
    for command in [
        "{ cd theirs; git restore -- mine/file; }",
        "{ cd theirs; }; git restore -- mine/file",
        "if true; then cd theirs; fi; git restore -- mine/file",
        "for d in a; do cd theirs; done; git restore -- mine/file",
        "while read x; do cd theirs; done; git restore -- mine/file",
        "if cd theirs; then true; fi; git restore -- mine/file",
        "if false; then cd mine; fi; git restore -- file",
        "for d in; do cd mine; done; git restore -- file",
        "{ command cd theirs; }; git restore -- mine/file",
        "! cd theirs; git restore -- mine/file",
        "f() { cd mine; }; git clean -fd .",
        "run_tests() { cd mine && cargo test; }; git restore -- file",
        "{ cd mine; } & git restore -- file",
        "{ cd mine; } | true; git restore -- file",
    ] {
        assert!(
            refusal(command, Path::new(CWD), &roots).is_some(),
            "{command}"
        );
    }
    assert!(refused("{ cd src/mine; }; git restore -- file").contains("unknown"));
    permitted("if true; then cd $DIR; fi; git status");
    permitted("echo cd theirs; git restore -- src/mine/file");
    permitted("cd src/mine; cd then; git restore -- file");
}

#[test]
fn substitution_directories_remain_unknown_after_or_lists() {
    for command in [
        "echo $(git restore -- src/mine/file)",
        "git status || true; echo $(git restore -- src/mine/file)",
        "echo $(cd $DIR && git restore -- src/mine/file)",
    ] {
        let reason = refused(command);
        assert!(reason.contains("working directory is unknown"), "{reason}");
    }
    permitted("echo $(git status); git restore -- src/mine/file");
    permitted("echo $(cd /ws/src/mine && git restore -- file)");
}

#[test]
fn dynamic_directory_changes_fail_closed_only_for_guarded_writes() {
    for target in [
        "$DIR",
        "\"$DIR\"",
        "$(pwd)",
        "\"$(pwd)\"",
        "`pwd`",
        "~",
        "src/*",
    ] {
        for change in ["cd", "pushd"] {
            let reason = refused(&format!(
                "{change} {target} && git restore -- src/mine/a.py"
            ));
            assert!(reason.contains("working directory is unknown"), "{reason}");
            permitted(&format!("{change} {target} && git status"));
        }
        refused(&format!("git -C {target} restore -- src/mine/a.py"));
    }
    permitted("cd '$DIR' && git restore -- ../notes.md");
    permitted("cd src/mine && git restore -- a.py");
    permitted("cd $DIR && cd /ws/src/mine && git restore -- a.py");
    permitted("cd $DIR && git clean -n");
}

#[test]
fn ownership_is_resolved_once_before_directory_changes() {
    let roots = vec![PathBuf::from("src/mine")];
    assert!(refusal("git -C src/mine restore -- a.py", Path::new(CWD), &roots).is_none());
    assert!(refusal("git -C src restore -- theirs/a.py", Path::new(CWD), &roots).is_some());
    assert!(refusal("cd src/mine && git restore -- a.py", Path::new(CWD), &roots).is_none());
    let roots = vec![PathBuf::from("relative/mine")];
    let cwd = Path::new("relative");
    assert!(refusal("git restore -- mine/file", cwd, &roots).is_none());
    assert!(refusal("git restore -- theirs/file", cwd, &roots).is_some());
    assert!(refusal("git -C mine restore -- file", cwd, &roots).is_none());
    assert!(refusal("cd mine && git restore -- file", cwd, &roots).is_none());
    assert!(refusal("git -C theirs restore -- mine/file", cwd, &roots).is_some());
}
