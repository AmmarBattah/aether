use super::*;
use crate::git_guard::invocation::GIT_EXIT_FLAGS;

#[test]
fn a_command_that_rewrites_the_whole_tree_is_refused() {
    for command in [
        "git stash",
        "git stash -u",
        "git stash push",
        "git stash push -m 'wip'",
        "git stash save wip",
        "git stash pop",
        "git stash apply",
        "git stash drop",
        "git stash clear",
        "git checkout -- .",
        "git checkout HEAD -- .",
        "git checkout .",
        "git checkout main -- ../elsewhere",
        "git restore .",
        "git restore --staged src/theirs.py",
        "git restore --source=HEAD -- src",
        "git reset --hard",
        "git reset --hard HEAD~1",
        "git clean -fd",
        "git clean -fdx src/theirs",
        "/usr/bin/git stash",
        "GIT_EDITOR=true git stash",
        "git -C /ws/src stash",
    ] {
        let reason = refused(command);
        assert!(
            reason.contains("/ws/src/mine") && reason.contains("git status"),
            "`{command}` must name the boundary and a read-only way out: {reason}"
        );
    }
}

#[test]
fn reading_and_path_scoped_forms_are_permitted() {
    for command in [
        "git stash list",
        "git stash show -p",
        "git diff",
        "git diff --stat && git log --oneline -5",
        "git status",
        "git log",
        "git clean -n",
        "git clean --dry-run -d",
        "git commit -am wip",
        "git checkout main",
        "git checkout -b feature/x",
        "git checkout origin/main",
        "git stash push -- src/mine/a.py",
        "git stash push -m 'wip' -- src/mine",
        "git checkout -- src/mine/a.py",
        "git checkout HEAD -- src/mine/a.py notes.md",
        "git restore src/mine/a.py",
        "git restore --staged -- notes.md",
        "git clean -fd src/mine",
        "cargo test && git diff",
        "echo 'git stash is dangerous'",
    ] {
        permitted(command);
    }
}

#[test]
fn a_path_outside_the_boundary_is_named_in_the_refusal() {
    let reason = refused("git checkout -- src/theirs/a.py");
    assert!(reason.contains("src/theirs/a.py"), "{reason}");
    assert!(reason.contains("/ws/notes.md"), "{reason}");

    // An escape through the boundary's own parent is still outside.
    let reason = refused("git restore -- src/mine/../theirs");
    assert!(reason.contains("outside"), "{reason}");

    // One bad path in an otherwise fine list still refuses.
    refused("git checkout -- src/mine/a.py src/theirs/b.py");

    // A magic pathspec is refused rather than interpreted.
    refused("git restore -- :(exclude)src/mine");
}

#[test]
fn an_agent_that_owns_nothing_is_unaffected() {
    for command in [
        "git stash",
        "git reset --hard",
        "git clean -fd",
        "git checkout -- .",
    ] {
        assert!(refusal(command, Path::new(CWD), &[]).is_none(), "{command}");
    }
}

#[test]
fn stash_save_messages_are_never_pathspecs() {
    assert!(refused("git stash save src/mine").contains("whole repository"));
    refused("git stash save -- src/mine");
    permitted("git stash push -- src/mine");
    permitted("git stash push -m src/mine -- notes.md");
    permitted("git stash list");
}

#[test]
fn clean_exclusions_do_not_turn_into_dry_run_options() {
    for option in [
        "-enode_modules",
        "-e node_modules",
        "--exclude=node_modules",
        "--exclude node_modules",
        "-fde node_modules",
        "-fdenode_modules",
    ] {
        refused(&format!("git clean -fd {option}"));
        permitted(&format!("git clean -fd {option} -- src/mine"));
        permitted(&format!("git clean -n {option}"));
    }
    refused("git clean -fd -- -n");
    refused("git clean -fd -- --dry-run");
    refused("git clean -fd -e -n");
    refused("git clean -fen");
    permitted("git clean -fdn -enode_modules");
    permitted("git clean -fne node_modules");
    permitted("git clean -fd -enode_modules --dry-run");
}

#[test]
fn global_option_values_are_not_subcommands() {
    for option in [
        "-c core.editor=true",
        "-ccore.editor=true",
        "--config-env core.editor=EDITOR",
        "--config-env=core.editor=EDITOR",
        "--git-dir .git",
        "--git-dir=.git",
        "--namespace mine",
        "--namespace=mine",
        "--super-prefix prefix",
        "--super-prefix=prefix",
        "--attr-source HEAD",
        "--attr-source=HEAD",
        "--exec-path=/usr/lib/git",
    ] {
        refused(&format!("git {option} stash"));
        permitted(&format!("git {option} restore -- src/mine/a.py"));
    }
    for option in [
        "-p",
        "-P",
        "--paginate",
        "--no-pager",
        "--bare",
        "--no-replace-objects",
        "--no-lazy-fetch",
        "--no-optional-locks",
        "--no-advice",
        "--literal-pathspecs",
        "--glob-pathspecs",
        "--noglob-pathspecs",
        "--icase-pathspecs",
    ] {
        refused(&format!("git {option} stash"));
        permitted(&format!("git {option} status"));
    }
    for option in GIT_EXIT_FLAGS {
        permitted(&format!("git {option} stash"));
    }
    permitted("git --list-cmds=main stash");
}

