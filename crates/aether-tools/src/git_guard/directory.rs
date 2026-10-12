use super::{
    absolute,
    invocation::{is_assignment, shell_prefix},
    lexer::{Boundary, Segment, Word},
};
use std::path::{Path, PathBuf};

#[derive(Clone, Default)]
pub(super) struct DirectoryContext {
    pub(super) cwd: Option<PathBuf>,
    pub(super) conditional_base: Option<Option<PathBuf>>,
    pub(super) ambiguous: bool,
}

#[derive(Clone)]
pub(super) struct DirectoryState {
    pub(super) current: DirectoryContext,
    pub(super) command_base: DirectoryContext,
    pub(super) list_base: DirectoryContext,
    pub(super) pipeline_base: Option<DirectoryContext>,
}

impl DirectoryState {
    pub(super) fn new(current: DirectoryContext) -> Self {
        Self {
            command_base: current.clone(),
            list_base: current.clone(),
            current,
            pipeline_base: None,
        }
    }

    pub(super) fn cross(&mut self, boundary: Boundary, parents: &mut Vec<Self>) {
        if !matches!(boundary, Boundary::Pipe | Boundary::Open | Boundary::Close) {
            self.finish_pipeline();
        }
        match boundary {
            Boundary::Open => {
                self.command_base = self.current.clone();
                parents.push(self.clone());
                *self = Self::new(self.current.clone());
            }
            Boundary::Close => {
                *self = parents
                    .pop()
                    .unwrap_or_else(|| Self::new(DirectoryContext::default()));
            }
            Boundary::Pipe => {
                let base = self.pipeline_base.get_or_insert(self.command_base.clone());
                self.current = base.clone();
            }
            Boundary::Background => {
                // An entire asynchronous AND/OR list runs outside the parent shell.
                *self = Self::new(self.list_base.clone());
            }
            Boundary::And => {
                self.current
                    .conditional_base
                    .get_or_insert(self.current.cwd.clone());
            }
            Boundary::Ambiguous => {
                if self.current.cwd != self.list_base.cwd {
                    self.current.cwd = None;
                }
                self.current.conditional_base = Some(self.current.cwd.clone());
                self.current.ambiguous = true;
            }
            Boundary::Substitution => *self = Self::new(DirectoryContext::default()),
            Boundary::Sequence => {
                self.current.ambiguous = false;
                // A cd in a skipped && branch must not relocate a later command.
                if let Some(before) = self.current.conditional_base.take()
                    && before != self.current.cwd
                {
                    self.current.cwd = None;
                }
                self.list_base = self.current.clone();
            }
        }
    }

    pub(super) fn finish_pipeline(&mut self) {
        if let Some(base) = self.pipeline_base.take() {
            self.current = base;
        }
    }
}

pub(super) fn change_directory(dir: &Word, cwd: &mut Option<PathBuf>) {
    if dir.dynamic {
        *cwd = None;
    } else if let Some(base) = cwd.as_ref() {
        *cwd = Some(absolute(Path::new(&dir.text), base));
    } else if Path::new(&dir.text).is_absolute() {
        *cwd = Some(absolute(Path::new(&dir.text), Path::new("/")));
    }
}

pub(super) fn update_cwd(words: &[Word], cwd: &mut Option<PathBuf>) {
    let mut prefix_len = 0;
    let mut coprocess = false;
    while let Some(word) = words.get(prefix_len) {
        if is_assignment(word.as_str()) {
            prefix_len += 1;
            continue;
        }
        if word.quoted || !shell_prefix(word.as_str()) {
            break;
        }
        coprocess |= word.as_str() == "coproc";
        prefix_len += 1;
        if word.as_str() == "time"
            && words
                .get(prefix_len)
                .is_some_and(|option| !option.quoted && option.as_str() == "-p")
        {
            prefix_len += 1;
        }
    }
    // A coprocess runs in a child shell, so its cd cannot move this shell.
    if coprocess {
        return;
    }
    let (prefix, words) = words.split_at(prefix_len);
    let Some((program, args)) = words.split_first() else {
        return;
    };
    let mut rest = args.iter();
    let compound = prefix.iter().any(|w| {
        !w.quoted
            && matches!(
                w.as_str(),
                "{" | "if" | "then" | "do" | "else" | "elif" | "while" | "until"
            )
    });
    if !matches!(program.as_str(), "cd" | "pushd" | "popd") {
        // An unmodeled compound may execute a directory change conditionally.
        if (!prefix.is_empty()
            || matches!(program.as_str(), "for" | "select" | "case" | "function"))
            && rest.any(|w| matches!(w.as_str(), "cd" | "pushd" | "popd"))
        {
            *cwd = None;
        }
        return;
    }
    let args: Vec<_> = rest
        .filter(|w| !matches!(w.as_str(), "--" | "-L" | "-P"))
        .collect();
    if compound || program.as_str() == "popd" || args.len() != 1 || args[0].text.starts_with('-') {
        *cwd = None;
        return;
    }
    change_directory(args[0], cwd);
}

pub(super) fn exits_list(segments: &[Segment]) -> bool {
    let [
        Segment::Command(words),
        Segment::Boundary(Boundary::Sequence),
        ..,
    ] = segments
    else {
        return false;
    };
    match words.as_slice() {
        [program] => program.as_str() == "exit" && !program.dynamic,
        [program, status] => {
            program.as_str() == "exit"
                && !program.dynamic
                && !status.dynamic
                && status.as_str().parse::<i64>().is_ok()
        }
        _ => false,
    }
}
