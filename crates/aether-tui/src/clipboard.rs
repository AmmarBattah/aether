//! Bounded clipboard delivery through the terminal and optional local helpers.

use std::io::Write;
use std::process::Stdio;
use std::time::Duration;

use base64::Engine;
use tokio::io::AsyncWriteExt;

use crate::ui::selection::MAX_COPY_BYTES;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Helper {
    program: &'static str,
    args: &'static [&'static str],
}

pub(crate) fn helpers(macos: bool, ssh: bool, wayland: bool, display: bool) -> Vec<Helper> {
    if ssh {
        return Vec::new();
    }
    if macos {
        return vec![Helper {
            program: "pbcopy",
            args: &[],
        }];
    }
    if wayland {
        return vec![Helper {
            program: "wl-copy",
            args: &[],
        }];
    }
    if display {
        return vec![
            Helper {
                program: "xclip",
                args: &["-selection", "clipboard"],
            },
            Helper {
                program: "xsel",
                args: &["--clipboard", "--input"],
            },
        ];
    }
    Vec::new()
}

pub(crate) fn osc52(text: &str) -> Result<String, String> {
    if text.len() > MAX_COPY_BYTES {
        return Err("selection exceeds the 1 MiB clipboard limit".into());
    }
    Ok(format!(
        "\x1b]52;c;{}\x07",
        base64::engine::general_purpose::STANDARD.encode(text)
    ))
}

async fn native_copy(text: &str, helpers: &[Helper]) -> Result<bool, String> {
    native_copy_with(helpers, |helper| run_helper(text, helper)).await
}

async fn native_copy_with<F, Fut>(helpers: &[Helper], mut run: F) -> Result<bool, String>
where
    F: FnMut(Helper) -> Fut,
    Fut: std::future::Future<Output = Result<bool, String>>,
{
    let mut failures = Vec::new();
    for &helper in helpers {
        match run(helper).await {
            Ok(true) => return Ok(true),
            Ok(false) => continue,
            Err(error) => failures.push(format!("{}: {error}", helper.program)),
        }
    }
    if failures.is_empty() {
        Ok(false)
    } else {
        Err(failures.join("; "))
    }
}

