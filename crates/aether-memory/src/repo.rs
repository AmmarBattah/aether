//! Repository knowledge: durable facts a repository carries in its own tree.
//!
//! A repository documents itself for the people who work in it; an agent
//! working there should start from the same notes. Committed markdown under
//! `.aether/knowledge/` becomes memory scoped to that repository (or, under
//! a role directory such as `team.steward/`, to that role family within
//! it), with provenance pointing at the file and the commit it was read at,
//! so a fact can always be traced back to the line that stated it.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use aether_core::memory::{
    MAX_ROLE_SEGMENTS, MemoryCandidate, MemoryKind, MemoryProvenance, MemoryScope, MemorySource,
    is_role_family,
};

/// Directory, relative to the repository root, holding committed notes.
pub const KNOWLEDGE_DIR: &str = ".aether/knowledge";

/// Committed notes are deliberate, so they start above the admission floor;
/// an agent's own observation has to earn its place against them.
const SEEDED_IMPORTANCE: f32 = 0.7;
const SEEDED_CONFIDENCE: f32 = 0.9;

/// Anyone who can get a branch into the tree can write these files, and what
/// they say reaches the system prompt. So the surface is bounded: a note set
/// this large is a mistake or an attack, and either way the answer is a
/// refusal a person can see rather than a prompt nobody reads to the end.
const MAX_FILES: usize = 64;
const MAX_FILE_BYTES: u64 = 64 * 1024;
const MAX_TOTAL_BYTES: u64 = 256 * 1024;
const MAX_NOTES: usize = 200;
/// Role directories read per scan; each one is a directory listing.
const MAX_ROLE_DIRS: usize = 16;

#[derive(Debug, thiserror::Error)]
pub enum RepoKnowledgeError {
    #[error("reading {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{path} is a symlink; repository notes are read from the tree itself")]
    Symlink { path: PathBuf },
    #[error("{0}")]
    TooMuch(String),
    #[error("{path}: {reason}")]
    Layout { path: PathBuf, reason: String },
}

/// Whether this tree carries committed notes at all. Cheap enough to ask on
/// every agent creation.
pub fn has_knowledge(root: &Path) -> bool {
    root.join(KNOWLEDGE_DIR).is_dir()
}

/// Read every note under `<root>/.aether/knowledge` for repository
/// `repository`, oldest path first. A note directly in the directory is for
/// the whole repository; one in a role directory (`team.steward/`) is for
/// agents of that role family there. Files that are not markdown are skipped;
/// a file that cannot be read is an error, because silently seeding half a
/// repository's rules is worse than seeding none - and so is a directory the
/// layout gives no meaning, whose notes would reach nobody.
///
/// Bounded and link-free by construction: this text reaches the system
/// prompt, so a symlink anywhere in the tree is refused rather than followed,
/// and exceeding the size, file, directory or note caps - totals across the
/// whole tree - is an error naming the cap.
pub fn scan(root: &Path, repository: &str) -> Result<Vec<MemoryCandidate>, RepoKnowledgeError> {
    let Some((root, dir)) = knowledge_dir(root)? else {
        return Ok(Vec::new());
    };
    let root = root.as_path();
    let too_many_files = |count: usize| {
        RepoKnowledgeError::TooMuch(format!(
            "{count} note files under {KNOWLEDGE_DIR}; capped at {MAX_FILES}"
        ))
    };
    let mut paths: Vec<(PathBuf, MemoryScope)> = Vec::new();
    let mut role_dirs: Vec<PathBuf> = Vec::new();
    for entry in entries(&dir)? {
        match entry {
            Entry::Note(path) => paths.push((
                path,
                MemoryScope::Repository {
                    id: repository.to_owned(),
                },
            )),
            Entry::Dir(path) => role_dirs.push(path),
        }
    }
    if role_dirs.len() > MAX_ROLE_DIRS {
        return Err(RepoKnowledgeError::TooMuch(format!(
            "{} role directories under {KNOWLEDGE_DIR}; capped at {MAX_ROLE_DIRS}",
            role_dirs.len()
        )));
    }
    role_dirs.sort();
    for role_dir in role_dirs {
        let role = role_dir
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| is_role_family(name))
            .ok_or_else(|| RepoKnowledgeError::Layout {
                path: role_dir.clone(),
                reason: format!(
                    "a directory here names a role family, like `team.steward`: 1 to \
                     {MAX_ROLE_SEGMENTS} dot-separated segments of ASCII letters, digits, `-` \
                     and `_`"
                ),
            })?
            .to_owned();
        // Listed as a real directory under the canonical `dir`; a link swapped in
        // since resolves elsewhere. Narrows the race, as for the knowledge directory.
        if canonical(&role_dir)? != role_dir {
            return Err(RepoKnowledgeError::Symlink { path: role_dir });
        }
        for entry in entries(&role_dir)? {
            match entry {
                Entry::Note(path) => paths.push((
                    path,
                    MemoryScope::RepositoryRole {
                        id: repository.to_owned(),
                        role: role.clone(),
                    },
                )),
                Entry::Dir(path) => {
                    return Err(RepoKnowledgeError::Layout {
                        path,
                        reason: format!(
                            "role directories are one level deep; the dots carry the \
                             hierarchy, so name it `{role}.<name>` next to `{role}`"
                        ),
                    });
                }
            }
        }
        if paths.len() > MAX_FILES {
            return Err(too_many_files(paths.len()));
        }
    }
    if paths.len() > MAX_FILES {
        return Err(too_many_files(paths.len()));
    }
    paths.sort_by(|a, b| a.0.cmp(&b.0));

    let mut out: Vec<MemoryCandidate> = Vec::new();
    let mut bytes = 0u64;
    for (path, scope) in paths {
        let text = read_note(root, &path)?;
        bytes += text.len() as u64;
        if bytes > MAX_TOTAL_BYTES {
            return Err(RepoKnowledgeError::TooMuch(format!(
                "notes under {KNOWLEDGE_DIR} exceed {MAX_TOTAL_BYTES} bytes"
            )));
        }
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .into_owned();
        let source = MemorySource {
            commit: commit_stating(root, &relative),
            file: relative,
        };
        out.extend(parse(&text).into_iter().map(|note| MemoryCandidate {
            scope: scope.clone(),
            kind: note.kind,
            text: note.text,
            structured: None,
            importance: SEEDED_IMPORTANCE,
            confidence: SEEDED_CONFIDENCE,
            tags: note.tags,
            provenance: MemoryProvenance {
                source: Some(source.clone()),
                ..Default::default()
            },
            supersedes: None,
        }));
        if out.len() > MAX_NOTES {
            return Err(RepoKnowledgeError::TooMuch(format!(
                "more than {MAX_NOTES} notes under {KNOWLEDGE_DIR}"
            )));
        }
    }
    Ok(out)
}

