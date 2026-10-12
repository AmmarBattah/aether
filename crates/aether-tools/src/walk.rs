//! Handle-anchored traversal with the tools' ignore::WalkBuilder filtering policy.

use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, BufRead};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, SystemTime};

use ignore::gitignore::{Gitignore, GitignoreBuilder};
use nix::dir::{Dir, Type};
use nix::fcntl::{AtFlags, OFlag, openat};
use nix::sys::stat::{FileStat, Mode, SFlag, fstatat};

use crate::fs::{
    directory_search_flags, open_directory, open_parent, open_regular_at, read_regular_file_at,
    root_name,
};
use crate::tool::ToolError;

const MAX_OPEN_DIRECTORIES: usize = 64;
const MAX_SKIPPED_PATHS: usize = 20;

#[derive(Default)]
pub(crate) struct Options {
    pub max_depth: Option<usize>,
    pub include_hidden: bool,
    pub sorted: bool,
}

#[derive(Clone)]
pub(crate) struct Entry {
    path: PathBuf,
    parent: Rc<File>,
    kind: SFlag,
    depth: usize,
}

impl Entry {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn file_name(&self) -> &OsStr {
        self.path.file_name().unwrap_or(OsStr::new("."))
    }

    pub fn depth(&self) -> usize {
        self.depth
    }

    pub fn is_dir(&self) -> bool {
        self.kind == SFlag::S_IFDIR
    }

    pub fn is_file(&self) -> bool {
        self.kind == SFlag::S_IFREG
    }

    fn metadata(&self) -> io::Result<FileStat> {
        fstatat(
            &*self.parent,
            Path::new(self.file_name()),
            AtFlags::AT_SYMLINK_NOFOLLOW,
        )
        .map_err(Into::into)
    }

    pub fn size(&self) -> Option<u64> {
        self.metadata().ok().map(|stat| stat.st_size as u64)
    }

    pub fn modified(&self) -> Option<SystemTime> {
        let stat = self.metadata().ok()?;
        let seconds = stat.st_mtime;
        let time = if seconds < 0 {
            SystemTime::UNIX_EPOCH.checked_sub(Duration::from_secs(seconds.unsigned_abs()))?
        } else {
            SystemTime::UNIX_EPOCH.checked_add(Duration::from_secs(seconds as u64))?
        };
        time.checked_add(Duration::from_nanos(stat.st_mtime_nsec as u64))
    }

    pub fn read(&self) -> Result<Vec<u8>, ToolError> {
        read_regular_file_at(&self.parent, &self.path)
    }
}

struct Frame {
    path: PathBuf,
    directory: Rc<File>,
    entries: std::vec::IntoIter<(Box<OsStr>, Option<Type>)>,
    depth: usize,
}

#[cfg(test)]
type AfterOpen = Box<dyn FnMut(&Path)>;

pub(crate) struct Walk {
    options: Options,
    pending: Option<Entry>,
    frames: Vec<Frame>,
    rules: Vec<Rules>,
    global: Gitignore,
    skipped_directories: Vec<PathBuf>,
    omitted_directories: usize,
    #[cfg(test)]
    after_open: Option<AfterOpen>,
}

impl Walk {
    pub fn new(root: &Path, options: Options) -> Result<Self, ToolError> {
        let name = root_name(root)?;
        let parent = if root.parent().is_some() {
            open_parent(root, false)
        } else {
            open_directory(root, false)
        }
        .map_err(|error| match error {
            ToolError::Io(error) => walk_error(root, error),
            error => error,
        })?;
        let stat = fstatat(&parent, Path::new(name), AtFlags::AT_SYMLINK_NOFOLLOW)
            .map_err(|e| walk_error(root, e.into()))?;
        // Callers already resolved and scope-checked aliases. Never resolve a new root link.
        if SFlag::from_bits_truncate(stat.st_mode) == SFlag::S_IFLNK {
            return Err(walk_error(root, nix::errno::Errno::ELOOP.into()));
        }
        let entry = Entry {
            path: root.to_path_buf(),
            parent: Rc::new(parent),
            kind: SFlag::from_bits_truncate(stat.st_mode),
            depth: 0,
        };
        let mut rules = Vec::new();
        if entry.is_dir() {
            for ancestor in root
                .ancestors()
                .skip(1)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
            {
                let directory = open_directory(ancestor, false)?;
                let (rule, errors) = Rules::load(&directory, ancestor);
                if !errors.is_empty() {
                    return Err(ToolError::Internal(
                        ignore::Error::Partial(errors).to_string(),
                    ));
                }
                rules.push(rule);
            }
        }
        let global = global_rules();
        Ok(Self {
            options,
            pending: Some(entry),
            frames: Vec::new(),
            rules,
            global,
            skipped_directories: Vec::new(),
            omitted_directories: 0,
            #[cfg(test)]
            after_open: None,
        })
    }

