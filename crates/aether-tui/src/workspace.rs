//! Workspaces as the operator picks them: one identity per directory, short
//! labels, git projects under the discovery roots, and typed-path completion.
//! Reads the filesystem only in bounded amounts; an agent's own workspace
//! never changes.

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

/// Filesystem calls discovery may make, all depths together.
const DISCOVERY_CALLS: usize = 2048;
/// Projects discovery reports.
const MAX_FOUND: usize = 40;
/// Filesystem calls reading one directory for completion may make.
const LISTING_CALLS: usize = 2048;
/// Directory checks one keystroke's completion may make.
const COMPLETION_PROBES: usize = 128;
/// Sub-directories a typed path completes to.
const MAX_COMPLETIONS: usize = 50;

/// Filesystem calls one picker action may make: directories opened, entries
/// read and metadata looked up. Once spent, the rest is skipped, not waited on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Budget {
    left: usize,
    used: usize,
}

impl Budget {
    const fn new(calls: usize) -> Budget {
        Budget {
            left: calls,
            used: 0,
        }
    }

    /// Take one call; `false` once none are left.
    fn spend(&mut self) -> bool {
        if self.left == 0 {
            return false;
        }
        self.left -= 1;
        self.used += 1;
        true
    }

    #[cfg(test)]
    fn used(&self) -> usize {
        self.used
    }
}

/// The operator's home, for `~` and discovery. Tests inject their own.
pub fn home_dir() -> Option<PathBuf> {
    dirs::home_dir()
}

/// One identity per directory: symlinks and `..` resolved, or the path as
/// given when it cannot be resolved (gone, unreadable).
pub fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// `~/src/aether` for a path under `home`, else the path as it is.
pub fn display(path: &Path, home: Option<&Path>) -> String {
    match home.and_then(|home| path.strip_prefix(home).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

/// The path's parts as [`display`] writes them, so `~` counts as one.
fn parts(path: &Path, home: Option<&Path>) -> Vec<String> {
    display(path, home)
        .split('/')
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect()
}

/// A short label: the directory's name, `~` for home, `/` for the root.
pub fn shorthand(path: &Path, home: Option<&Path>) -> String {
    if home == Some(path) {
        return "~".to_owned();
    }
    match path.file_name() {
        Some(name) => name.to_string_lossy().into_owned(),
        None => display(path, home),
    }
}

/// [`shorthand`] for each of `paths`, except that paths sharing a name get
/// the shortest trailing suffix that tells them apart (`wt/aether`, `~/aether`).
pub fn shorthands(paths: &[PathBuf], home: Option<&Path>) -> HashMap<PathBuf, String> {
    let mut by_name: HashMap<String, Vec<&PathBuf>> = HashMap::new();
    for path in paths {
        let group = by_name.entry(shorthand(path, home)).or_default();
        if !group.contains(&path) {
            group.push(path);
        }
    }
    let mut labels = HashMap::new();
    for (name, group) in by_name {
        if group.len() == 1 {
            labels.insert(group[0].clone(), name);
            continue;
        }
        let split: Vec<Vec<String>> = group.iter().map(|p| parts(p, home)).collect();
        for (at, path) in group.iter().enumerate() {
            labels.insert((*path).clone(), distinct_suffix(&split, at, path, home));
        }
    }
    labels
}

/// The shortest suffix of `split[at]` no other entry ends with; the whole
/// display form once the parts run out.
fn distinct_suffix(split: &[Vec<String>], at: usize, path: &Path, home: Option<&Path>) -> String {
    let own = &split[at];
    let suffix = |parts: &[String], n: usize| parts[parts.len().saturating_sub(n)..].join("/");
    for n in 1..=own.len() {
        let mine = suffix(own, n);
        let shared = split
            .iter()
            .enumerate()
            .any(|(other, parts)| other != at && suffix(parts, n) == mine);
        if !shared {
            return mine;
        }
    }
    display(path, home)
}

/// Levels below a root discovery searches when `ui.toml` does not say.
const DEFAULT_DEPTH: usize = 2;
/// Deepest search `workspace_depth` may ask for; deeper costs budget for little.
const MAX_DEPTH: usize = 4;

/// Where the picker looks for git projects: `ui.toml`'s `workspace_roots`
/// (home when absent) to `workspace_depth` levels down.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Discovery {
    /// Absolute, as written; resolved to one identity while searching.
    roots: Vec<PathBuf>,
    /// Configured roots that name no absolute path (relative, `~user`).
    unanchored: Vec<String>,
    /// Whether the roots came from `ui.toml`; only those are reported missing.
    configured: bool,
    depth: usize,
    /// Searched but never itself a project, as before roots were configurable.
    home: Option<PathBuf>,
}

/// What a search found, and the configured roots it could not search.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Found {
    /// Canonical, shallowest first.
    pub projects: Vec<PathBuf>,
    /// Each once, as [`display`] writes it or as written when unanchored.
    pub missing: Vec<String>,
}

