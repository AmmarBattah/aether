use super::lexer::Word;

// Value-taking global options from `git help git`; --exec-path only accepts
// an attached value. -C is handled separately because it changes resolution.
const GIT_VALUE_FLAGS: &[&str] = &[
    "-c",
    "--config-env",
    "--git-dir",
    "--work-tree",
    "--namespace",
    "--super-prefix",
    "--attr-source",
];
pub(super) const GIT_EXIT_FLAGS: &[&str] = &[
    "-v",
    "--version",
    "-h",
    "--help",
    "--exec-path",
    "--html-path",
    "--man-path",
    "--info-path",
    "--list-cmds",
];

#[derive(Debug)]
pub(super) struct GitCall<'a> {
    pub(super) dirs: Vec<Word>,
    pub(super) relocated: bool,
    pub(super) subcommand: &'a str,
    pub(super) args: Vec<&'a str>,
    pub(super) dynamic_args: Vec<usize>,
}

pub(super) fn shell_prefix(word: &str) -> bool {
    matches!(
        word,
        "{" | "}"
            | "then"
            | "do"
            | "else"
            | "elif"
            | "!"
            | "fi"
            | "done"
            | ";;"
            | "if"
            | "while"
            | "until"
            | "time"
            | "coproc"
    )
}

/// The git call a segment runs, or `None` when it does not plainly run git.
pub(super) fn git_call(words: &[Word]) -> Option<GitCall<'_>> {
    let mut relocated = false;
    let words = unwrap_program(words, &mut relocated)?;
    let mut rest = words.iter();
    let program = rest.next()?.as_str();
    if program != "git" && !program.ends_with("/git") {
        return None;
    }
    let mut dirs = Vec::new();
    while let Some(word) = rest.next() {
        let arg = word.as_str();
        if !arg.starts_with('-') {
            return Some(GitCall {
                dirs,
                relocated,
                subcommand: arg,
                dynamic_args: rest
                    .clone()
                    .enumerate()
                    .filter_map(|(index, word)| word.dynamic.then_some(index))
                    .collect(),
                args: rest.map(Word::as_str).collect(),
            });
        }
        let flag = arg.split('=').next()?;
        if GIT_EXIT_FLAGS.contains(&arg) || flag == "--list-cmds" {
            return None;
        }
        relocated |= flag == "--work-tree";
        if arg == "-C" {
            let dir = rest.next()?;
            dirs.push(Word {
                text: dir.text.clone(),
                started: true,
                dynamic: dir.dynamic,
                quoted: dir.quoted,
            });
        } else if let Some(dir) = arg.strip_prefix("-C") {
            dirs.push(Word {
                text: dir.to_owned(),
                started: true,
                dynamic: word.dynamic,
                quoted: word.quoted,
            });
        } else if GIT_VALUE_FLAGS.contains(&arg) {
            rest.next()?;
        }
    }
    None
}

/// Unwrap only literal launchers; their option values are not program names.
fn unwrap_program<'a>(mut words: &'a [Word], relocated: &mut bool) -> Option<&'a [Word]> {
    let mut prefix_len = 0;
    while words
        .get(prefix_len)
        .is_some_and(|w| !w.quoted && shell_prefix(w.as_str()))
    {
        let prefix = words[prefix_len].as_str();
        prefix_len += 1;
        if prefix == "time" && words.get(prefix_len).is_some_and(|w| w.as_str() == "-p") {
            prefix_len += 1;
        }
        if prefix == "coproc"
            && words.get(prefix_len).is_some_and(|name| {
                !name.quoted
                    && name.as_str() != "git"
                    && is_assignment(&format!("{}=", name.as_str()))
                    && words.get(prefix_len + 1).is_some()
            })
        {
            prefix_len += 1;
        }
    }
    words = &words[prefix_len..];
    loop {
        let (program, tail) = words.split_first()?;
        if is_assignment(program.as_str()) {
            *relocated |= program.text.starts_with("GIT_WORK_TREE=");
            words = tail;
            continue;
        }
        if program.dynamic {
            return None;
        }
        let name = program.text.rsplit('/').next()?;
        if !matches!(
            name,
            "env"
                | "command"
                | "nice"
                | "timeout"
                | "sudo"
                | "nohup"
                | "setsid"
                | "stdbuf"
                | "time"
        ) {
            return Some(words);
        }
        words = wrapper_args(name, tail, relocated)?;
    }
}