    pub fn depth_limit_notice(&self) -> Option<String> {
        if self.skipped_directories.is_empty() {
            return None;
        }
        let mut notices = self.skipped_directories.iter().map(|path| format!(
            "[skipped {}: directory depth limit of {MAX_OPEN_DIRECTORIES} reached; search this directory directly]",
            path.display()
        )).collect::<Vec<_>>();
        if self.omitted_directories > 0 {
            notices.push(format!(
                "[and {} more skipped directories]",
                self.omitted_directories
            ));
        }
        Some(notices.join("\n"))
    }

    fn descend(&mut self, entry: &Entry) -> Result<(), ToolError> {
        // Bound daemon-wide descriptor pressure without reopening mutable directory paths.
        if self.frames.len() == MAX_OPEN_DIRECTORIES {
            if self.skipped_directories.len() < MAX_SKIPPED_PATHS {
                self.skipped_directories.push(entry.path.clone());
            } else {
                self.omitted_directories += 1;
            }
            return Ok(());
        }
        let directory = Rc::new(
            open_directory_at(&entry.parent, Path::new(entry.file_name()), OFlag::O_RDONLY)
                .map_err(|e| walk_error(entry.path(), e))?,
        );
        #[cfg(test)]
        if let Some(hook) = &mut self.after_open {
            hook(entry.path());
        }
        let (rules, errors) = Rules::load(&directory, entry.path());
        // WalkBuilder attaches child parse errors to entries; these tools use the valid rules.
        for error in errors {
            tracing::debug!(%error, "directory ignore rules partially loaded");
        }
        let dir = Dir::from_fd(directory.try_clone()?.into())
            .map_err(|e| walk_error(entry.path(), e.into()))?;
        let entries = dir.into_iter().filter_map(|entry| match entry {
            Ok(entry) => {
                let name = OsStr::from_bytes(entry.file_name().to_bytes());
                (name != "." && name != "..")
                    .then(|| Ok((name.to_os_string().into_boxed_os_str(), entry.file_type())))
            }
            Err(error) => Some(Err(error.into())),
        });
        // Close the stream's duplicate before retaining the single anchored directory handle.
        let mut entries = entries
            .collect::<io::Result<Vec<_>>>()
            .map_err(|e| walk_error(entry.path(), e))?;
        if self.options.sorted {
            entries.sort_by(|a, b| a.0.cmp(&b.0));
        }
        self.rules.push(rules);
        self.frames.push(Frame {
            path: entry.path.clone(),
            directory,
            entries: entries.into_iter(),
            depth: entry.depth,
        });
        Ok(())
    }

    fn ignored(&self, entry: &Entry) -> bool {
        // .ignore beats .gitignore at every depth; nearer rules win within each class.
        for class in 0..3 {
            for rules in self.rules.iter().rev() {
                let matched = rules.matchers[class].matched(entry.path(), entry.is_dir());
                if !matched.is_none() {
                    return matched.is_ignore();
                }
            }
        }
        let matched = self.global.matched(entry.path(), entry.is_dir());
        if !matched.is_none() {
            return matched.is_ignore();
        }
        !self.options.include_hidden && entry.file_name().as_bytes().starts_with(b".")
    }
}

