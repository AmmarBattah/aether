//! A guard over git commands run by Aether's built-in shell.
//! Engine-native Bash (claude-code/codex using their own shell tool) never
//! reaches this guard.
//!
//! `allowed_paths` bounds the file tools, but a shell call can rewrite any
//! file in the workspace. The expensive case in practice is not malice: an
//! agent sharing a worktree with a sibling runs `git stash` or
//! `git checkout -- .` to isolate its own work and reverts the sibling's
//! uncommitted edits.
//!
//! So when a scope names `allowed_paths`, the git invocations that throw
//! working-tree changes away are refused unless they name paths inside that
//! boundary. Reading is never refused: `git status`, `git diff`, `git log`,
//! `git stash list`, `git stash create`/`store`, `git clean --dry-run`, and
//! `--help`/`-h` pass through. Guarded forms are
//! stash mutations, checkout pathspecs or `-f`/`--force`, restore, clean writes,
//! reset `--hard`/`--merge`/`--keep`, and switch `--discard-changes`.
//! Leading shell keywords are recognized, but directory changes prefixed by
//! braces or conditional keywords make the working directory unknown.
//!
//! This is a guardrail against a plausible accident, not a sandbox - a shell
//! can still overwrite any file through any other command. It is a bounded
//! tokenizer, so these forms are knowingly not caught: a git invocation the
//! shell builds at runtime (`$CMD stash`, `eval`, `bash -c "git stash"`, a
//! script, an alias, `env -S`), input typed into a `tty` session with
//! `write_stdin` (only the command that starts the session is checked),
//! `git checkout <name>` where the argument is neither
//! `--`-separated nor visibly a path, and a branch switch, which rewrites the
//! tree but is indistinguishable here from ordinary navigation. Also not
//! interpreted: symlink canonicalization, heredocs, `case` syntax,
//! `git merge/rebase --abort`, `git apply -R`, `git rm -rf`,
//! `git checkout-index -f -a`, `git worktree remove -f`,
//! `git submodule update --force`, and compound-statement shapes the tokenizer
//! does not model. Pathspec magic such as `:(exclude)src` is refused rather
//! than interpreted.

use std::path::{Path, PathBuf};

mod directory;
mod invocation;
mod lexer;
mod validation;

use directory::{DirectoryContext, DirectoryState, change_directory, exits_list, update_cwd};
use invocation::git_call;
use lexer::{Boundary, Segment, segments};
use validation::check;

/// An empty boundary leaves commands unguarded. Resolve ownership once, before
/// any shell `cd` or Git `-C` changes the command's working directory.
pub(crate) fn refusal(command: &str, cwd: &Path, allowed: &[PathBuf]) -> Option<String> {
    if allowed.is_empty() {
        return None;
    }
    let base = if cwd.is_absolute() {
        cwd.to_path_buf()
    } else {
        match std::env::current_dir() {
            Ok(base) => base,
            Err(error) => return Some(format!("Cannot resolve shell working directory: {error}")),
        }
    };
    let allowed: Vec<_> = allowed.iter().map(|p| absolute(p, &base)).collect();
    let mut state = DirectoryState::new(DirectoryContext {
        cwd: Some(absolute(cwd, &base)),
        ..DirectoryContext::default()
    });
    let mut parents = Vec::new();
    let segments = segments(command);
    for (index, segment) in segments.iter().enumerate() {
        match segment {
            Segment::Boundary(Boundary::Ambiguous) if exits_list(&segments[index + 1..]) => {
                // Continuing past `left || exit` requires the left side to succeed.
                state.finish_pipeline();
                state.current.conditional_base = None;
            }
            Segment::Boundary(boundary) => state.cross(*boundary, &mut parents),
            Segment::Command(words) => {
                state.command_base = state.current.clone();
                update_cwd(words, &mut state.current.cwd);
                if state.current.ambiguous && state.current.cwd != state.command_base.cwd {
                    state.current.cwd = None;
                }
                if let Some(call) = git_call(words) {
                    let mut git_cwd = state.current.cwd.clone();
                    for dir in &call.dirs {
                        change_directory(dir, &mut git_cwd);
                    }
                    if let Some(reason) = check(&call, git_cwd.as_deref(), &allowed) {
                        return Some(reason);
                    }
                }
            }
        }
    }
    None
}

fn absolute(path: &Path, cwd: &Path) -> PathBuf {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };
    let resolved = crate::fs::normalize_lexically(&joined);
    debug_assert!(
        resolved.is_absolute() || !cwd.is_absolute(),
        "an absolute cwd must yield an absolute path: {} in {}",
        path.display(),
        cwd.display()
    );
    resolved
}

#[cfg(test)]
mod tests;
