//! Host programs the Linux network-denied sandboxes need. Claude's native
//! sandbox and Codex's loopback sandbox share them.

pub(crate) fn executable_on_path(program: &str, path: Option<&std::ffi::OsStr>) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.is_some_and(|path| {
        std::env::split_paths(path).any(|directory| {
            directory.join(program).metadata().is_ok_and(|metadata| {
                metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
            })
        })
    })
}

/// Why network-denied `engine` turns cannot run here, if `bwrap` or `socat`
/// is missing. Non-Linux hosts need neither.
pub(crate) fn dependency_limitation(
    engine: &str,
    has_executable: impl Fn(&str) -> bool,
) -> Option<String> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    let missing: Vec<_> = ["bwrap", "socat"]
        .into_iter()
        .filter(|program| !has_executable(program))
        .collect();
    (!missing.is_empty()).then(|| format!(
        "Network-denied {engine} turns require offline sandbox dependencies on PATH; missing: {}. Install the missing dependencies if you have host authority; otherwise delegate without network=false when offline execution is not required by the task. If offline execution is required and you cannot install them, call fail_task with this reason rather than waiting.",
        missing.join(", ")
    ))
}