#[test]
fn work_tree_relocation_refuses_writes_but_not_reads() {
    for prefix in [
        "git --work-tree=theirs",
        "git --work-tree theirs",
        "GIT_WORK_TREE=theirs git",
    ] {
        assert!(refused(&format!("{prefix} restore -- src/mine/a.py")).contains("relocation"));
        refused(&format!("{prefix} stash push -- src/mine"));
        permitted(&format!("{prefix} status"));
        permitted(&format!("{prefix} stash list"));
        permitted(&format!("{prefix} clean -n"));
    }
    permitted("GIT_EDITOR=true git restore -- src/mine/a.py");
}

#[test]
fn help_shaped_option_values_do_not_hide_writes() {
    for (action, option) in [
        ("stash push", "-m"),
        ("stash push", "--message"),
        ("stash push", "-um"),
        ("clean -fd", "-e"),
        ("clean", "-fde"),
        ("clean -fd", "--exclude"),
        ("restore", "-s"),
        ("restore", "--source"),
    ] {
        for help in ["-h", "--help"] {
            refused(&format!("git {action} {option} {help} theirs/file"));
            permitted(&format!("git {action} {option} {help} -- src/mine/file"));
            permitted(&format!("git {action} {option} value {help} theirs/file"));
        }
    }
    for action in ["stash push", "checkout", "restore"] {
        refused(&format!("git {action} --pathspec-from-file -h"));
    }
    permitted("git stash push -m --pathspec-from-file -- src/mine/file");
}

#[test]
fn help_before_the_path_separator_is_read_only() {
    for action in ["stash", "restore", "clean", "checkout", "reset", "switch"] {
        for help in ["--help", "-h"] {
            permitted(&format!("git {action} {help}"));
            permitted(&format!("git {action} src/theirs {help}"));
            permitted(&format!("git --work-tree=theirs {action} {help}"));
        }
    }
    permitted("git stash push --help");
    permitted("git reset --hard -h");
    permitted("git checkout --pathspec-from-file=paths --help");
    refused("git restore -- --help");
    refused("git clean -fd -- -h");
    refused("git stash push -- --help");
}

#[test]
fn stash_create_and_store_do_not_rewrite_working_files() {
    for action in [
        "create",
        "create snapshot",
        "store abc123",
        "store -m snapshot abc123",
    ] {
        permitted(&format!("git stash {action}"));
        permitted(&format!("git --work-tree=theirs stash {action}"));
    }
    refused("git stash push -- create");
    refused("git stash save create");
}

#[test]
fn pathspec_files_cannot_bypass_owned_paths() {
    for action in ["checkout", "restore", "stash push"] {
        for option in [
            "--pathspec-from-file=src/mine/paths",
            "--pathspec-from-file src/mine/paths",
        ] {
            let reason = refused(&format!("git {action} {option}"));
            assert!(reason.contains("pathspecs read from a file"), "{reason}");
            refused(&format!("git {action} {option} -- src/mine/file"));
        }
    }
    permitted("cd src/mine; git checkout -- --pathspec-from-file=literal");
}

#[test]
fn dynamic_pathspecs_fail_closed_only_for_guarded_writes() {
    for path in [
        "src/mine/$FILE",
        "\"src/mine/${FILE}\"",
        "\"src/mine/$(printf file)\"",
        "$PATHS",
        "src/mine/*",
    ] {
        for action in ["restore", "checkout", "stash push", "clean -fd"] {
            let reason = refused(&format!("git {action} -- {path}"));
            assert!(reason.contains("non-literal pathspec"), "{reason}");
            permitted(&format!("git {action} --help -- {path}"));
        }
        permitted(&format!("git diff -- {path}"));
        permitted(&format!("git clean -n -- {path}"));
    }
    permitted("git restore -- 'src/mine/$FILE'");
    permitted("git restore -- src/mine/\\$FILE");
    permitted("git restore -- 'src/mine/*'");
    permitted("git stash push -m \"$MESSAGE\" -- src/mine/file");
    permitted("git stash push -m src/mine/$FILE -- 'src/mine/$FILE'");
    permitted("git restore --source \"$REV\" -- src/mine/file");
    permitted("git clean -fd -e \"$EXCLUDE\" -- src/mine/file");
    permitted("git checkout \"$BRANCH\"");
    permitted("git switch \"$BRANCH\"");
}

#[test]
fn additional_whole_tree_forms_are_guarded() {
    for command in [
        "git reset --merge",
        "git reset --keep HEAD~1",
        "git checkout -f",
        "git checkout --force main",
        "git switch --discard-changes main",
    ] {
        let reason = refused(command);
        assert!(reason.contains("whole repository"), "{reason}");
        permitted(&format!("{command} --help"));
        assert!(refusal(command, Path::new(CWD), &[]).is_none(), "{command}");
    }
    permitted("git reset --soft HEAD~1");
    permitted("git reset --mixed HEAD~1");
    permitted("git switch main");
    permitted("cd src/mine; git checkout -- --force");
    permitted("cd src/mine; git restore -- --merge");
}
