//! Authentication delegated to an official CLI (Claude Code, Codex).
//!
//! The CLI owns the credential; Aether only asks it whether it is logged in
//! and hands the user to its login flow. Nothing is impersonated: inference
//! for these providers also runs through the CLI (see the engine crate).

use std::collections::HashMap;
use std::path::PathBuf;

use futures::stream::{FuturesOrdered, StreamExt};

use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::auth::types::AuthError;

mod discovery;
pub use discovery::discover_programs;

/// Status and login commands of a credential-owning CLI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CliAuth {
    /// Program name or path, resolved through `PATH`.
    pub program: String,
    /// Arguments that print login status and exit 0.
    pub status: Vec<String>,
    /// Arguments that run the interactive login flow.
    pub login: Vec<String>,
    #[serde(default)]
    pub logout: Vec<String>,
    /// Substrings in the status output that mean "logged in".
    pub logged_in_markers: Vec<String>,
    /// Shown in login pickers, e.g. "Claude subscription (Claude Code)".
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliStatus {
    /// The program exists and answered.
    pub available: bool,
    pub logged_in: bool,
    /// Selected install, or the first candidate to use for login.
    pub program: Option<String>,
    /// One line of detail for the UI.
    pub detail: String,
}

const CACHE_TTL: Duration = Duration::from_secs(30);
/// "Not logged in" is re-checked quickly so a login just completed shows up.
const NEGATIVE_CACHE_TTL: Duration = Duration::from_secs(3);

#[derive(Default)]
struct ProbeCache {
    statuses: HashMap<String, (Instant, CliStatus)>,
    generation: Arc<()>,
}

fn cache() -> &'static Mutex<ProbeCache> {
    static CACHE: OnceLock<Mutex<ProbeCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(ProbeCache::default()))
}

fn lock_cache() -> MutexGuard<'static, ProbeCache> {
    cache().lock().unwrap_or_else(|error| {
        let mut guard = error.into_inner();
        guard.statuses.clear();
        guard.generation = Arc::new(());
        cache().clear_poison();
        tracing::error!("CLI auth cache poisoned; discarded cached probes");
        guard
    })
}

impl CliAuth {
    pub fn candidates(&self) -> Vec<PathBuf> {
        discover_programs(
            &self.program,
            std::env::var_os("PATH").as_deref(),
            dirs::home_dir().as_deref(),
        )
    }

    fn cache_key(&self, candidates: &[PathBuf]) -> String {
        format!(
            "{:?} {:?} {:?} {:?}",
            self.program, candidates, self.status, self.logged_in_markers
        )
    }

    /// Cached briefly for model listings; explicit AuthStatus requests invalidate it.
    pub async fn status(&self) -> CliStatus {
        self.status_on(self.candidates()).await
    }

    async fn status_on(&self, candidates: Vec<PathBuf>) -> CliStatus {
        if self.program.trim().is_empty()
            || self.logged_in_markers.is_empty()
            || self.logged_in_markers.iter().any(String::is_empty)
        {
            return CliStatus {
                available: false,
                logged_in: false,
                program: None,
                detail:
                    "invalid CLI auth configuration: program and login markers must be nonempty"
                        .into(),
            };
        }
        let key = self.cache_key(&candidates);
        let generation = {
            let cache = lock_cache();
            if let Some((at, status)) = cache.statuses.get(&key) {
                let ttl = if status.logged_in {
                    CACHE_TTL
                } else {
                    NEGATIVE_CACHE_TTL
                };
                if at.elapsed() < ttl {
                    return status.clone();
                }
            }
            cache.generation.clone()
        };
        let status = self
            .probe_candidates(candidates, Duration::from_secs(15))
            .await;
        assert!(
            !status.logged_in || status.available,
            "logged-in CLI must be available"
        );
        assert!(
            !status.logged_in || status.program.is_some(),
            "logged-in CLI must have a selected path"
        );
        let mut cache = lock_cache();
        if Arc::ptr_eq(&cache.generation, &generation) {
            cache.statuses.insert(key, (Instant::now(), status.clone()));
        }
        status
    }

    #[cfg(test)]
    async fn status_uncached(&self) -> CliStatus {
        self.probe_candidates(self.candidates(), Duration::from_secs(15))
            .await
    }

    async fn probe_candidates(&self, candidates: Vec<PathBuf>, timeout: Duration) -> CliStatus {
        assert!(!timeout.is_zero(), "probe timeout must be positive");
        assert!(!self.program.is_empty(), "CLI program must be configured");
        let mut probes: FuturesOrdered<_> = candidates
            .iter()
            .map(|path| self.probe(path, timeout))
            .collect();
        let mut failed = Vec::new();
        let mut available = false;
        let mut program = None;
        while let Some(status) = probes.next().await {
            if status.logged_in {
                return status;
            }
            available |= status.available;
            if program.is_none() {
                program = status.program;
            }
            failed.push(status.detail);
        }
        let detail = if failed.is_empty() {
            format!("{} not found in PATH, ~/.local/bin or ~/bin", self.program)
        } else {
            let suffix = if failed.len() > 1 {
                " (other installs exist)"
            } else {
                ""
            };
            format!("{}{suffix}", failed.join("; "))
        };
        CliStatus {
            available,
            logged_in: false,
            program,
            detail: detail.replace(['\n', '\r'], " "),
        }
    }

