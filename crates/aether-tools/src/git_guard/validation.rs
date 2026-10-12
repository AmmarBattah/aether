use super::{
    absolute,
    invocation::{GitCall, consumes_next_word},
};
use std::path::{Path, PathBuf};

/// `save` takes a message, not pathspecs; the others act on whole stash entries.
const WHOLE_TREE_STASH: &[&str] = &["save", "pop", "apply", "drop", "clear", "branch"];

/// The refusal this call earns, if any. Only the subcommands that discard
/// working-tree state are considered; everything else is someone else's
/// problem (the file tools, the approval gate) and passes through.
pub(super) fn check(call: &GitCall<'_>, cwd: Option<&Path>, allowed: &[PathBuf]) -> Option<String> {
    if !guarded(call) {
        return None;
    }
    let what = format!("git {}", call.subcommand);
    if call.relocated {
        return Some(format!(
            "Cannot verify `{what}` with --work-tree/GIT_WORK_TREE or wrapper relocation. \
            Use an ordinary working directory and name owned paths. {ADVICE}"
        ));
    }
    if whole_tree_form(call) {
        return Some(whole_tree_message(&what, allowed));
    }
    if has_option(call, &["--pathspec-from-file"]) {
        return Some(format!(
            "Cannot verify `{what}` pathspecs read from a file. {ADVICE}"
        ));
    }
    if has_dynamic_path(call) {
        return Some(format!(
            "Cannot verify `{what}` with a non-literal pathspec. {ADVICE}"
        ));
    }
    let args = call.args.as_slice();
    match call.subcommand {
        "stash" => check_stash(args, cwd, allowed),
        "checkout" => verdict(&what, &checkout_paths(args)?, cwd, allowed),
        "restore" => verdict(&what, &path_args(args, &["-s", "--source"]), cwd, allowed),
        "clean" => {
            // Clean without paths deletes only below cwd, not across the repository.
            let named = path_args(args, &["-e", "--exclude"]);
            let paths = if named.is_empty() { vec!["."] } else { named };
            verdict(&what, &paths, cwd, allowed)
        }
        _ => None,
    }
}

fn value_flags(subcommand: &str) -> &'static [&'static str] {
    match subcommand {
        "stash" => &["-m", "--message", "--pathspec-from-file"],
        "restore" => &["-s", "--source", "--pathspec-from-file"],
        "clean" => &["-e", "--exclude"],
        "checkout" => &["--pathspec-from-file"],
        _ => &[],
    }
}

fn has_dynamic_path(call: &GitCall<'_>) -> bool {
    let paths = path_indices(&call.args, value_flags(call.subcommand));
    call.dynamic_args.iter().any(|index| paths.contains(index))
}

fn has_option(call: &GitCall<'_>, options: &[&str]) -> bool {
    let mut rest = call.args.iter().copied();
    while let Some(arg) = rest.next() {
        if arg == "--" {
            break;
        }
        if options.contains(&arg.split('=').next().unwrap_or(arg)) {
            return true;
        }
        if consumes_next_word(arg, value_flags(call.subcommand)) {
            rest.next();
        }
    }
    false
}

fn whole_tree_form(call: &GitCall<'_>) -> bool {
    let options: &[&str] = match call.subcommand {
        "reset" => &["--hard", "--merge", "--keep"],
        "checkout" => &["-f", "--force"],
        "switch" => &["--discard-changes"],
        _ => return false,
    };
    has_option(call, options)
}

fn check_stash(args: &[&str], cwd: Option<&Path>, allowed: &[PathBuf]) -> Option<String> {
    let message_flags: &[&str] = &["-m", "--message"];
    let action = first_operand(args, message_flags);
    match action {
        Some(("list" | "show" | "create" | "store", _)) => None,
        Some((word, _)) if WHOLE_TREE_STASH.contains(&word) => {
            Some(whole_tree_message(&format!("git stash {word}"), allowed))
        }
        // Only `push` (including the implicit action) accepts pathspecs.
        Some(("push", tail)) => verdict("git stash", &path_args(tail, message_flags), cwd, allowed),
        _ => verdict("git stash", &path_args(args, message_flags), cwd, allowed),
    }
}

/// The pathspecs a `checkout` names, or `None` when it names none this
/// tokenizer can be sure of - a bare `git checkout <name>` is far more often
/// a branch than a file, and refusing those would be noise.
fn checkout_paths<'a>(args: &[&'a str]) -> Option<Vec<&'a str>> {
    if let Some(at) = args.iter().position(|a| *a == "--") {
        let paths: Vec<&str> = args[at + 1..].to_vec();
        return (!paths.is_empty()).then_some(paths);
    }
    let paths: Vec<&str> = args
        .iter()
        .copied()
        .filter(|a| is_plainly_a_path(a))
        .collect();
    (!paths.is_empty()).then_some(paths)
}

/// An argument no branch name would be: refs cannot be `.` or `..`, cannot
/// start with `/` or `./`, and cannot end with `/`. A branch with a slash in
/// the middle (`origin/main`, `feature/x`) stays a ref.
fn is_plainly_a_path(arg: &str) -> bool {
    arg == "."
        || arg == ".."
        || arg.starts_with(['/', ':'])
        || arg.starts_with("./")
        || arg.starts_with("../")
        || arg.ends_with('/')
}

