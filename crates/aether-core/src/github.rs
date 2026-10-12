//! Opt-in GitHub ingestion. Credentials and external request text never live in settings.
use serde::{Deserialize, Serialize};

use crate::{id::AgentId, schedule::SpawnSpec, time::Timestamp};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct GithubWatcherConfig {
    pub enabled: bool,
    pub mention_regex: String,
    pub label_regex: String,
    pub poll_interval_secs: u64,
    pub repositories: Vec<GithubRepositoryConfig>,
    /// GitHub logins whose own requests carry operator authority on every
    /// watched repository, stored lowercase. Absent trusts nobody that far.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operator_users: Option<Vec<String>>,
}

impl Default for GithubWatcherConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            mention_regex: r"(?i)(?:^|[^a-z0-9_@])@aether(?:$|[^a-z0-9_-])".into(),
            label_regex: "(?i)aether".into(),
            poll_interval_secs: 120,
            repositories: Vec::new(),
            operator_users: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GithubRepositoryConfig {
    /// Explicit GitHub.com `owner/name`, not a URL or a local path.
    pub repository: String,
    pub enabled: bool,
    /// Existing agent chosen by the operator; GitHub cannot select its target.
    /// Required when `dispatch` is `Fixed`; ignored under `Thread`.
    #[serde(default)]
    pub agent: Option<AgentId>,
    /// How a matched request picks its target agent.
    #[serde(default)]
    pub dispatch: GithubDispatchMode,
    /// GitHub logins allowed to dispatch, stored lowercase. Absent allows any
    /// writer; a listed login must still pass the write-permission check.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_users: Option<Vec<String>>,
}

/// How a repository's accepted requests reach an agent.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
#[allow(clippy::large_enum_variant)]
pub enum GithubDispatchMode {
    /// Deliver every trigger to the fixed, operator-chosen `agent`. Stale if
    /// that agent pauses or goes terminal; an operator must repoint it.
    #[default]
    Fixed,
    /// Create an Aether-managed checkout and agent for each issue/PR thread.
    /// The daemon's current model, approval, and network defaults apply.
    Automatic,
    /// Deliver to (or spawn, the first time or after the previous one went
    /// terminal) a durable agent keyed by (repository, issue/PR number), so
    /// the watcher heals itself instead of pointing at a dead agent forever.
    Thread(SpawnSpec),
}

impl GithubRepositoryConfig {
    /// The one-step watch mode. Explicit fixed and custom thread routes remain
    /// available for operators who need a particular agent or scope.
    pub fn automatic(repository: String) -> Self {
        Self {
            repository,
            enabled: true,
            agent: None,
            dispatch: GithubDispatchMode::Automatic,
            allowed_users: None,
        }
    }

    /// Whether `login` may request work before the permission check runs.
    /// GitHub logins are case-insensitive, so matching is too.
    pub fn allows_requester(&self, login: &str) -> bool {
        debug_assert!(!login.is_empty(), "candidates carry a validated login");
        match &self.allowed_users {
            None => true,
            Some(users) => users.iter().any(|user| user.eq_ignore_ascii_case(login)),
        }
    }

    /// Canonical stored form: lowercase repository and logins, with repeated
    /// logins dropped in first-seen order.
    pub fn normalize(&mut self) {
        self.repository.make_ascii_lowercase();
        if let Some(users) = &mut self.allowed_users {
            normalize_logins(users);
        }
    }
}

/// Lowercase `users` and drop repeats, keeping first-seen order.
fn normalize_logins(users: &mut Vec<String>) {
    let listed = users.len();
    let mut seen = std::collections::HashSet::new();
    users.retain_mut(|user| {
        user.make_ascii_lowercase();
        seen.insert(user.clone())
    });
    debug_assert!(users.len() <= listed, "dedup never adds logins");
    debug_assert_eq!(users.is_empty(), listed == 0, "dedup keeps the first login");
}

/// Upper bound on one list of logins (an allowlist, or `operator_users`);
/// enough for any team, small enough to keep matching and display trivial.
pub const MAX_ALLOWED_USERS: usize = 100;