fn open_note(root: &Path, path: &Path) -> Result<std::fs::File, RepoKnowledgeError> {
    use rustix::fs::{Mode, OFlags, open, openat};

    let relative = path
        .strip_prefix(root)
        .map_err(|_| RepoKnowledgeError::Layout {
            path: path.to_owned(),
            reason: "note path is outside the repository root".into(),
        })?;
    let read_error = |source: rustix::io::Errno| {
        if source == rustix::io::Errno::LOOP {
            RepoKnowledgeError::Symlink {
                path: path.to_owned(),
            }
        } else {
            RepoKnowledgeError::Read {
                path: path.to_owned(),
                source: source.into(),
            }
        }
    };
    let flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let mut directory = open(root, flags | OFlags::DIRECTORY, Mode::empty()).map_err(read_error)?;
    let parent = relative
        .parent()
        .ok_or_else(|| RepoKnowledgeError::Layout {
            path: path.to_owned(),
            reason: "note path has no parent".into(),
        })?;
    // Each component is opened relative to a held directory; renames cannot redirect it.
    for component in parent.components() {
        directory = openat(
            &directory,
            component.as_os_str(),
            flags | OFlags::DIRECTORY,
            Mode::empty(),
        )
        .map_err(read_error)?;
    }
    let name = relative
        .file_name()
        .ok_or_else(|| RepoKnowledgeError::Layout {
            path: path.to_owned(),
            reason: "note path has no file name".into(),
        })?;
    let file =
        openat(&directory, name, flags | OFlags::NONBLOCK, Mode::empty()).map_err(read_error)?;
    Ok(file.into())
}

fn read_note(root: &Path, path: &Path) -> Result<String, RepoKnowledgeError> {
    use std::io::Read;

    let read_error = |source| RepoKnowledgeError::Read {
        path: path.to_owned(),
        source,
    };
    let file = open_note(root, path)?;
    let metadata = file.metadata().map_err(read_error)?;
    if !metadata.is_file() {
        return Err(RepoKnowledgeError::Layout {
            path: path.to_owned(),
            reason: "a note must be a regular file".into(),
        });
    }
    let too_large = || {
        RepoKnowledgeError::TooMuch(format!(
            "{} exceeds {MAX_FILE_BYTES} bytes; a note file is capped at {MAX_FILE_BYTES}",
            path.display()
        ))
    };
    if metadata.len() > MAX_FILE_BYTES {
        return Err(too_large());
    }
    let mut text = String::new();
    file.take(MAX_FILE_BYTES + 1)
        .read_to_string(&mut text)
        .map_err(read_error)?;
    if text.len() as u64 > MAX_FILE_BYTES {
        return Err(too_large());
    }
    Ok(text)
}

