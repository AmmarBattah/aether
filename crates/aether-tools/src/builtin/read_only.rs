//! Which shell commands are plainly read-only, so they need no exclusive
//! lease on the workspace.
//!
//! Deliberately conservative and syntactic: anything the classifier does not
//! fully recognise is treated as mutating, which costs concurrency and never
//! correctness.

/// Commands that only read. A bare name, never a path.
const READ_ONLY: &[&str] = &[
    "basename",
    "cat",
    "cksum",
    "cmp",
    "cut",
    "date",
    "df",
    "diff",
    "dirname",
    "du",
    "echo",
    "egrep",
    "false",
    "fgrep",
    "file",
    "find",
    "grep",
    "head",
    "hostname",
    "jq",
    "ls",
    "md5sum",
    "nl",
    "printf",
    "pwd",
    "readlink",
    "realpath",
    "rg",
    "sha256sum",
    "stat",
    "tail",
    "true",
    "uname",
    "wc",
    "which",
    "whoami",
];

/// Git subcommands that only report. `git` reaches everything, so the
/// subcommand must be the first word: `git -C dir status` is not recognised.
const GIT_READ_ONLY: &[&str] = &[
    "blame",
    "cat-file",
    "describe",
    "diff",
    "log",
    "ls-files",
    "rev-parse",
    "shortlog",
    "show",
    "status",
];

/// Flags that make an otherwise read-only command write.
const WRITING_FLAGS: &[&str] = &[
    "--output", "-delete", "-exec", "-execdir", "-fls", "-fprint", "-fprint0", "-fprintf", "-ok",
    "-okdir",
];

/// Whether a shell command clearly only reads. `false` means "not proven
/// read-only", not "writes".
pub(crate) fn is_read_only_command(command: &str) -> bool {
    let command = command.trim();
    if command.is_empty() {
        return false;
    }
    // Redirection, chaining and substitution can run or write anything; only
    // a plain pipeline is classified.
    if command.contains(['>', '<', ';', '&', '`', '\n', '\r']) || command.contains("$(") {
        return false;
    }
    command.split('|').all(reads_only)
}

fn reads_only(segment: &str) -> bool {
    let mut words = segment.split_whitespace();
    let Some(command) = words.next() else {
        return false;
    };
    let args: Vec<&str> = words.collect();
    if args.iter().any(|a| {
        WRITING_FLAGS
            .iter()
            .any(|f| *a == *f || a.starts_with(&format!("{f}=")))
    }) {
        return false;
    }
    match command {
        "git" => args.first().is_some_and(|sub| GIT_READ_ONLY.contains(sub)),
        other => READ_ONLY.contains(&other),
    }
}

#[cfg(test)]
mod tests {
    use super::is_read_only_command;

    #[test]
    fn plain_readers_are_recognised() {
        for command in [
            "ls -la",
            "cat src/main.rs",
            "grep -rn todo crates",
            "rg --hidden pattern",
            "find . -name '*.rs'",
            "git status",
            "git log --format=%s main..HEAD | grep fix",
            "wc -l Cargo.toml",
        ] {
            assert!(is_read_only_command(command), "{command}");
        }
    }

    #[test]
    fn anything_that_could_write_is_not_read_only() {
        for command in [
            "",
            "cargo test",
            "sort input",
            "rm -rf target",
            "cat a > b",
            "ls; rm -rf .",
            "ls && rm -rf .",
            "echo $(rm -rf .)",
            "echo `rm -rf .`",
            "cat a < b",
            "find . -delete",
            "find . -exec rm {} +",
            "grep x . --output=out",
            "git commit -m x",
            "git push",
            "git -C /elsewhere status",
            "sed -i s/a/b/ f",
            "awk '{print > \"f\"}' f",
            "xargs rm",
            "env FOO=1 rm -rf .",
            "/bin/ls",
            "ls | tee out",
            "sort -o out input",
            "sort -oout input",
            "sort -roout input",
            "sort '-o' out input",
            r"sort \-oout input",
            "sort --out=out input",
            "sort --o out input",
            "tree -o out",
            "uniq input output",
        ] {
            assert!(!is_read_only_command(command), "{command}");
        }
    }
}