impl Iterator for Walk {
    type Item = Result<Entry, ToolError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(entry) = self.pending.take() {
            // Descend on the next call so a result cap never opens an unneeded directory.
            if entry.is_dir()
                && self.options.max_depth.is_none_or(|max| entry.depth < max)
                && let Err(error) = self.descend(&entry)
            {
                return Some(Err(error));
            }
            if entry.depth == 0 {
                return Some(Ok(entry));
            }
        }
        loop {
            let frame = self.frames.last_mut()?;
            let (name, file_type) = match frame.entries.next() {
                Some(child) => child,
                None => {
                    self.frames.pop();
                    self.rules.pop();
                    continue;
                }
            };
            let path = frame.path.join(&*name);
            let kind = match file_type {
                Some(Type::Directory) => SFlag::S_IFDIR,
                Some(Type::File) => SFlag::S_IFREG,
                Some(Type::Symlink) => SFlag::S_IFLNK,
                Some(Type::Fifo) => SFlag::S_IFIFO,
                Some(Type::CharacterDevice) => SFlag::S_IFCHR,
                Some(Type::BlockDevice) => SFlag::S_IFBLK,
                Some(Type::Socket) => SFlag::S_IFSOCK,
                None => match fstatat(
                    &*frame.directory,
                    Path::new(&*name),
                    AtFlags::AT_SYMLINK_NOFOLLOW,
                ) {
                    Ok(stat) => SFlag::from_bits_truncate(stat.st_mode),
                    Err(error) => return Some(Err(walk_error(&path, error.into()))),
                },
            };
            let entry = Entry {
                path,
                parent: frame.directory.clone(),
                kind,
                depth: frame.depth + 1,
            };
            if self.ignored(&entry) {
                continue;
            }
            if entry.is_dir() {
                self.pending = Some(entry.clone());
            }
            return Some(Ok(entry));
        }
    }
}

fn open_directory_at(parent: &File, name: &Path, access: OFlag) -> io::Result<File> {
    openat(
        parent,
        name,
        access | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
        Mode::empty(),
    )
    .map(Into::into)
    .map_err(Into::into)
}

fn walk_error(path: &Path, error: io::Error) -> ToolError {
    ToolError::Internal(format!(
        "{path}: IO error for operation on {path}: {error}",
        path = path.display()
    ))
}

struct Rules {
    matchers: [Gitignore; 3],
}

impl Rules {
    fn load(directory: &File, path: &Path) -> (Self, Vec<ignore::Error>) {
        // Local .ignore, .gitignore and .git symlinks deliberately do not redirect rule reads.
        let mut errors = Vec::new();
        let ignore = read_ignore(directory, path, ".ignore", &mut errors);
        let gitignore = read_ignore(directory, path, ".gitignore", &mut errors);
        // With require_git(false), WalkBuilder reads .git/info/exclude without gitdir redirects.
        let exclude = open_directory_at(directory, Path::new(".git"), directory_search_flags())
            .and_then(|git| open_directory_at(&git, Path::new("info"), directory_search_flags()));
        let exclude = match exclude {
            Ok(info) => read_ignore(&info, path, ".git/info/exclude", &mut errors),
            Err(error) => {
                tracing::trace!(%error, path = %path.display(), "optional git exclude directory unavailable");
                Gitignore::empty()
            }
        };
        (
            Self {
                matchers: [ignore, gitignore, exclude],
            },
            errors,
        )
    }
}

fn global_rules() -> Gitignore {
    // Host configuration is trusted; dotfile managers commonly symlink global excludes.
    let (matcher, error) = Gitignore::global();
    if let Some(error) = error {
        tracing::debug!(%error, "global ignore rules partially loaded");
    }
    matcher
}