/// What one directory entry in the knowledge tree is, for the scan.
enum Entry {
    /// A regular markdown file, within the per-file cap.
    Note(PathBuf),
    /// A real directory, not a link to one.
    Dir(PathBuf),
}

/// The notes and directories directly in `dir`. Other files, and hidden
/// directories (an editor's `.obsidian/`), are skipped.
///
/// Every entry is examined without following it: `is_file`, `is_dir` and
/// `read_to_string` all follow links, and git commits links happily, so
/// `notes.md -> ../../../../secrets` or `team -> /elsewhere` would pull a
/// file from outside the tree into the prompt, with the host path written
/// into its provenance. A link is refused whatever it is named.
fn entries(dir: &Path) -> Result<Vec<Entry>, RepoKnowledgeError> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|source| RepoKnowledgeError::Read {
        path: dir.to_owned(),
        source,
    })? {
        let path = entry
            .map_err(|source| RepoKnowledgeError::Read {
                path: dir.to_owned(),
                source,
            })?
            .path();
        let meta = std::fs::symlink_metadata(&path).map_err(|source| RepoKnowledgeError::Read {
            path: path.clone(),
            source,
        })?;
        if meta.is_symlink() {
            return Err(RepoKnowledgeError::Symlink { path });
        }
        if meta.is_dir() {
            if !path
                .file_name()
                .is_some_and(|n| n.as_encoded_bytes().starts_with(b"."))
            {
                out.push(Entry::Dir(path));
            }
            continue;
        }
        if !meta.is_file() || path.extension().is_none_or(|e| e != "md") {
            continue;
        }
        if meta.len() > MAX_FILE_BYTES {
            return Err(RepoKnowledgeError::TooMuch(format!(
                "{} is {} bytes; a note file is capped at {MAX_FILE_BYTES}",
                path.display(),
                meta.len()
            )));
        }
        out.push(Entry::Note(path));
    }
    Ok(out)
}

/// The canonical repository root and knowledge directory, or `None` when the
/// tree carries no notes.
///
/// The whole path is checked, not just its last component: `symlink_metadata`
/// does not follow the final component but every component before it is
/// followed, so a committed `.aether -> /elsewhere` would otherwise report a
/// real directory holding real files and read them into the system prompt.
/// Each component under the root is refused if it is a link, and the resolved
/// directory must still be inside the resolved root - a link created between
/// the two checks is caught by the second.
fn knowledge_dir(root: &Path) -> Result<Option<(PathBuf, PathBuf)>, RepoKnowledgeError> {
    let mut path = root.to_path_buf();
    for part in Path::new(KNOWLEDGE_DIR).components() {
        path.push(part);
        match link_metadata(&path)? {
            None => return Ok(None),
            Some(m) if m.is_symlink() => return Err(RepoKnowledgeError::Symlink { path }),
            Some(_) => {}
        }
    }
    // Canonical against canonical: the root itself may legitimately sit under
    // a link (`/tmp` on macOS), which says nothing about the notes.
    let root = canonical(root)?;
    let dir = canonical(&path)?;
    if !dir.starts_with(&root) {
        return Err(RepoKnowledgeError::Symlink { path });
    }
    Ok(dir.is_dir().then_some((root, dir)))
}

fn canonical(path: &Path) -> Result<PathBuf, RepoKnowledgeError> {
    path.canonicalize()
        .map_err(|source| RepoKnowledgeError::Read {
            path: path.to_owned(),
            source,
        })
}