impl Discovery {
    /// Roots as written, `~` expanded; one that names no absolute path is
    /// not searched. The depth is clamped to `1..=MAX_DEPTH`.
    pub fn new(roots: Option<&[String]>, depth: Option<usize>, home: Option<&Path>) -> Discovery {
        let mut discovery = Discovery {
            roots: Vec::new(),
            unanchored: Vec::new(),
            configured: roots.is_some(),
            depth: depth.unwrap_or(DEFAULT_DEPTH).clamp(1, MAX_DEPTH),
            home: home.map(Path::to_path_buf),
        };
        let Some(written) = roots else {
            discovery.roots.extend(home.map(Path::to_path_buf));
            return discovery;
        };
        let mut unanchored = HashSet::new();
        for root in written {
            match resolve(root, home, None) {
                Some(path) => discovery.roots.push(path),
                None if unanchored.insert(root.as_str()) => discovery.unanchored.push(root.clone()),
                None => {}
            }
        }
        discovery
    }

    /// Git projects under the roots, skipping hidden directories and symlinks
    /// below them and never looking inside one found; bounded so a huge tree
    /// costs a few milliseconds at most.
    pub fn search(&self) -> Found {
        self.within(&mut Budget::new(DISCOVERY_CALLS))
    }

    /// Breadth first across every root at once, so when the budget runs out
    /// the shallow projects are the ones found.
    fn within(&self, budget: &mut Budget) -> Found {
        let mut found = Found::default();
        let home = self
            .home
            .as_ref()
            .filter(|_| budget.spend())
            .map(|home| canonical(home));
        let mut written = HashSet::new();
        let mut seen = HashSet::new();
        let mut level = Vec::new();
        for root in &self.roots {
            if found.projects.len() >= MAX_FOUND {
                break;
            }
            // A repeated spelling costs nothing; an alias, two calls to resolve.
            if !written.insert(root) {
                continue;
            }
            if !(budget.spend() && budget.spend()) {
                break;
            }
            let Some(real) = std::fs::canonicalize(root)
                .ok()
                .filter(|real| real.is_dir())
            else {
                if self.configured {
                    found.missing.push(display(root, self.home.as_deref()));
                }
                continue;
            };
            if !seen.insert(real.clone()) {
                continue;
            }
            if Some(&real) != home.as_ref() && is_repository(&real, budget) {
                found.projects.push(real);
            } else {
                level.push(real);
            }
        }
        found.missing.extend(self.unanchored.iter().cloned());
        if found.projects.len() >= MAX_FOUND {
            return found;
        }
        for _ in 0..self.depth {
            let mut next = Vec::new();
            for dir in &level {
                for sub in subdirs(dir, budget) {
                    if found.projects.len() >= MAX_FOUND {
                        return found;
                    }
                    if !seen.insert(sub.clone()) {
                        continue;
                    }
                    if Some(&sub) != home.as_ref() && is_repository(&sub, budget) {
                        found.projects.push(sub);
                    } else {
                        next.push(sub);
                    }
                }
            }
            level = next;
        }
        found
    }
}

fn is_repository(dir: &Path, budget: &mut Budget) -> bool {
    // A worktree or submodule has a `.git` file rather than a directory.
    budget.spend() && dir.join(".git").symlink_metadata().is_ok()
}

/// Visible, real (not symlinked) sub-directories of `dir`, by name. An unreadable directory has none
/// to offer: these are suggestions, and the typed path is checked on use.
fn subdirs(dir: &Path, budget: &mut Budget) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = read_entries(dir, budget)
        .into_iter()
        .filter(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
        .filter(|entry| budget.spend() && entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.path())
        .collect();
    dirs.sort();
    dirs
}

