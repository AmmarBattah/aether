use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// PATH installs first, then the per-user native CLI install directories.
/// Explicit paths bypass discovery and are preserved, including relative paths.
pub fn discover_programs(
    program: &str,
    search_path: Option<&OsStr>,
    home: Option<&Path>,
) -> Vec<PathBuf> {
    if program.is_empty() {
        return Vec::new();
    }
    if program.contains(std::path::MAIN_SEPARATOR) || program.contains('/') {
        return vec![PathBuf::from(program)];
    }
    let directories = search_path
        .into_iter()
        .flat_map(std::env::split_paths)
        .chain(
            home.into_iter()
                .flat_map(|h| [h.join(".local/bin"), h.join("bin")]),
        );
    let mut seen = HashSet::new();
    let candidates: Vec<_> = directories
        .filter_map(|dir| {
            let path = dir.join(program);
            if !executable(&path) {
                return None;
            }
            match path.canonicalize() {
                Ok(path) if seen.insert(path.clone()) => Some(path),
                Ok(_) => None,
                Err(error) => {
                    tracing::warn!(path = %path.display(), %error, "could not resolve CLI install");
                    None
                }
            }
        })
        .collect();
    assert!(
        candidates.iter().all(|path| path.is_absolute()),
        "discovered paths must be absolute"
    );
    assert_eq!(
        candidates.len(),
        seen.len(),
        "CLI installs must be distinct"
    );
    candidates
}

fn executable(path: &Path) -> bool {
    let metadata = match path.metadata() {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return false,
        Err(error) => {
            tracing::warn!(path = %path.display(), %error, "could not inspect CLI install");
            return false;
        }
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    metadata.is_file()
}