    async fn probe(&self, path: &std::path::Path, timeout: Duration) -> CliStatus {
        assert!(!path.as_os_str().is_empty(), "probe path cannot be empty");
        assert!(
            !self.logged_in_markers.is_empty(),
            "CLI login markers must be configured"
        );
        let program = match std::path::absolute(path) {
            Ok(program) => program.to_string_lossy().into_owned(),
            Err(error) => {
                return CliStatus {
                    available: false,
                    logged_in: false,
                    program: None,
                    detail: format!(
                        "{}: could not resolve program path ({error})",
                        path.display()
                    ),
                };
            }
        };
        let output = tokio::time::timeout(
            timeout,
            tokio::process::Command::new(path)
                .args(&self.status)
                .stdin(std::process::Stdio::null())
                .kill_on_drop(true)
                .output(),
        )
        .await;
        let (available, logged_in, reason) = match output {
            Ok(Ok(out)) => {
                let text = format!(
                    "{}{}",
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr)
                );
                let logged_in =
                    out.status.success() && self.logged_in_markers.iter().any(|m| text.contains(m));
                let reason = if !out.status.success() {
                    format!("non-zero exit ({})", out.status)
                } else if logged_in {
                    "logged in".into()
                } else {
                    "not logged in".into()
                };
                (true, logged_in, reason)
            }
            Ok(Err(e)) => (false, false, format!("not found or could not start ({e})")),
            Err(_) => (true, false, "status timed out".into()),
        };
        CliStatus {
            available,
            logged_in,
            detail: format!("{program}: {reason}").replace(['\n', '\r'], " "),
            program: Some(program),
        }
    }

    /// Run the CLI's own login flow attached to the current terminal.
    pub async fn login(&self) -> Result<(), AuthError> {
        self.run_inherit(&self.login).await?;
        self.forget();
        Ok(())
    }

    pub async fn logout(&self) -> Result<(), AuthError> {
        if self.logout.is_empty() {
            return Err(AuthError::Flow(format!(
                "{} has no logout command",
                self.program
            )));
        }
        self.run_inherit(&self.logout).await?;
        self.forget();
        Ok(())
    }

    async fn run_inherit(&self, args: &[String]) -> Result<(), AuthError> {
        let status = tokio::process::Command::new(&self.program)
            .args(args)
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status()
            .await
            .map_err(|e| AuthError::Flow(format!("could not run {}: {e}", self.program)))?;
        if !status.success() {
            return Err(AuthError::Flow(format!(
                "{} {} exited with {status}",
                self.program,
                args.join(" ")
            )));
        }
        Ok(())
    }

    /// Drop the cached status so the next check reflects a fresh login.
    pub fn forget(&self) {
        let mut cache = lock_cache();
        cache.generation = Arc::new(());
        let prefix = format!("{:?} ", self.program);
        cache.statuses.retain(|key, _| !key.starts_with(&prefix));
    }
}

#[cfg(all(test, unix))]
#[path = "cli/tests.rs"]
mod install_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn fake(script: &str) -> CliAuth {
        CliAuth {
            program: "sh".into(),
            status: vec!["-c".into(), script.into()],
            login: vec![],
            logout: vec![],
            logged_in_markers: vec!["\"loggedIn\": true".into(), "Logged in".into()],
            label: "fake".into(),
        }
    }

    #[tokio::test]
    async fn parses_logged_in_and_out() {
        let yes = fake("printf '{\"loggedIn\": true, \"email\": \"x\"}'");
        let s = yes.status().await;
        assert!(s.available && s.logged_in, "{s:?}");
        let no = fake("printf 'Not logged in'; exit 0");
        assert!(!no.status().await.logged_in);
        let failing = fake("printf 'Logged in'; exit 3");
        assert!(
            !failing.status().await.logged_in,
            "exit code must fail the check"
        );
    }

    #[tokio::test]
    async fn missing_program_is_unavailable() {
        let missing = CliAuth {
            program: "definitely-not-a-real-binary-aether".into(),
            ..fake("true")
        };
        let s = missing.status().await;
        assert!(!s.available && !s.logged_in);
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn cancelled_status_probe_does_not_outlive_its_caller() {
        let directory = tempfile::tempdir().unwrap();
        let pid_file = directory.path().join("probe.pid");
        let auth = CliAuth {
            status: vec![
                "-c".into(),
                "echo $$ > \"$1\"; exec sleep 30".into(),
                "probe".into(),
                pid_file.to_string_lossy().into_owned(),
            ],
            ..fake("true")
        };
        let probe = tokio::spawn(async move { auth.status_uncached().await });
        let started = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let Ok(text) = std::fs::read_to_string(&pid_file)
                    && let Ok(pid) = text.trim().parse::<u32>()
                {
                    break pid;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await;
        probe.abort();
        assert!(probe.await.unwrap_err().is_cancelled());
        let pid = started.expect("probe published its process id");
        let process = std::path::PathBuf::from(format!("/proc/{pid}"));
        let reaped = tokio::time::timeout(Duration::from_secs(5), async {
            while process.exists() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await;
        if reaped.is_err() {
            // Clean up only this fixture's process before reporting the regression.
            let status = tokio::process::Command::new("kill")
                .args(["-KILL", &pid.to_string()])
                .status()
                .await
                .expect("kill surviving fixture");
            assert!(status.success(), "could not clean up fixture {pid}");
        }
        assert!(reaped.is_ok(), "cancelled CLI probe {pid} survived");
    }
}
