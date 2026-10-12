use super::*;

#[test]
fn every_segment_of_a_compound_command_is_inspected() {
    for command in [
        "git diff --stat && git stash",
        "git diff; git stash pop",
        "cargo test || git checkout -- .",
        "git stash | tee log",
        "true\ngit clean -fd",
        "(cd src && git stash)",
        "echo $(git stash)",
    ] {
        refused(command);
    }
}

#[test]
fn shell_prefixes_do_not_hide_guarded_git_calls() {
    for (command, problem) in [
        (
            "if ! git diff --quiet; then git stash; fi",
            "whole repository",
        ),
        (
            "for f in $(git status --porcelain); do git checkout -- $f; done",
            "non-literal pathspec",
        ),
        (
            "while read f; do git restore -- $f; done < list.txt",
            "non-literal pathspec",
        ),
        ("{ git stash; }", "whole repository"),
        ("! git stash", "whole repository"),
        (
            "if false; then true; else command git stash; fi",
            "whole repository",
        ),
        ("(git stash)", "whole repository"),
    ] {
        let reason = refused(command);
        assert!(reason.contains(problem), "{reason}");
        assert!(reason.contains("git status"), "{reason}");
    }
    for command in [
        "if git diff --quiet; then echo clean; fi",
        "{ git status; }",
        "! command git status",
        "'then' git stash",
        "\"!\" git stash",
        "\\{ git stash",
    ] {
        permitted(command);
    }
}

#[test]
fn quoting_does_not_hide_or_invent_a_command() {
    let owned = owned();
    // A quoted separator is not a segment boundary.
    assert!(refusal("echo 'a; git stash'", Path::new(CWD), &owned).is_none());
    // A quoted path is still the path.
    assert!(refusal("git restore -- 'src/mine/a b.py'", Path::new(CWD), &owned).is_none());
    assert!(refusal("git restore -- 'src/theirs/a.py'", Path::new(CWD), &owned).is_some());
}

#[test]
fn empty_quoted_words_are_arguments_and_empty_c_is_a_noop() {
    for empty in ["\"\"", "''"] {
        refused(&format!("git -C {empty} stash"));
        refused(&format!("git -C {empty} restore -- src/theirs"));
        permitted(&format!("git -C {empty} restore -- src/mine/a.py"));
        permitted(&format!("git -C src/mine -C {empty} restore -- a.py"));
        permitted(&format!("git stash push -m {empty} -- src/mine"));
    }
    refused("git restore -- '' src/mine");
}

#[test]
fn literal_launch_wrappers_are_unwrapped() {
    for wrapper in [
        "env",
        "env -i NAME=value",
        "env --unset NAME",
        "env -uNAME",
        "command",
        "command -p --",
        "nice",
        "nice -n 19",
        "nice --adjustment=19",
        "timeout 10",
        "timeout -s TERM -k 2s 10s",
        "timeout --signal=TERM -- 1m",
        "sudo",
        "sudo -n -u root",
        "sudo --user=root --",
        "sudo -E NAME=value",
        "sudo -nu root",
        "sudo -u RDuser",
        "sudo -uRDuser",
        "/usr/bin/env -i",
        "env NAME=value command nice -n19 timeout 1 sudo -n",
    ] {
        refused(&format!("{wrapper} git stash"));
        refused(&format!("{wrapper} git restore -- src/theirs/file"));
        permitted(&format!("{wrapper} git restore -- src/mine/file"));
        permitted(&format!("{wrapper} git status"));
    }
    permitted("command -v git stash");
    permitted("command -V git restore -- src/theirs/file");
    permitted("nice --help git stash");
    refused("env GIT_WORK_TREE=theirs git restore -- src/mine/file");
    refused("sudo -n GIT_WORK_TREE=theirs git restore -- src/mine/file");
}

#[test]
fn additional_literal_launchers_do_not_hide_guarded_git_calls() {
    for prefix in [
        "time",
        "time -p",
        "/usr/bin/time -f elapsed",
        "nohup",
        "setsid",
        "setsid -f",
        "stdbuf -oL -e0",
        "stdbuf -i 0 -o L -e 0",
        "stdbuf --output L --error 0",
    ] {
        refused(&format!("{prefix} git stash"));
        refused(&format!("{prefix} git restore -- src/theirs/file"));
        permitted(&format!("{prefix} git restore -- src/mine/file"));
        permitted(&format!("{prefix} git status"));
    }
}

#[test]
fn coprocess_commands_are_checked_after_an_optional_name() {
    refused("coproc git stash");
    refused("coproc git restore -- src/theirs/file");
    permitted("coproc git restore -- src/mine/file");
    permitted("coproc git status");
    refused("coproc WORKER git stash");
    refused("coproc WORKER git restore -- src/theirs/file");
    permitted("coproc WORKER git restore -- src/mine/file");
    permitted("coproc WORKER git status");
}

#[test]
fn relocating_wrappers_fail_closed_only_for_writes() {
    for wrapper in [
        "env -C theirs",
        "env -iC theirs",
        "env --chdir=theirs",
        "sudo -D theirs",
        "sudo -nD theirs",
        "sudo --chroot=/elsewhere",
    ] {
        let reason = refused(&format!("{wrapper} git restore -- src/mine/file"));
        assert!(reason.contains("relocation"), "{reason}");
        permitted(&format!("{wrapper} git status"));
        permitted(&format!("{wrapper} git restore --help"));
    }
}

#[test]
fn redirections_are_not_pathspecs() {
    for redirect in [
        "2>/dev/null",
        "> /dev/null",
        ">>/dev/null",
        "< /dev/null",
        "&>/dev/null",
        "2>&1",
        "2> '/dev/null'",
        "2>&1 >/dev/null",
        "&>>/dev/null",
    ] {
        permitted(&format!("git restore -- src/mine/a.py {redirect}"));
        permitted(&format!("git {redirect} restore -- src/mine/a.py"));
        refused(&format!("git restore -- src/theirs/a.py {redirect}"));
    }
    refused("git restore -- src/mine/a.py '2'>/dev/null");
    refused("git restore -- src/mine/a.py 2 > /dev/null");
    permitted("git restore -- 'src/mine/a>b'");
    refused("git restore -- 'src/theirs/a>b'");
}

#[test]
fn word_start_comments_are_not_arguments_or_commands() {
    permitted("git restore -- src/mine/a.py # cleanup");
    refused("git restore -- src/theirs/a.py # cleanup");
    permitted("git restore -- src/mine/a.py # git stash");
    refused("git status # comment\ngit stash");
    permitted("git restore -- src/mine/a#b");
    refused("git restore -- '#not-a-comment' src/mine/a.py");
    refused("git restore -- src/theirs#file");
}