/// The entries of `dir`, as many as `budget` allows: a call to open it and
/// one per entry read. Unreadable ones are skipped; these are suggestions.
fn read_entries(dir: &Path, budget: &mut Budget) -> Vec<std::fs::DirEntry> {
    if !budget.spend() {
        return Vec::new();
    }
    let Ok(mut entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    while budget.spend() {
        match entries.next() {
            Some(Ok(entry)) => out.push(entry),
            Some(Err(_)) => {}
            None => break,
        }
    }
    out
}

/// Typed text the picker reads as a path rather than a filter.
pub fn looks_like_path(text: &str) -> bool {
    text.starts_with('/')
        || text.starts_with('~')
        || text.starts_with("./")
        || text.starts_with("../")
}

/// `text` as an absolute path: `~` expanded, a relative path taken from
/// `base`, `.` and `..` folded. `None` when there is nothing to anchor it to.
pub fn resolve(text: &str, home: Option<&Path>, base: Option<&Path>) -> Option<PathBuf> {
    let path = if text == "~" {
        home?.to_path_buf()
    } else if let Some(rest) = text.strip_prefix("~/") {
        home?.join(rest)
    } else if text.starts_with('~') {
        // `~user` would need the password database; say no rather than guess.
        return None;
    } else if Path::new(text).is_absolute() {
        PathBuf::from(text)
    } else {
        base?.join(text)
    };
    Some(fold(&path))
}

/// A path `ui.toml` lists, `~` allowed, as the directory it names.
pub fn pinned_path(text: &str, home: Option<&Path>) -> Option<PathBuf> {
    resolve(text, home, None).map(|path| canonical(&path))
}

/// `.` and `..` folded lexically, the way a shell's `cd` reads them.
fn fold(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

/// What a typed path offers: itself when it is a directory, then the
/// sub-directories of its parent that start with its last segment.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Completion {
    pub exact: Option<PathBuf>,
    pub candidates: Vec<PathBuf>,
}

/// `listing` caches the parent directory between keystrokes; one for
/// another directory is replaced.
pub fn complete(
    text: &str,
    home: Option<&Path>,
    base: Option<&Path>,
    listing: &mut Option<Listing>,
) -> Completion {
    let mut probes = Budget::new(COMPLETION_PROBES);
    let exact = resolve(text, home, base).filter(|path| probes.spend() && path.is_dir());
    // Split the text, not the folded path: `~/.` lists hidden directories of home.
    let (dir_text, prefix) = match text.rfind('/') {
        Some(at) => (&text[..=at], &text[at + 1..]),
        None => (text, ""),
    };
    let Some(parent) = resolve(dir_text, home, base) else {
        return Completion::default();
    };
    if listing.as_ref().is_some_and(|known| known.dir != parent) {
        *listing = None;
    }
    let listing =
        listing.get_or_insert_with(|| Listing::read(&parent, &mut Budget::new(LISTING_CALLS)));
    // One spare, for `exact` among them.
    let mut candidates = listing.dirs(prefix, MAX_COMPLETIONS + 1, &mut probes);
    candidates.retain(|dir| Some(dir) != exact.as_ref());
    candidates.truncate(MAX_COMPLETIONS);
    Completion { exact, candidates }
}

/// One directory's entries by name, read once while the picker is open.
/// Whether each is a directory is looked up only when a typed prefix asks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listing {
    dir: PathBuf,
    /// By name; `None` until looked up.
    entries: Vec<(OsString, Option<bool>)>,
}

impl Listing {
    fn read(dir: &Path, budget: &mut Budget) -> Listing {
        let mut entries: Vec<(OsString, Option<bool>)> = read_entries(dir, budget)
            .into_iter()
            .map(|entry| (entry.file_name(), None))
            .collect();
        entries.sort();
        Listing {
            dir: dir.to_path_buf(),
            entries,
        }
    }

    /// Up to `limit` sub-directories whose names start with `prefix`, hidden
    /// ones only for a hidden prefix. A symlink to a directory is somewhere to
    /// go, so links are followed, but only for names that match.
    fn dirs(&mut self, prefix: &str, limit: usize, budget: &mut Budget) -> Vec<PathBuf> {
        let hidden = prefix.starts_with('.');
        let mut out = Vec::new();
        for (name, is_dir) in &mut self.entries {
            if out.len() >= limit {
                break;
            }
            let text = name.to_string_lossy();
            if !text.starts_with(prefix) || (!hidden && text.starts_with('.')) {
                continue;
            }
            let path = self.dir.join(&*name);
            if is_dir.is_none() {
                if !budget.spend() {
                    break;
                }
                *is_dir = Some(path.is_dir());
            }
            if *is_dir == Some(true) {
                out.push(path);
            }
        }
        out
    }
}

/// A directory whose path is not UTF-8. The daemon and `ui.toml` take UTF-8
/// paths only, and a lossy spelling could name a different directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotUtf8(pub PathBuf);

impl std::fmt::Display for NotUtf8 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "workspace path is not UTF-8, so agents cannot use it: {}",
            self.0.display()
        )
    }
}

impl std::error::Error for NotUtf8 {}

/// What Tab writes for `chosen`: the typed text up to its last `/`, then the
/// chosen name and a `/`, so the operator keeps descending in their own spelling.
pub fn completed_text(
    typed: &str,
    chosen: &Path,
    home: Option<&Path>,
    base: Option<&Path>,
) -> Result<String, NotUtf8> {
    if chosen.to_str().is_none() {
        return Err(NotUtf8(chosen.to_path_buf()));
    }
    if !looks_like_path(typed) {
        return Ok(with_slash(display(chosen, home)));
    }
    if resolve(typed, home, base).as_deref() == Some(chosen) {
        return Ok(with_slash(typed.to_owned()));
    }
    let stem = match typed.rfind('/') {
        Some(at) => &typed[..=at],
        None => "~/",
    };
    let name = chosen
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    Ok(with_slash(format!("{stem}{name}")))
}

fn with_slash(mut text: String) -> String {
    if !text.ends_with('/') {
        text.push('/');
    }
    text
}

#[cfg(test)]
mod tests;