/// `symlink_metadata`, with a missing path as `None` rather than an error.
fn link_metadata(path: &Path) -> Result<Option<std::fs::Metadata>, RepoKnowledgeError> {
    match std::fs::symlink_metadata(path) {
        Ok(m) => Ok(Some(m)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(RepoKnowledgeError::Read {
            path: path.to_owned(),
            source,
        }),
    }
}

struct Note {
    kind: MemoryKind,
    text: String,
    tags: Vec<String>,
}

/// One top-level markdown bullet is one fact:
///
/// ```text
/// - **[convention]** Run scripts/ci.sh, never the steps by hand. #build
/// ```
///
/// The kind is optional and defaults to `fact`; `#tag` words become tags.
/// Everything that is not a top-level bullet - headings, prose, indented
/// continuation - is prose for people and is ignored.
fn parse(text: &str) -> Vec<Note> {
    let mut notes = Vec::new();
    for line in text.lines() {
        // Indented bullets are sub-points of the line above, not facts.
        if line.starts_with(char::is_whitespace) {
            continue;
        }
        let Some(body) = line
            .strip_prefix("- ")
            .or_else(|| line.strip_prefix("* "))
            .map(str::trim)
        else {
            continue;
        };
        let (kind, rest) = split_kind(body);
        let mut tags = Vec::new();
        let mut words = Vec::new();
        for word in rest.split_whitespace() {
            match word.strip_prefix('#') {
                Some(tag) if !tag.is_empty() => tags.push(tag.trim_matches(',').to_lowercase()),
                _ => words.push(word),
            }
        }
        let text = words.join(" ").trim().to_owned();
        if text.is_empty() {
            continue;
        }
        notes.push(Note { kind, text, tags });
    }
    notes
}

/// `**[kind]**` or `[kind]` at the head of a bullet, if it names a real kind.
fn split_kind(body: &str) -> (MemoryKind, &str) {
    let stripped = body.strip_prefix("**").unwrap_or(body);
    let Some(rest) = stripped.strip_prefix('[') else {
        return (MemoryKind::Fact, body);
    };
    let Some(end) = rest.find(']') else {
        return (MemoryKind::Fact, body);
    };
    let kind = match rest[..end].trim().to_lowercase().as_str() {
        "fact" => MemoryKind::Fact,
        "decision" => MemoryKind::Decision,
        "lesson" | "pitfall" => MemoryKind::Lesson,
        "constraint" | "invariant" => MemoryKind::Constraint,
        "preference" | "convention" => MemoryKind::Preference,
        _ => return (MemoryKind::Fact, body),
    };
    let after = rest[end + 1..].trim_start();
    (kind, after.strip_prefix("**").unwrap_or(after).trim_start())
}

/// The commit whose content is the text being seeded, or `None`.
///
/// A label of `file@abc1234` is a promise that checking out that commit shows
/// this line. It does not hold for a file that is untracked, or tracked and
/// edited since: the last commit that touched such a file says something
/// else, or nothing. Those are the "version unknown" case, not a case for
/// naming the nearest commit.
fn commit_stating(root: &Path, relative: &str) -> Option<String> {
    let dirty = git(root, &["status", "--porcelain", "--", relative]).is_some();
    if dirty {
        return None;
    }
    git(root, &["log", "-1", "--format=%H", "--", relative])
}

/// This checkout's HEAD, or `None` when `root` is not a repository with
/// commits.
pub fn head_commit(root: &Path) -> Option<String> {
    git(root, &["rev-parse", "HEAD"])
}

/// Whether this checkout's history contains `commit`. A scope is shared by
/// every clone, so a note missing from a tree that never saw the commit it
/// was read at says nothing about whether the repository still states it.
pub fn contains_commit(root: &Path, commit: &str) -> bool {
    // Exits 0 and prints nothing, so the answer is the status, not stdout.
    git_status(root, &["merge-base", "--is-ancestor", commit, "HEAD"])
}

/// The facts `commit`'s committed content of `file` states. Empty when the
/// path is gone from that tree - the case a retraction exists for - and when
/// git cannot answer at all, which the caller has already excluded.
///
/// `<rev>:./<path>` resolves relative to `root`, so this holds when the
/// workspace root sits below the repository's top level.
pub fn notes_at(root: &Path, commit: &str, file: &str) -> BTreeSet<String> {
    git(root, &["show", &format!("{commit}:./{file}")])
        .map(|text| parse(&text).into_iter().map(|n| n.text).collect())
        .unwrap_or_default()
}

/// Run git in `root` and return whether it exited 0, for a question whose
/// answer is the exit code rather than the output.
fn git_status(root: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .args(["-c", "core.hooksPath=/dev/null", "-c", "core.fsmonitor="])
        .arg("--no-optional-locks")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .is_ok_and(|out| out.status.success())
}

/// Run git in `root` and return its trimmed stdout, or `None` when it fails
/// or says nothing. Git is provenance here, never control flow: a tree with
/// no git, or no commits, yields notes with an unknown version rather than no
/// notes.
///
/// The root may be a directory an agent wrote, so the repository's own
/// configuration gets no say: no hooks, no fsmonitor daemon, no index writes.
pub fn git(root: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .args(["-c", "core.hooksPath=/dev/null", "-c", "core.fsmonitor="])
        .arg("--no-optional-locks")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8(out.stdout).ok()?.trim().to_owned();
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests;