async fn run_helper(text: &str, helper: Helper) -> Result<bool, String> {
    let mut child = match tokio::process::Command::new(helper.program)
        .args(helper.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
    {
        Ok(child) => child,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(format!("{}: {e}", helper.program)),
    };
    let result = tokio::time::timeout(Duration::from_secs(2), async {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| "clipboard helper has no stdin".to_owned())?;
        stdin
            .write_all(text.as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        drop(stdin);
        let status = child.wait().await.map_err(|e| e.to_string())?;
        if !status.success() {
            return Err(format!("{} exited with {status}", helper.program));
        }
        Ok(true)
    })
    .await;
    match result {
        Ok(result) => result,
        Err(_) => Err(format!("{} timed out", helper.program)),
    }
}

fn osc52_reliable() -> bool {
    let term = std::env::var("TERM").unwrap_or_default();
    let program = std::env::var("TERM_PROGRAM").unwrap_or_default();
    ["kitty", "foot", "alacritty", "wezterm"]
        .iter()
        .any(|name| term.contains(name))
        || matches!(program.as_str(), "iTerm.app" | "WezTerm" | "vscode")
        || std::env::var_os("WT_SESSION").is_some()
}

pub(crate) fn send_osc52(text: &str, out: &mut impl Write) -> Result<(), String> {
    let sequence = osc52(text)?;
    out.write_all(sequence.as_bytes())
        .and_then(|()| out.flush())
        .map_err(|e| e.to_string())
}

pub(crate) async fn copy(
    text: String,
    terminal: Result<(), String>,
) -> Result<(usize, bool), String> {
    if text.len() > MAX_COPY_BYTES {
        return Err("selection exceeds the 1 MiB clipboard limit".into());
    }
    let helpers = helpers(
        cfg!(target_os = "macos"),
        std::env::var_os("SSH_CONNECTION").is_some() || std::env::var_os("SSH_TTY").is_some(),
        std::env::var_os("WAYLAND_DISPLAY").is_some(),
        std::env::var_os("DISPLAY").is_some(),
    );
    let native = native_copy(&text, &helpers).await;
    let reliable = terminal.is_ok() && osc52_reliable();
    // OSC 52 has no acknowledgement; unknown terminals report "sent".
    copy_outcome(text.lines().count(), terminal, native, reliable)
}

fn copy_outcome(
    lines: usize,
    terminal: Result<(), String>,
    native: Result<bool, String>,
    reliable: bool,
) -> Result<(usize, bool), String> {
    match (terminal, native) {
        (_, Ok(true)) => Ok((lines, true)),
        (Ok(()), native) => {
            if let Err(error) = native {
                tracing::warn!("local clipboard helpers failed; OSC 52 sent: {error}");
            }
            Ok((lines, reliable))
        }
        (Err(terminal), native) => Err(match native {
            Err(native) => format!("terminal: {terminal}; local clipboard: {native}"),
            _ => format!("terminal: {terminal}; no local clipboard helper found"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn missing_helpers_still_report_osc52_sent() {
        let local = helpers(false, false, false, true);
        let native = native_copy_with(&local, |_| std::future::ready(Ok(false))).await;
        assert_eq!(copy_outcome(2, Ok(()), native, false), Ok((2, false)));
    }

    #[tokio::test]
    async fn failing_helper_falls_through_to_the_next() {
        let local = helpers(false, false, false, true);
        let mut tried = Vec::new();
        let native = native_copy_with(&local, |helper| {
            tried.push(helper.program);
            std::future::ready(if helper.program == "xclip" {
                Err("xclip failed".into())
            } else {
                Ok(true)
            })
        })
        .await;
        assert_eq!(native, Ok(true));
        assert_eq!(tried, ["xclip", "xsel"]);
    }

    #[test]
    fn clipboard_outcome_matrix_reports_delivery_and_preserves_failures() {
        for reliable in [false, true] {
            for native in [Ok(true), Ok(false), Err("helper failed".into())] {
                assert_eq!(
                    copy_outcome(3, Ok(()), native.clone(), reliable),
                    Ok((3, native == Ok(true) || reliable)),
                );
                let outcome =
                    copy_outcome(3, Err("terminal failed".into()), native.clone(), reliable);
                if native == Ok(true) {
                    assert_eq!(outcome, Ok((3, true)));
                } else {
                    assert!(outcome.is_err_and(|error| error.contains("terminal failed")));
                }
            }
        }
    }

    #[tokio::test]
    async fn exhausted_helpers_keep_every_failure_visible() {
        let local = helpers(false, false, false, true);
        let mut tried = Vec::new();
        let result = native_copy_with(&local, |helper| {
            tried.push(helper.program);
            std::future::ready(Err(format!("{} unavailable", helper.program)))
        })
        .await;
        let error = result.unwrap_err();
        assert_eq!(tried, ["xclip", "xsel"]);
        assert!(error.contains("xclip unavailable"));
        assert!(error.contains("xsel unavailable"));
    }

    #[test]
    fn osc52_write_and_flush_errors_are_visible_and_oversize_never_writes() {
        struct FailedWriter {
            fail_write: bool,
            writes: usize,
        }
        impl Write for FailedWriter {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.writes += 1;
                if self.fail_write {
                    Err(std::io::Error::other("write failed"))
                } else {
                    Ok(bytes.len())
                }
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Err(std::io::Error::other("flush failed"))
            }
        }
        for fail_write in [true, false] {
            let mut out = FailedWriter {
                fail_write,
                writes: 0,
            };
            assert!(
                send_osc52("text", &mut out).is_err_and(|error| error.contains(if fail_write {
                    "write failed"
                } else {
                    "flush failed"
                }))
            );
            assert_eq!(out.writes, 1);
            out.writes = 0;
            assert!(send_osc52(&"x".repeat(MAX_COPY_BYTES + 1), &mut out).is_err());
            assert_eq!(out.writes, 0);
        }
    }

    #[test]
    fn commands_follow_the_local_display_and_skip_remote_hosts() {
        assert!(helpers(false, true, true, true).is_empty());
        assert_eq!(helpers(true, false, true, true)[0].program, "pbcopy");
        assert_eq!(helpers(false, false, true, true)[0].program, "wl-copy");
        let x = helpers(false, false, false, true);
        assert_eq!(
            x[0],
            Helper {
                program: "xclip",
                args: &["-selection", "clipboard"]
            }
        );
        assert_eq!(
            x[1],
            Helper {
                program: "xsel",
                args: &["--clipboard", "--input"]
            }
        );
        assert!(helpers(false, false, false, false).is_empty());
    }

    #[test]
    fn osc52_encodes_unicode_and_rejects_oversize_payloads() {
        for (plain, encoded) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("hello world", "aGVsbG8gd29ybGQ="),
        ] {
            assert_eq!(osc52(plain).unwrap(), format!("\x1b]52;c;{encoded}\x07"));
        }
        assert_eq!(osc52("é").unwrap(), "\x1b]52;c;w6k=\x07");
        assert!(osc52(&"x".repeat(MAX_COPY_BYTES + 1)).is_err());
    }
}

#[cfg(all(test, unix))]
mod process_tests {
    use super::*;

    #[tokio::test]
    async fn helper_failures_and_timeouts_are_visible() {
        let failed = native_copy(
            "text",
            &[Helper {
                program: "/bin/false",
                args: &[],
            }],
        )
        .await;
        assert!(failed.is_err());
        let timed_out = native_copy(
            "text",
            &[Helper {
                program: "/bin/sleep",
                args: &["10"],
            }],
        )
        .await;
        assert!(timed_out.is_err_and(|e| e.contains("timed out")));
        assert!(native_copy("text", &[]).await.is_ok_and(|copied| !copied));
        assert!(
            native_copy(
                "text",
                &[Helper {
                    program: "/nonexistent/aether-clipboard-helper",
                    args: &[]
                }]
            )
            .await
            .is_ok_and(|copied| !copied)
        );
    }
}