fn wrapper_args<'a>(name: &str, mut words: &'a [Word], relocated: &mut bool) -> Option<&'a [Word]> {
    let value_flags = wrapper_value_flags(name);
    while let Some((word, tail)) = words.split_first() {
        let arg = word.as_str();
        if arg == "--" {
            words = tail;
            break;
        }
        if !arg.starts_with('-') {
            break;
        }
        if wrapper_does_not_run_literal(name, arg) {
            return None;
        }
        *relocated |= wrapper_relocates(name, arg);
        words = if consumes_next_word(arg, value_flags) {
            tail.get(1..)?
        } else {
            tail
        };
    }
    if name == "timeout" {
        words = words.get(1..)?; // The duration precedes the program.
    }
    Some(words)
}

fn wrapper_value_flags(name: &str) -> &'static [&'static str] {
    match name {
        "env" => &["-u", "--unset", "-C", "--chdir"],
        "nice" => &["-n", "--adjustment"],
        "timeout" => &["-s", "--signal", "-k", "--kill-after"],
        "sudo" => &[
            "-a",
            "--auth-type",
            "-u",
            "--user",
            "-g",
            "--group",
            "-h",
            "--host",
            "-p",
            "--prompt",
            "-C",
            "--close-from",
            "-T",
            "--command-timeout",
            "-R",
            "--chroot",
            "-D",
            "--chdir",
            "-r",
            "--role",
            "-t",
            "--type",
        ],
        "time" => &["-f", "--format", "-o", "--output"],
        "stdbuf" => &["-i", "-o", "-e", "--input", "--output", "--error"],
        _ => &[],
    }
}

fn wrapper_does_not_run_literal(name: &str, arg: &str) -> bool {
    matches!(arg, "--help" | "--version")
        || (name == "command" && arg.starts_with('-') && arg.contains(['v', 'V']))
        || (name == "env" && (arg.starts_with("-S") || arg.starts_with("--split-string")))
}

fn wrapper_relocates(name: &str, arg: &str) -> bool {
    let relocation_flags: &[&str] = match name {
        "env" => &["-C", "--chdir"],
        "sudo" => &["-D", "-R", "--chdir", "--chroot"],
        _ => return false,
    };
    if arg.starts_with("--") {
        return relocation_flags.contains(&arg.split('=').next().unwrap_or(arg));
    }
    let Some(options) = arg.strip_prefix('-') else {
        return false;
    };
    for option in options.chars() {
        let flag = format!("-{option}");
        if relocation_flags.contains(&flag.as_str()) {
            return true;
        }
        if wrapper_value_flags(name).contains(&flag.as_str()) {
            break;
        }
    }
    false
}

/// `NAME=value` before the program name, which bash treats as environment.
pub(super) fn is_assignment(word: &str) -> bool {
    let Some((name, _)) = word.split_once('=') else {
        return false;
    };
    !name.is_empty()
        && name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// A short value flag ends its cluster: the rest is its value, not options.
pub(super) fn consumes_next_word(arg: &str, value_flags: &[&str]) -> bool {
    if value_flags.contains(&arg) {
        return true;
    }
    if !arg.starts_with('-') || arg.starts_with("--") {
        return false;
    }
    let mut options = arg[1..].chars();
    while let Some(option) = options.next() {
        if value_flags.iter().any(|flag| *flag == format!("-{option}")) {
            return options.as_str().is_empty();
        }
    }
    false
}
