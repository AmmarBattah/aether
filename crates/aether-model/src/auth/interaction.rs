//! How a login flow talks to a person: prompts and notifications, decoupled
//! from any particular UI.

use super::types::AuthError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectOption {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthPrompt {
    Text {
        message: String,
        placeholder: Option<String>,
    },
    Secret {
        message: String,
    },
    Select {
        message: String,
        options: Vec<SelectOption>,
    },
    /// The user may paste an authorization code or the full redirect URL.
    ManualCode {
        message: String,
        placeholder: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthEvent {
    Info {
        message: String,
    },
    AuthUrl {
        url: String,
        instructions: String,
    },
    DeviceCode {
        user_code: String,
        verification_uri: String,
        interval_secs: u64,
        expires_in_secs: u64,
    },
    Progress {
        message: String,
    },
}

#[async_trait::async_trait]
pub trait AuthInteraction: Send + Sync {
    /// Returns the entered text, or the selected option id for `Select`.
    async fn prompt(&self, prompt: AuthPrompt) -> Result<String, AuthError>;
    fn notify(&self, event: AuthEvent);
}

/// Stdio-based interaction for `aether login`.
#[derive(Debug, Clone)]
pub struct TerminalInteraction {
    pub open_browser: bool,
}

impl Default for TerminalInteraction {
    fn default() -> Self {
        Self { open_browser: true }
    }
}

#[async_trait::async_trait]
impl AuthInteraction for TerminalInteraction {
    async fn prompt(&self, prompt: AuthPrompt) -> Result<String, AuthError> {
        tokio::task::spawn_blocking(move || match prompt {
            AuthPrompt::Secret { message } => rpassword::prompt_password(format!("{message}: "))
                .map_err(|e| AuthError::Flow(e.to_string())),
            AuthPrompt::Text {
                message,
                placeholder,
            }
            | AuthPrompt::ManualCode {
                message,
                placeholder,
            } => {
                match placeholder {
                    Some(p) => eprint!("{message} [{p}]: "),
                    None => eprint!("{message}: "),
                }
                read_line()
            }
            AuthPrompt::Select { message, options } => {
                eprintln!("{message}");
                for (i, opt) in options.iter().enumerate() {
                    eprintln!("  {}) {}", i + 1, opt.label);
                }
                eprint!("Choice [1]: ");
                let line = read_line()?;
                let choice = if line.trim().is_empty() {
                    1
                } else {
                    line.trim()
                        .parse::<usize>()
                        .map_err(|_| AuthError::Flow(format!("not a number: {line}")))?
                };
                options
                    .get(choice.saturating_sub(1))
                    .map(|o| o.id.clone())
                    .ok_or_else(|| AuthError::Flow(format!("no option {choice}")))
            }
        })
        .await
        .map_err(|e| AuthError::Flow(format!("prompt task: {e}")))?
    }

    fn notify(&self, event: AuthEvent) {
        match event {
            AuthEvent::Info { message } | AuthEvent::Progress { message } => eprintln!("{message}"),
            AuthEvent::AuthUrl { url, instructions } => {
                eprintln!("{instructions}\n\n  {url}\n");
                if self.open_browser
                    && let Err(e) = open::that_detached(&url)
                {
                    eprintln!("(could not open a browser: {e})");
                }
            }
            AuthEvent::DeviceCode {
                user_code,
                verification_uri,
                expires_in_secs,
                ..
            } => {
                eprintln!(
                    "Open {verification_uri} and enter the code: {user_code}\n(expires in {} minutes)",
                    expires_in_secs / 60
                );
                if self.open_browser {
                    let _ = open::that_detached(&verification_uri);
                }
            }
        }
    }
}

fn read_line() -> Result<String, AuthError> {
    use std::io::Write;
    let _ = std::io::stderr().flush();
    let mut line = String::new();
    std::io::stdin()
        .read_line(&mut line)
        .map_err(|e| AuthError::Flow(e.to_string()))?;
    Ok(line.trim().to_owned())
}

/// A scripted interaction for tests: answers prompts in order and records events.
#[derive(Debug, Default)]
pub struct ScriptedInteraction {
    answers: std::sync::Mutex<std::collections::VecDeque<String>>,
    pub events: std::sync::Mutex<Vec<AuthEvent>>,
    pub prompts: std::sync::Mutex<Vec<AuthPrompt>>,
}

impl ScriptedInteraction {
    pub fn new(answers: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            answers: std::sync::Mutex::new(answers.into_iter().map(Into::into).collect()),
            events: Default::default(),
            prompts: Default::default(),
        }
    }
}

#[async_trait::async_trait]
impl AuthInteraction for ScriptedInteraction {
    async fn prompt(&self, prompt: AuthPrompt) -> Result<String, AuthError> {
        self.prompts
            .lock()
            .map_err(|_| AuthError::Flow("poisoned".into()))?
            .push(prompt);
        self.answers
            .lock()
            .map_err(|_| AuthError::Flow("poisoned".into()))?
            .pop_front()
            .ok_or_else(|| AuthError::Flow("no scripted answer left".into()))
    }

    fn notify(&self, event: AuthEvent) {
        if let Ok(mut events) = self.events.lock() {
            events.push(event);
        }
    }
}
