use std::path::Path;

/// A copyable command for the selected daemon. Environment selection is
/// made explicit so the hint also works in another shell. `explicit` records
/// selection by a CLI flag, including a flag naming the default path.
/// Unprintable paths yield a shell comment rather than an executable command.
pub fn command_hint(socket: &Path, explicit: bool) -> String {
    let selected = explicit
        || std::env::var_os("AETHER_SOCKET").is_some()
        || socket != crate::paths::socket_path();
    format_command_hint(selected.then_some(socket))
}

fn format_command_hint(socket: Option<&Path>) -> String {
    let Some(socket) = socket else {
        return "aether".into();
    };
    let path = socket.as_os_str().as_encoded_bytes();
    // POSIX single quotes protect spaces and shell metacharacters. A path
    // that cannot be represented faithfully must never become a wrong target.
    let Ok(path) = std::str::from_utf8(path) else {
        return "# socket path is not UTF-8; reuse the original --socket argument".into();
    };
    if path.chars().any(char::is_control)
        || aether_core::text::strip_controls(path).as_ref() != path
    {
        return "# socket path contains terminal controls; reuse the original --socket argument"
            .into();
    }
    format!("aether --socket '{}'", path.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_command_hints_keep_the_plain_invocation() {
        assert_eq!(format_command_hint(None), "aether");
        assert_eq!(
            format_command_hint(Some(Path::new("/isolated/d.sock"))),
            "aether --socket '/isolated/d.sock'"
        );
        assert!(command_hint(&crate::paths::socket_path(), true).starts_with("aether --socket '"));
    }

    #[test]
    fn unprintable_socket_paths_never_become_executable_hints() {
        use std::os::unix::ffi::OsStrExt;
        for path in [
            Path::new("/socket\nwith-controls"),
            Path::new(std::ffi::OsStr::from_bytes(b"/socket-\xff")),
        ] {
            let hint = command_hint(path, true);
            assert!(hint.starts_with("# socket path"), "{hint}");
            assert!(!hint.contains(['\n', '\u{1b}']));
        }
    }
}