/// The operands of a subcommand: everything that is not a flag or a flag's
/// value. After `--` every word is an operand, flag-shaped or not.
fn path_args<'a>(args: &[&'a str], value_flags: &[&str]) -> Vec<&'a str> {
    path_indices(args, value_flags)
        .into_iter()
        .map(|index| args[index])
        .collect()
}

fn path_indices(args: &[&str], value_flags: &[&str]) -> Vec<usize> {
    let mut paths = Vec::new();
    let mut rest = args.iter().copied().enumerate();
    while let Some((index, arg)) = rest.next() {
        if arg == "--" {
            paths.extend(rest.map(|(index, _)| index));
            break;
        }
        if arg.starts_with('-') {
            if consumes_next_word(arg, value_flags) {
                rest.next();
            }
            continue;
        }
        paths.push(index);
    }
    paths
}

/// The first operand and what follows it, skipping flags and their values.
fn first_operand<'a, 'b>(
    args: &'b [&'a str],
    value_flags: &[&str],
) -> Option<(&'a str, &'b [&'a str])> {
    let mut i = 0;
    while i < args.len() {
        let arg = args[i];
        if arg == "--" {
            return None;
        }
        if arg.starts_with('-') {
            i += if consumes_next_word(arg, value_flags) {
                2
            } else {
                1
            };
            continue;
        }
        return Some((arg, &args[i + 1..]));
    }
    None
}

fn guarded(call: &GitCall<'_>) -> bool {
    if has_option(call, &["--help", "-h"]) {
        return false;
    }
    if whole_tree_form(call) {
        return true;
    }
    match call.subcommand {
        "stash" => !matches!(
            first_operand(&call.args, &["-m", "--message"]),
            Some(("list" | "show" | "create" | "store", _))
        ),
        "checkout" => {
            checkout_paths(&call.args).is_some() || has_option(call, &["--pathspec-from-file"])
        }
        "restore" => true,
        "clean" => !is_dry_run(&call.args),
        _ => false,
    }
}

fn is_dry_run(args: &[&str]) -> bool {
    let mut rest = args.iter().copied();
    while let Some(arg) = rest.next() {
        match arg {
            "--" => break,
            "--dry-run" => return true,
            "--exclude" => {
                rest.next();
            }
            _ if arg.starts_with("--") => {}
            _ if arg.starts_with('-') => {
                let mut options = arg[1..].chars();
                while let Some(option) = options.next() {
                    match option {
                        'n' => return true,
                        'e' => {
                            if options.as_str().is_empty() {
                                rest.next();
                            }
                            break;
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    false
}

/// Refuse a command that names no path inside the boundary.
fn verdict(what: &str, paths: &[&str], cwd: Option<&Path>, allowed: &[PathBuf]) -> Option<String> {
    if paths.is_empty() {
        return Some(whole_tree_message(what, allowed));
    }
    let Some(cwd) = cwd else {
        return Some(format!(
            "Cannot verify `{what}` because the working directory is unknown \
            after a non-literal cd/pushd or -C target, or ambiguous shell control flow. Use a literal directory. {ADVICE}"
        ));
    };
    let outside = paths.iter().find(|spec| !inside(spec, cwd, allowed))?;
    // A relative pathspec means little without the directory it is read from.
    let resolved = match pathspec(outside, cwd) {
        Some(path) => format!(" ({})", path.display()),
        None => String::new(),
    };
    Some(format!(
        "`{what}` names `{outside}`{resolved}, which is outside the paths this agent owns ({}). \
         Files there may hold another agent's uncommitted work. {ADVICE}",
        boundary(allowed)
    ))
}

fn whole_tree_message(what: &str, allowed: &[PathBuf]) -> String {
    format!(
        "`{what}` would discard working-tree changes across the whole repository, but this \
         agent owns only {}. The rest may hold another agent's uncommitted work. {ADVICE}",
        boundary(allowed)
    )
}

/// The same escape hatch in every refusal: look, or name your own files.
const ADVICE: &str = "Inspect instead with `git status`, `git diff`, `git log` or \
                      `git stash list`, or name your own paths explicitly, as in \
                      `git restore -- <path you own>`.";

fn boundary(allowed: &[PathBuf]) -> String {
    allowed
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Whether a pathspec resolves inside the boundary. One this guard cannot
/// read never counts as inside.
fn inside(spec: &str, cwd: &Path, allowed: &[PathBuf]) -> bool {
    let Some(path) = pathspec(spec, cwd) else {
        return false;
    };
    debug_assert!(allowed.iter().all(|root| root.is_absolute()));
    allowed.iter().any(|root| path.starts_with(root))
}

/// The file a pathspec points at, or `None` for magic this guard does not
/// interpret (`:(exclude)x`, `:/`).
fn pathspec(spec: &str, cwd: &Path) -> Option<PathBuf> {
    if spec.is_empty() || spec.starts_with(':') {
        return None;
    }
    Some(absolute(Path::new(spec), cwd))
}