fn read_ignore(
    directory: &File,
    base: &Path,
    name: impl AsRef<Path>,
    errors: &mut Vec<ignore::Error>,
) -> Gitignore {
    let path = base.join(name);
    let file = match open_regular_at(directory, &path, OFlag::O_RDONLY) {
        Ok(file) => file,
        Err(error) => {
            // Optional ignore-file I/O failures are nonfatal, matching ignore::WalkBuilder.
            tracing::trace!(%error, path = %path.display(), "optional ignore file unavailable");
            return Gitignore::empty();
        }
    };
    let mut builder = GitignoreBuilder::new(base);
    for (i, line) in io::BufReader::new(file).lines().enumerate() {
        let line = match line {
            Ok(line) => line,
            Err(error) => {
                tracing::debug!(%error, path = %path.display(), "ignore file read stopped");
                break;
            }
        };
        let line = if i == 0 {
            line.trim_start_matches('\u{feff}')
        } else {
            &line
        };
        if let Err(error) = builder.add_line(Some(path.clone()), line) {
            errors.push(ignore::Error::WithPath {
                path: path.clone(),
                err: Box::new(ignore::Error::WithLineNumber {
                    line: (i + 1) as u64,
                    err: Box::new(error),
                }),
            });
        }
    }
    match builder.build() {
        Ok(matcher) => matcher,
        Err(error) => {
            errors.push(error);
            Gitignore::empty()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::resolve_path;
    use crate::testing::ctx_in;
    use std::os::unix::fs::symlink;

    fn names(walk: Walk, root: &Path) -> Result<Vec<String>, ToolError> {
        walk.filter_map(|entry| match entry {
            Ok(entry) if entry.depth() == 0 => None,
            Ok(entry) => Some(Ok(entry
                .path()
                .strip_prefix(root)
                .unwrap()
                .display()
                .to_string())),
            Err(error) => Some(Err(error)),
        })
        .collect()
    }

    fn swap(path: &Path, outside: &Path) {
        std::fs::rename(path, path.with_extension("moved")).unwrap();
        symlink(outside, path).unwrap();
    }

    #[test]
    fn global_excludes_follow_file_and_directory_symlinks() {
        const FIXTURE: &str = "AETHER_WALK_GLOBAL_CONFIG_FIXTURE";
        if let Some(root) = std::env::var_os(FIXTURE) {
            let root = PathBuf::from(root);
            let actual = names(
                Walk::new(
                    &root,
                    Options {
                        sorted: true,
                        ..Options::default()
                    },
                )
                .unwrap(),
                &root,
            )
            .unwrap();
            let expected: Vec<_> = ignore::WalkBuilder::new(&root)
                .require_git(false)
                .sort_by_file_path(|a, b| a.cmp(b))
                .build()
                .map(|entry| entry.unwrap())
                .filter(|entry| entry.depth() > 0)
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect();
            assert_eq!(expected, ["kept"]);
            assert_eq!(actual, expected);
            return;
        }
        let fixture = tempfile::tempdir().unwrap();
        let base = fixture.path().canonicalize().unwrap();
        let root = base.join("workspace");
        let home = base.join("home");
        let real_git = base.join("dotfiles/git");
        for directory in [&root, &home, &real_git] {
            std::fs::create_dir_all(directory).unwrap();
        }
        std::fs::write(root.join("kept"), "").unwrap();
        std::fs::write(root.join("globally-ignored"), "").unwrap();
        std::fs::write(real_git.join("ignore"), "globally-ignored\n").unwrap();
        let empty_config = base.join("empty-config");
        std::fs::write(&empty_config, "").unwrap();
        for directory_link in [false, true] {
            let xdg = base.join(if directory_link {
                "directory-link"
            } else {
                "file-link"
            });
            std::fs::create_dir(&xdg).unwrap();
            if directory_link {
                symlink(&real_git, xdg.join("git")).unwrap();
            } else {
                std::fs::create_dir(xdg.join("git")).unwrap();
                symlink(real_git.join("ignore"), xdg.join("git/ignore")).unwrap();
            }
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "walk::tests::global_excludes_follow_file_and_directory_symlinks",
                    "--nocapture",
                ])
                .current_dir(&root)
                .env(FIXTURE, &root)
                .env("HOME", &home)
                .env("XDG_CONFIG_HOME", &xdg)
                .env("GIT_CONFIG_GLOBAL", &empty_config)
                .env("GIT_CONFIG_SYSTEM", &empty_config)
                .output()
                .unwrap();
            assert!(
                output.status.success()
                    && String::from_utf8_lossy(&output.stdout).contains("1 passed"),
                "directory_link={directory_link}: {}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn deep_walk_bounds_file_descriptors() {
        const FIXTURE: &str = "AETHER_WALK_FD_FIXTURE";
        if std::env::var_os(FIXTURE).is_none() {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "walk::tests::deep_walk_bounds_file_descriptors",
                    "--nocapture",
                ])
                .env(FIXTURE, "1")
                .output()
                .unwrap();
            assert!(
                output.status.success()
                    && String::from_utf8_lossy(&output.stdout).contains("1 passed"),
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        fn fd_count() -> usize {
            std::fs::read_dir("/proc/self/fd")
                .unwrap()
                .collect::<io::Result<Vec<_>>>()
                .unwrap()
                .len()
        }
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path().canonicalize().unwrap();
        let mut leaf = root.clone();
        for _ in 0..700 {
            leaf.push("d");
        }
        std::fs::create_dir_all(&leaf).unwrap();
        std::fs::write(leaf.join("too-deep"), "").unwrap();
        std::fs::write(root.join("z-kept"), "").unwrap();
        let before = fd_count();
        let peak = Rc::new(std::cell::Cell::new(before));
        let mut walk = Walk::new(&root, Options::default()).unwrap();
        let observed = peak.clone();
        walk.after_open = Some(Box::new(move |_| {
            observed.set(observed.get().max(fd_count()))
        }));
        let mut deepest = 0;
        let mut kept = false;
        for entry in walk.by_ref() {
            let entry = entry.unwrap();
            deepest = deepest.max(entry.depth());
            kept |= entry.file_name() == "z-kept";
            assert_ne!(entry.file_name(), "too-deep");
            peak.set(peak.get().max(fd_count()));
            assert!(
                peak.get() <= before + MAX_OPEN_DIRECTORIES + 4,
                "open descriptors: baseline={before}, peak={}",
                peak.get()
            );
        }
        assert!(kept);
        assert_eq!(deepest, MAX_OPEN_DIRECTORIES);
        assert_eq!(walk.skipped_directories.len(), 1);
        let notice = walk.depth_limit_notice().unwrap();
        assert!(notice.contains(&format!("directory depth limit of {MAX_OPEN_DIRECTORIES}")));
        assert!(notice.contains(&walk.skipped_directories[0].display().to_string()));
        drop(walk);
        assert_eq!(fd_count(), before);
    }

    #[tokio::test]
    async fn depth_limited_tools_preserve_partial_results_and_name_skipped_paths() {
        use crate::builtin::{glob::GlobTool, grep::GrepTool, list_dir::ListDirTool};
        use crate::tool::{Tool, ToolExecution};
        use serde_json::json;
        use tokio_util::sync::CancellationToken;

        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path().canonicalize().unwrap();
        let mut capped = root.clone();
        for _ in 0..MAX_OPEN_DIRECTORIES {
            capped.push("d");
        }
        std::fs::create_dir_all(capped.join("d")).unwrap();
        std::fs::write(capped.join("d/too-deep"), "needle").unwrap();
        std::fs::write(root.join("z-kept"), "needle").unwrap();
        let tools: Vec<(Box<dyn Tool>, serde_json::Value)> = vec![
            (
                Box::new(ListDirTool),
                json!({"depth": MAX_OPEN_DIRECTORIES + 2}),
            ),
            (Box::new(GlobTool), json!({"pattern": "**"})),
            (Box::new(GrepTool), json!({"pattern": "needle"})),
        ];
        for (tool, mut input) in tools {
            let ToolExecution::Outcome(out) = tool
                .execute(input.clone(), &ctx_in(&root), CancellationToken::new())
                .await
                .unwrap()
            else {
                panic!()
            };
            assert!(out.truncated);
            assert!(out.content.contains("z-kept"));
            assert!(!out.content.contains("too-deep"));
            assert!(out.content.contains(&capped.display().to_string()));
            assert!(
                out.content
                    .contains(&format!("directory depth limit of {MAX_OPEN_DIRECTORIES}"))
            );
            input["path"] = json!(capped);
            let ToolExecution::Outcome(out) = tool
                .execute(input, &ctx_in(&root), CancellationToken::new())
                .await
                .unwrap()
            else {
                panic!()
            };
            assert!(!out.truncated);
            assert!(out.content.contains("too-deep"));
            assert!(!out.content.contains("directory depth limit"));
        }
        let mut walk = Walk::new(
            &root,
            Options {
                max_depth: Some(MAX_OPEN_DIRECTORIES),
                ..Options::default()
            },
        )
        .unwrap();
        for entry in walk.by_ref() {
            entry.unwrap();
        }
        assert!(
            walk.depth_limit_notice().is_none(),
            "the requested depth was completely walked"
        );
    }

    #[test]
    fn depth_limit_notice_bounds_skipped_paths() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path().canonicalize().unwrap();
        let mut parent = root.clone();
        for _ in 1..MAX_OPEN_DIRECTORIES {
            parent.push("d");
        }
        for total in [20, 25] {
            for i in 0..total {
                std::fs::create_dir_all(parent.join(format!("child-{i:02}"))).unwrap();
            }
            let mut walk = Walk::new(
                &root,
                Options {
                    sorted: true,
                    ..Options::default()
                },
            )
            .unwrap();
            for entry in walk.by_ref() {
                entry.unwrap();
            }
            let notice = walk.depth_limit_notice().unwrap();
            assert_eq!(walk.skipped_directories.len(), 20);
            for i in 0..20 {
                assert!(
                    notice.contains(&parent.join(format!("child-{i:02}")).display().to_string())
                );
            }
            assert!(!notice.contains("child-20"));
            if total == 20 {
                assert_eq!(notice.lines().count(), 20);
                assert!(!notice.contains("more skipped directories"));
            } else {
                assert_eq!(notice.lines().count(), 21);
                assert!(notice.ends_with("[and 5 more skipped directories]"));
            }
        }
    }

    #[test]
    fn roots_ending_in_parent_components_are_rejected() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path().canonicalize().unwrap().join("..");
        for path in [
            root.as_path(),
            Path::new(".."),
            Path::new("."),
            Path::new(""),
        ] {
            assert!(matches!(
                Walk::new(path, Options::default()),
                Err(ToolError::InvalidInput(_))
            ));
            assert!(matches!(
                crate::fs::file_type(path),
                Err(ToolError::InvalidInput(_))
            ));
        }
        assert_eq!(root_name(Path::new("/")).unwrap(), ".");
        assert_eq!(
            crate::fs::file_type(Path::new("/")).unwrap(),
            SFlag::S_IFDIR
        );
    }

    #[test]
    fn root_swap_after_scope_check_never_follows_the_link() {
        let workspace = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let root = workspace.path().join("root");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(outside.path().join("outside-secret"), "secret").unwrap();
        std::fs::write(outside.path().join(".gitignore"), "[invalid\n").unwrap();
        let root = resolve_path(&ctx_in(workspace.path()), "root").unwrap();
        swap(&root, outside.path());
        assert!(Walk::new(&root, Options::default()).is_err());
    }

    #[test]
    fn ancestor_swap_after_scope_check_never_follows_the_link() {
        let workspace = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(workspace.path().join("parent/root")).unwrap();
        std::fs::create_dir(outside.path().join("root")).unwrap();
        std::fs::write(outside.path().join("root/outside-secret"), "secret").unwrap();
        let root = resolve_path(&ctx_in(workspace.path()), "parent/root").unwrap();
        swap(root.parent().unwrap(), outside.path());
        assert!(Walk::new(&root, Options::default()).is_err());
    }

    #[test]
    fn root_swap_after_stat_never_reads_outside_entries_or_rules() {
        let workspace = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let root = workspace.path().canonicalize().unwrap().join("root");
        std::fs::create_dir(&root).unwrap();
        let mut walk = Walk::new(&root, Options::default()).unwrap();
        swap(&root, outside.path());
        assert!(walk.next().unwrap().is_err());
    }

    #[test]
    fn mid_walk_swap_after_cached_entry_type_cannot_descend() {
        let workspace = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let root = workspace.path().canonicalize().unwrap();
        std::fs::create_dir(root.join("sub")).unwrap();
        std::fs::write(outside.path().join("outside-secret"), "secret").unwrap();
        let mut walk = Walk::new(&root, Options::default()).unwrap();
        assert_eq!(walk.next().unwrap().unwrap().depth(), 0);
        assert_eq!(walk.next().unwrap().unwrap().path(), root.join("sub"));
        swap(&root.join("sub"), outside.path());
        assert!(walk.next().unwrap().is_err());
    }

    #[test]
    fn pinned_directories_keep_entries_ignore_files_and_contents_inside() {
        for target in ["root", "root/sub"] {
            let workspace = tempfile::tempdir().unwrap();
            let outside = tempfile::tempdir().unwrap();
            let root = workspace.path().canonicalize().unwrap().join("root");
            std::fs::create_dir_all(root.join("sub/.git/info")).unwrap();
            let target = workspace.path().canonicalize().unwrap().join(target);
            for path in [&target, outside.path()] {
                std::fs::create_dir_all(path.join(".git/info")).unwrap();
            }
            std::fs::write(target.join("inside"), "inside contents").unwrap();
            std::fs::write(target.join("ignored"), "").unwrap();
            std::fs::write(target.join(".gitignore"), "ignored\n").unwrap();
            std::fs::write(outside.path().join("outside-secret"), "secret").unwrap();
            // Reading any redirected rule would hide the inside sentinel.
            for name in [".ignore", ".gitignore", ".git/info/exclude"] {
                std::fs::write(outside.path().join(name), "inside\n").unwrap();
            }
            let mut walk = Walk::new(&root, Options::default()).unwrap();
            let swapped = target.clone();
            walk.after_open = Some(Box::new(move |path| {
                if path == swapped {
                    swap(path, outside.path());
                }
            }));
            let mut found_inside = false;
            for entry in walk {
                let entry = entry.unwrap();
                assert_ne!(entry.file_name(), "outside-secret");
                assert_ne!(entry.file_name(), "ignored");
                if entry.path() == target.join("inside") {
                    assert_eq!(entry.size(), Some(15));
                    assert!(entry.modified().is_some());
                    assert_eq!(entry.read().unwrap(), b"inside contents");
                    found_inside = true;
                }
            }
            assert!(
                found_inside,
                "anchored ignore reads for {}",
                target.display()
            );
        }
    }

    #[test]
    fn ignore_file_and_exclude_directory_links_are_not_followed() {
        let workspace = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let root = workspace.path().canonicalize().unwrap();
        std::fs::write(root.join("inside"), "").unwrap();
        std::fs::write(outside.path().join("rules"), "inside\n").unwrap();
        std::fs::create_dir(outside.path().join("info")).unwrap();
        std::fs::write(outside.path().join("info/exclude"), "inside\n").unwrap();
        symlink(outside.path().join("rules"), root.join(".ignore")).unwrap();
        symlink(outside.path().join("rules"), root.join(".gitignore")).unwrap();
        symlink(outside.path(), root.join(".git")).unwrap();
        assert_eq!(
            names(Walk::new(&root, Options::default()).unwrap(), &root).unwrap(),
            ["inside"]
        );
    }

    #[test]
    fn matches_walkbuilder_filtering_order_depth_and_parent_rules() {
        let workspace = tempfile::tempdir().unwrap();
        let parent = workspace.path().canonicalize().unwrap();
        let root = parent.join("root");
        std::fs::create_dir_all(root.join("sub/deep")).unwrap();
        std::fs::create_dir_all(root.join("pruned")).unwrap();
        std::fs::create_dir_all(root.join(".hidden-dir")).unwrap();
        std::fs::create_dir_all(root.join(".git/info")).unwrap();
        for name in [
            "visible",
            "parent-ignore",
            "parent-git",
            "ignored",
            "keep.tmp",
            "skip.tmp",
            "by-exclude",
            ".hidden",
            ".allowed",
            "sub/keep.tmp",
            "sub/skip.tmp",
            "sub/parent-ignore",
            "sub/parent-git",
            "sub/by-exclude",
            "sub/visible",
            "sub/deep/visible",
            "pruned/file",
            ".hidden-dir/file",
        ] {
            std::fs::write(root.join(name), "").unwrap();
        }
        std::fs::write(parent.join(".ignore"), "parent-ignore\n!keep.tmp\n").unwrap();
        std::fs::write(parent.join(".gitignore"), "parent-git\n").unwrap();
        std::fs::write(
            root.join(".gitignore"),
            "\u{feff}*.tmp\nignored\n!.allowed\npruned/\n",
        )
        .unwrap();
        std::fs::write(root.join(".git/info/exclude"), "by-exclude\n").unwrap();
        std::fs::write(root.join("sub/.gitignore"), "!skip.tmp\n!parent-git\n").unwrap();
        std::fs::write(root.join("sub/.ignore"), "!parent-ignore\nkeep.tmp\n").unwrap();
        symlink("sub", root.join("alias")).unwrap();
        for start in [&root, &root.join("sub")] {
            for max_depth in [None, Some(1), Some(2)] {
                for include_hidden in [false, true] {
                    for sorted in [false, true] {
                        let mut builder = ignore::WalkBuilder::new(start);
                        builder
                            .require_git(false)
                            .hidden(!include_hidden)
                            .max_depth(max_depth);
                        if sorted {
                            builder.sort_by_file_path(|a, b| a.cmp(b));
                        }
                        let expected: Vec<_> = builder
                            .build()
                            .map(|entry| entry.unwrap())
                            .filter(|entry| entry.depth() > 0)
                            .map(|entry| {
                                entry
                                    .path()
                                    .strip_prefix(start)
                                    .unwrap()
                                    .display()
                                    .to_string()
                            })
                            .collect();
                        let actual = names(
                            Walk::new(
                                start,
                                Options {
                                    max_depth,
                                    include_hidden,
                                    sorted,
                                },
                            )
                            .unwrap(),
                            start,
                        )
                        .unwrap();
                        assert_eq!(
                            actual, expected,
                            "start={start:?}, depth={max_depth:?}, hidden={include_hidden}, sorted={sorted}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn listing_a_readable_directory_does_not_require_search_permission() {
        use std::os::unix::fs::PermissionsExt;

        if nix::unistd::geteuid().is_root() {
            return;
        }
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().canonicalize().unwrap().join("root");
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::write(root.join("file"), "").unwrap();
        let walk = Walk::new(
            &root,
            Options {
                max_depth: Some(1),
                sorted: true,
                ..Options::default()
            },
        )
        .unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o400)).unwrap();
        let expected = ignore::WalkBuilder::new(&root)
            .require_git(false)
            .max_depth(Some(1))
            .sort_by_file_path(|a, b| a.cmp(b))
            .build()
            .map(|entry| {
                entry.map(|entry| {
                    entry
                        .path()
                        .strip_prefix(&root)
                        .unwrap()
                        .display()
                        .to_string()
                })
            })
            .collect::<Result<Vec<_>, _>>();
        let actual = names(walk, &root);
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(
            actual.unwrap(),
            expected.unwrap().into_iter().skip(1).collect::<Vec<_>>()
        );
    }

    #[test]
    fn errors_and_partial_ignore_rules_match_walkbuilder() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().canonicalize().unwrap().join("root");
        let expected = ignore::WalkBuilder::new(&root)
            .require_git(false)
            .build()
            .next()
            .unwrap()
            .unwrap_err()
            .to_string();
        let actual = match Walk::new(&root, Options::default()) {
            Ok(_) => panic!("missing root accepted"),
            Err(ToolError::Internal(error)) => error,
            Err(error) => panic!("unexpected error: {error}"),
        };
        assert_eq!(actual, expected);
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("hidden-by-valid-rule"), "").unwrap();
        std::fs::write(root.join(".gitignore"), "[z-a]\nhidden-by-valid-rule\n").unwrap();
        assert!(
            names(Walk::new(&root, Options::default()).unwrap(), &root)
                .unwrap()
                .is_empty()
        );
        std::fs::rename(root.join(".gitignore"), workspace.path().join(".gitignore")).unwrap();
        let expected = ignore::WalkBuilder::new(&root)
            .require_git(false)
            .build()
            .next()
            .unwrap()
            .unwrap_err()
            .to_string();
        let actual = match Walk::new(&root, Options::default()) {
            Ok(_) => panic!("invalid parent rule accepted"),
            Err(ToolError::Internal(error)) => error,
            Err(error) => panic!("unexpected error: {error}"),
        };
        assert_eq!(actual, expected);
    }

    #[test]
    fn single_file_roots_and_scoped_root_aliases_remain_supported() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().canonicalize().unwrap();
        std::fs::write(root.join("file"), "contents").unwrap();
        symlink("file", root.join("alias")).unwrap();
        let resolved = resolve_path(&ctx_in(&root), "alias").unwrap();
        let entries: Vec<_> = Walk::new(&resolved, Options::default())
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].is_file());
        assert_eq!(entries[0].read().unwrap(), b"contents");
        std::fs::create_dir(root.join("directory")).unwrap();
        std::fs::write(root.join("directory/inside"), "").unwrap();
        symlink("directory", root.join("directory-alias")).unwrap();
        let resolved = resolve_path(&ctx_in(&root), "directory-alias").unwrap();
        assert_eq!(
            names(Walk::new(&resolved, Options::default()).unwrap(), &resolved).unwrap(),
            ["inside"]
        );
    }
}