/// GitHub's login rule: 1-39 ASCII alphanumerics or single inner hyphens.
pub fn is_github_login(login: &str) -> bool {
    !login.is_empty()
        && login.len() <= 39
        && !login.starts_with('-')
        && !login.ends_with('-')
        && !login.contains("--")
        && login
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GithubWatcherStatus {
    pub revision: u64,
    pub config: GithubWatcherConfig,
    pub repositories: Vec<GithubRepositoryStatus>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GithubRepositoryStatus {
    pub repository: String,
    pub last_poll_at: Option<Timestamp>,
    pub last_success_at: Option<Timestamp>,
    pub next_poll_at: Option<Timestamp>,
    pub last_error: Option<String>,
    pub dispatched: u64,
    pub ignored: u64,
}

impl GithubWatcherConfig {
    /// Whether `login` is trusted as the operator. Never enough by itself:
    /// the watcher also verifies dispatch rights and who wrote the text.
    pub fn is_operator(&self, login: &str) -> bool {
        debug_assert!(!login.is_empty(), "candidates carry a validated login");
        self.operator_users
            .as_ref()
            .is_some_and(|users| users.iter().any(|user| user.eq_ignore_ascii_case(login)))
    }

    /// Canonical stored form of every repository and of `operator_users`.
    pub fn normalize(&mut self) {
        for repo in &mut self.repositories {
            repo.normalize();
        }
        if let Some(users) = &mut self.operator_users {
            normalize_logins(users);
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if !(60..=86400).contains(&self.poll_interval_secs) {
            return Err("GitHub poll_interval_secs must be between 60 and 86400".into());
        }
        if self.repositories.len() > 32 {
            return Err("at most 32 GitHub repositories may be configured".into());
        }
        let mut names = std::collections::HashSet::new();
        for repo in &self.repositories {
            let parts: Vec<_> = repo.repository.split('/').collect();
            if parts.len() != 2
                || parts.iter().any(|s| {
                    s.is_empty()
                        || *s == "."
                        || *s == ".."
                        || s.len() > 100
                        || !s
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
                })
            {
                return Err(format!(
                    "invalid GitHub repository {:?}; expected owner/name",
                    repo.repository
                ));
            }
            if !names.insert(repo.repository.to_ascii_lowercase()) {
                return Err(format!("duplicate GitHub repository {}", repo.repository));
            }
            validate_dispatch(repo)?;
            validate_allowed_users(repo)?;
        }
        if let Some(users) = &self.operator_users {
            check_operator_users(users)?;
        }
        for (name, pattern) in [
            ("mention_regex", &self.mention_regex),
            ("label_regex", &self.label_regex),
        ] {
            if pattern.is_empty() || pattern.len() > 4096 {
                return Err(format!("GitHub {name} must contain 1..4096 bytes"));
            }
            regex::RegexBuilder::new(pattern)
                .size_limit(1 << 20)
                .build()
                .map_err(|e| format!("invalid GitHub {name}: {e}"))?;
        }
        Ok(())
    }
}

/// An allowlist only narrows who may dispatch, so an empty one is refused
/// rather than read as "allow everyone" or silently as "allow no one".
fn validate_allowed_users(repo: &GithubRepositoryConfig) -> Result<(), String> {
    let Some(users) = &repo.allowed_users else {
        return Ok(());
    };
    check_allowed_users(users)
        .map_err(|error| format!("GitHub repository {}: {error}", repo.repository))
}

/// The daemon's allowlist rules, shared so clients report them before saving.
/// The cap counts logins as stored: lowercase, each counted once.
pub fn check_allowed_users(users: &[String]) -> Result<(), String> {
    if users.is_empty() {
        return Err(
            "allowed_users is empty; omit it to allow every collaborator with \
             write access, or list at least one login"
                .into(),
        );
    }
    check_logins("allowed_users", users)
}

/// The same rules for `operator_users`. An empty list is refused rather than
/// stored as a setting that looks set and trusts nobody.
pub fn check_operator_users(users: &[String]) -> Result<(), String> {
    if users.is_empty() {
        return Err(
            "operator_users is empty; omit it to give no GitHub login operator \
             authority, or list at least one login"
                .into(),
        );
    }
    check_logins("operator_users", users)
}

/// Login syntax and the distinct-login cap for the non-empty list `field`.
fn check_logins(field: &str, users: &[String]) -> Result<(), String> {
    debug_assert!(!users.is_empty(), "callers refuse an empty {field} first");
    if let Some(invalid) = users.iter().find(|user| !is_github_login(user)) {
        return Err(format!(
            "invalid GitHub login {invalid:?} in {field}; expected 1-39 letters, \
             digits or single hyphens, not starting or ending with a hyphen"
        ));
    }
    let distinct = users
        .iter()
        .map(|user| user.to_ascii_lowercase())
        .collect::<std::collections::HashSet<_>>()
        .len();
    debug_assert!(
        (1..=users.len()).contains(&distinct),
        "de-duplication keeps at least one and never adds logins"
    );
    if distinct > MAX_ALLOWED_USERS {
        return Err(format!(
            "{field} lists {distinct} distinct logins; at most \
             {MAX_ALLOWED_USERS} are allowed"
        ));
    }
    Ok(())
}

/// `Fixed` needs an operator-chosen target; `Thread` needs a template that
/// can spawn a fresh, bounded run for every mentioning thread (mirrors
/// `aether-runtime`'s `checked_template` for a spawn schedule's first run).
fn validate_dispatch(repo: &GithubRepositoryConfig) -> Result<(), String> {
    match &repo.dispatch {
        GithubDispatchMode::Fixed => {
            if repo.agent.is_none() {
                return Err(format!(
                    "GitHub repository {} uses fixed dispatch but names no agent",
                    repo.repository
                ));
            }
        }
        GithubDispatchMode::Automatic => {
            if repo.agent.is_some() {
                return Err(format!(
                    "GitHub repository {} uses automatic dispatch but names a fixed agent",
                    repo.repository
                ));
            }
        }
        GithubDispatchMode::Thread(template) => {
            if template.role.trim().is_empty() {
                return Err(format!(
                    "GitHub repository {}'s thread dispatch template needs a role",
                    repo.repository
                ));
            }
            if template.objective.trim().is_empty() {
                return Err(format!(
                    "GitHub repository {}'s thread dispatch template needs an objective",
                    repo.repository
                ));
            }
            if let Some(config) = &template.config {
                if config.deadline.is_some() {
                    return Err(format!(
                        "GitHub repository {}'s thread dispatch template cannot set an absolute \
                         deadline: it would pass after the first spawned agent; use max_turns or \
                         a budget",
                        repo.repository
                    ));
                }
                if config.long_lived {
                    return Err(format!(
                        "GitHub repository {}'s thread dispatch template cannot be long-lived: \
                         a thread's spawned agent must end so a later mention can resume or \
                         replace it",
                        repo.repository
                    ));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_are_opt_in_and_patterns_validate() {
        let config = GithubWatcherConfig::default();
        assert!(!config.enabled);
        assert!(config.repositories.is_empty());
        assert!(config.validate().is_ok());
    }
    #[test]
    fn invalid_patterns_intervals_and_repository_paths_are_rejected() {
        let mut c = GithubWatcherConfig {
            mention_regex: "[".into(),
            ..Default::default()
        };
        assert!(c.validate().is_err());
        c = GithubWatcherConfig {
            poll_interval_secs: 0,
            ..Default::default()
        };
        assert!(c.validate().is_err());
        for repository in [
            "https://github.com/a/b",
            "a/..",
            "a/b?token=secret",
            "a/b/c",
        ] {
            c = GithubWatcherConfig::default();
            c.repositories.push(GithubRepositoryConfig {
                repository: repository.into(),
                enabled: true,
                agent: Some(AgentId::new()),
                dispatch: GithubDispatchMode::default(),
                allowed_users: None,
            });
            assert!(c.validate().is_err());
        }
    }

    fn repo(dispatch: GithubDispatchMode, agent: Option<AgentId>) -> GithubRepositoryConfig {
        GithubRepositoryConfig {
            repository: "owner/repo".into(),
            enabled: true,
            agent,
            dispatch,
            allowed_users: None,
        }
    }

    #[test]
    fn fixed_dispatch_without_an_agent_is_rejected() {
        let mut c = GithubWatcherConfig::default();
        c.repositories.push(repo(GithubDispatchMode::Fixed, None));
        let error = c.validate().unwrap_err();
        assert!(error.contains("fixed dispatch"), "{error}");
    }

    #[test]
    fn automatic_dispatch_needs_no_target_and_rejects_a_stale_fixed_agent() {
        let mut c = GithubWatcherConfig::default();
        c.repositories
            .push(GithubRepositoryConfig::automatic("owner/repo".into()));
        assert!(c.validate().is_ok());
        c.repositories[0].agent = Some(AgentId::new());
        assert!(c.validate().unwrap_err().contains("automatic dispatch"));
    }

    #[test]
    fn thread_dispatch_with_empty_role_or_objective_is_rejected() {
        let base = SpawnSpec {
            role: "worker".into(),
            name: None,
            objective: "Handle this thread.".into(),
            model: None,
            fallbacks: Vec::new(),
            scope: None,
            config: None,
            approval: None,
        };
        for template in [
            SpawnSpec {
                role: "  ".into(),
                ..base.clone()
            },
            SpawnSpec {
                objective: " ".into(),
                ..base.clone()
            },
        ] {
            let mut c = GithubWatcherConfig::default();
            c.repositories
                .push(repo(GithubDispatchMode::Thread(template), None));
            assert!(c.validate().is_err());
        }
    }

    #[test]
    fn thread_dispatch_with_long_lived_or_deadline_config_is_rejected() {
        let base = SpawnSpec {
            role: "worker".into(),
            name: None,
            objective: "Handle this thread.".into(),
            model: None,
            fallbacks: Vec::new(),
            scope: None,
            config: None,
            approval: None,
        };
        let long_lived = SpawnSpec {
            config: Some(crate::agent::AgentConfig {
                long_lived: true,
                ..Default::default()
            }),
            ..base.clone()
        };
        let deadline = SpawnSpec {
            config: Some(crate::agent::AgentConfig {
                deadline: Some(crate::time::now()),
                ..Default::default()
            }),
            ..base
        };
        for template in [long_lived, deadline] {
            let mut c = GithubWatcherConfig::default();
            c.repositories
                .push(repo(GithubDispatchMode::Thread(template), None));
            assert!(c.validate().is_err());
        }
    }

    #[test]
    fn a_valid_thread_dispatch_config_validates() {
        let template = SpawnSpec {
            role: "worker".into(),
            name: None,
            objective: "Handle this thread.".into(),
            model: None,
            fallbacks: Vec::new(),
            scope: None,
            config: None,
            approval: None,
        };
        let mut c = GithubWatcherConfig::default();
        c.repositories
            .push(repo(GithubDispatchMode::Thread(template), None));
        assert!(c.validate().is_ok());
    }

    fn allowlisted(users: &[&str]) -> GithubWatcherConfig {
        let mut repository = GithubRepositoryConfig::automatic("owner/repo".into());
        repository.allowed_users = Some(users.iter().map(|user| (*user).into()).collect());
        GithubWatcherConfig {
            repositories: vec![repository],
            ..Default::default()
        }
    }

    #[test]
    fn an_empty_allowlist_is_rejected_rather_than_read_as_everyone() {
        let error = allowlisted(&[]).validate().unwrap_err();
        assert!(error.contains("allowed_users is empty"), "{error}");
        assert!(allowlisted(&["octoCat"]).validate().is_ok());
    }

    #[test]
    fn allowlisted_logins_must_be_github_logins() {
        let longest = "a".repeat(39);
        for valid in [
            "a",
            "octoCat",
            "dev-arch-overclocked",
            "0x1",
            longest.as_str(),
        ] {
            assert!(allowlisted(&[valid]).validate().is_ok(), "{valid}");
        }
        let too_long = "a".repeat(40);
        for invalid in [
            "",
            "-lead",
            "trail-",
            "double--hyphen",
            "under_score",
            "bot[bot]",
            "d\u{e9}v",
            "a b",
            too_long.as_str(),
        ] {
            let error = allowlisted(&[invalid]).validate().unwrap_err();
            assert!(error.contains("invalid GitHub login"), "{invalid}: {error}");
        }
    }

    fn crowd(logins: impl Iterator<Item = String>) -> GithubWatcherConfig {
        let logins: Vec<String> = logins.collect();
        let logins: Vec<&str> = logins.iter().map(String::as_str).collect();
        allowlisted(&logins)
    }

    #[test]
    fn allowlist_cap_counts_distinct_logins_after_normalization() {
        let over = MAX_ALLOWED_USERS + 1;
        let distinct = crowd((0..over).map(|n| format!("u{n}")));
        let error = distinct.validate().unwrap_err();
        assert!(
            error.contains("101 distinct logins") && error.contains("owner/repo"),
            "{error}"
        );

        let repeated = crowd((0..over).map(|n| format!("u{}", n % MAX_ALLOWED_USERS)));
        assert_eq!(
            repeated.repositories[0]
                .allowed_users
                .as_ref()
                .unwrap()
                .len(),
            over
        );
        assert!(repeated.validate().is_ok());

        let case_variants = crowd((0..over).map(|n| match n {
            0 => "OctoCat".to_owned(),
            1 => "octocat".to_owned(),
            n => format!("u{n}"),
        }));
        assert!(case_variants.validate().is_ok());
        let mut normalized = case_variants.repositories[0].clone();
        normalized.normalize();
        assert_eq!(normalized.allowed_users.unwrap().len(), MAX_ALLOWED_USERS);
    }

    #[test]
    fn normalizing_lowercases_and_deduplicates_in_first_seen_order() {
        let mut config = allowlisted(&["octoCat", "MonaLisa", "OCTOCAT"]);
        config.repositories[0].repository = "Owner/Repo".into();
        config.repositories[0].normalize();
        assert_eq!(config.repositories[0].repository, "owner/repo");
        assert_eq!(
            config.repositories[0].allowed_users.as_deref(),
            Some(&["octocat".to_owned(), "monalisa".to_owned()][..])
        );
        let mut open = GithubRepositoryConfig::automatic("o/r".into());
        open.normalize();
        assert_eq!(open.allowed_users, None);
    }

    #[test]
    fn requester_matching_is_case_insensitive_and_absent_list_allows_all() {
        let config = allowlisted(&["octocat"]);
        let repository = &config.repositories[0];
        assert!(repository.allows_requester("OctoCat"));
        assert!(repository.allows_requester("octocat"));
        assert!(!repository.allows_requester("Some-Stranger"));
        assert!(GithubRepositoryConfig::automatic("o/r".into()).allows_requester("anyone"));
    }

    #[test]
    fn allowlist_is_optional_on_the_wire_and_round_trips() {
        let open = GithubRepositoryConfig::automatic("o/r".into());
        let json = serde_json::to_value(&open).unwrap();
        assert!(json.get("allowed_users").is_none(), "{json}");
        let legacy: GithubRepositoryConfig = serde_json::from_value(serde_json::json!({
            "repository": "o/r", "enabled": true, "dispatch": {"mode": "automatic"}
        }))
        .unwrap();
        assert_eq!(legacy, open);
        let config = allowlisted(&["octocat", "monalisa"]);
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains(r#""allowed_users":["octocat","monalisa"]"#));
        assert_eq!(
            serde_json::from_str::<GithubWatcherConfig>(&json).unwrap(),
            config
        );
    }

    fn operators(users: &[&str]) -> GithubWatcherConfig {
        GithubWatcherConfig {
            operator_users: Some(users.iter().map(|user| (*user).into()).collect()),
            ..Default::default()
        }
    }

    #[test]
    fn operator_users_follow_the_allowlist_rules() {
        let error = operators(&[]).validate().unwrap_err();
        assert!(error.contains("operator_users is empty"), "{error}");
        for invalid in ["", "-lead", "double--hyphen", "bot[bot]", "a b"] {
            let error = operators(&[invalid]).validate().unwrap_err();
            assert!(
                error.contains("invalid GitHub login") && error.contains("operator_users"),
                "{invalid}: {error}"
            );
        }
        let crowd: Vec<String> = (0..=MAX_ALLOWED_USERS).map(|n| format!("u{n}")).collect();
        let crowd: Vec<&str> = crowd.iter().map(String::as_str).collect();
        let error = operators(&crowd).validate().unwrap_err();
        assert!(
            error.contains("operator_users lists 101 distinct logins"),
            "{error}"
        );
        assert!(operators(&crowd[1..]).validate().is_ok());
        assert!(operators(&["mengsig", "octoCat"]).validate().is_ok());
    }

    #[test]
    fn operator_users_normalize_and_match_case_insensitively() {
        let mut config = operators(&["Mengsig", "octoCat", "MENGSIG"]);
        config.normalize();
        assert_eq!(
            config.operator_users.as_deref(),
            Some(&["mengsig".to_owned(), "octocat".to_owned()][..])
        );
        assert!(config.is_operator("mengsig"));
        assert!(config.is_operator("OctoCat"));
        assert!(!config.is_operator("mengsig-bot"));
        assert!(!config.is_operator("Some-Stranger"));
    }

    #[test]
    fn nobody_is_an_operator_unless_the_list_names_them() {
        let unset = GithubWatcherConfig::default();
        assert_eq!(unset.operator_users, None);
        assert!(!unset.is_operator("mengsig"));
        // An allowlisted requester is not thereby an operator.
        assert!(!allowlisted(&["mengsig"]).is_operator("mengsig"));
        let empty = operators(&[]);
        assert!(!empty.is_operator("mengsig"));
        assert!(empty.validate().is_err());
    }

    #[test]
    fn operator_users_are_absent_on_the_wire_when_unset_and_round_trip() {
        let unset = GithubWatcherConfig::default();
        let json = serde_json::to_value(&unset).unwrap();
        assert!(json.get("operator_users").is_none(), "{json}");
        let legacy: GithubWatcherConfig = serde_json::from_value(json).unwrap();
        assert_eq!(legacy, unset);
        let config = operators(&["mengsig", "octocat"]);
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains(r#""operator_users":["mengsig","octocat"]"#));
        assert_eq!(
            serde_json::from_str::<GithubWatcherConfig>(&json).unwrap(),
            config
        );
    }
}
