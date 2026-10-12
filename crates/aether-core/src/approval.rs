//! Remembered approval decisions: durable rules that answer a tool call the
//! way a person already answered one like it.
//!
//! A rule says *what* it matches (a tool, optionally narrowed by a glob over
//! the call's argument or by a resource prefix) and *where* it applies (one
//! agent, an agent and its descendants, a run, or everything). Rules are
//! created and revoked by events; nothing evaluates a revoked rule.

use serde::{Deserialize, Serialize};

use crate::event::Actor;
use crate::id::{AgentId, ApprovalRuleId, RunId};
use crate::time::Timestamp;
use crate::tool::{ApprovalDecision, ResourceKey};

/// Where a remembered decision applies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "scope", rename_all = "snake_case")]
pub enum RuleScope {
    /// Every agent this daemon runs.
    Everywhere,
    /// One agent only.
    Agent { id: AgentId },
    /// An agent and everything it delegates to.
    Subtree { root: AgentId },
    /// Every agent in one run.
    Run { id: RunId },
}

/// What a rule matches. An absent field matches anything, so the empty
/// matcher covers every call — deliberately, so "allow everything for this
/// run" is expressible without a magic pattern.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RuleMatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    /// Glob over the call's argument (the command for a shell, the path for
    /// a file tool). `*` crosses separators.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub argument: Option<String>,
    /// Literal prefix a touched resource key must start with, e.g.
    /// `file:/repo/src`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_prefix: Option<String>,
}

impl RuleMatch {
    pub fn tool(name: impl Into<String>) -> Self {
        Self {
            tool: Some(name.into()),
            ..Default::default()
        }
    }

    pub fn with_argument(mut self, glob: impl Into<String>) -> Self {
        self.argument = Some(glob.into());
        self
    }

    /// Match exactly this argument: glob metacharacters in it are escaped, so
    /// remembering `rm -rf *` never widens into a wildcard.
    pub fn with_exact_argument(mut self, literal: &str) -> Self {
        self.argument = Some(globset::escape(literal));
        self
    }

    pub fn with_resource_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.resource_prefix = Some(prefix.into());
        self
    }

    /// Reject a matcher whose glob cannot compile, at the boundary where the
    /// rule is created rather than when a call is gated.
    pub fn validate(&self) -> Result<(), RuleError> {
        if let Some(pattern) = &self.argument {
            compile(pattern)?;
        }
        Ok(())
    }

    fn matches(&self, subject: &CallSubject) -> bool {
        if let Some(tool) = &self.tool
            && tool != &subject.tool
        {
            return false;
        }
        if let Some(pattern) = &self.argument {
            let Some(argument) = &subject.argument else {
                return false;
            };
            match compile(pattern) {
                Ok(glob) => {
                    if !glob.is_match(argument.as_str()) {
                        return false;
                    }
                }
                // A stored rule with a broken glob must never widen access.
                Err(_) => return false,
            }
        }
        if let Some(prefix) = &self.resource_prefix
            && !subject.resources.iter().any(|r| covers(prefix, &r.0))
        {
            return false;
        }
        true
    }

    /// How narrow this matcher is, used only to report the rule that decided.
    fn specificity(&self) -> u8 {
        self.tool.is_some() as u8
            + self.argument.is_some() as u8
            + self.resource_prefix.is_some() as u8
    }
}

/// Whether `prefix` covers `key`, matching only at a path boundary: a rule
/// about `file:/repo/src` must not reach `file:/repo/src-backup`.
fn covers(prefix: &str, key: &str) -> bool {
    let Some(rest) = key.strip_prefix(prefix) else {
        return false;
    };
    rest.is_empty() || prefix.ends_with('/') || rest.starts_with('/')
}

fn compile(pattern: &str) -> Result<globset::GlobMatcher, RuleError> {
    globset::GlobBuilder::new(pattern)
        .literal_separator(false)
        .build()
        .map(|g| g.compile_matcher())
        .map_err(|e| RuleError::BadGlob {
            pattern: pattern.to_owned(),
            reason: e.to_string(),
        })
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RuleError {
    #[error("pattern `{pattern}` is not a valid glob: {reason}")]
    BadGlob { pattern: String, reason: String },
}

/// Why a rule stopped applying. Kept on the record so the log explains a
/// decision that would be made differently today.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Revocation {
    pub at: Timestamp,
    pub by: Actor,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// A decision a person asked Aether to remember.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ApprovalRule {
    pub id: ApprovalRuleId,
    pub decision: ApprovalDecision,
    #[serde(rename = "match")]
    pub matcher: RuleMatch,
    pub scope: RuleScope,
    pub created_at: Timestamp,
    pub created_by: Actor,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked: Option<Revocation>,
}

impl ApprovalRule {
    pub fn is_active(&self) -> bool {
        self.revoked.is_none()
    }

    fn applies_to(&self, ctx: &RuleContext) -> bool {
        match &self.scope {
            RuleScope::Everywhere => true,
            RuleScope::Agent { id } => *id == ctx.agent,
            RuleScope::Subtree { root } => *root == ctx.agent || ctx.ancestors.contains(root),
            RuleScope::Run { id } => Some(*id) == ctx.run,
        }
    }

    /// Whether this rule decides `subject` for the agent described by `ctx`.
    pub fn matches(&self, subject: &CallSubject, ctx: &RuleContext) -> bool {
        self.is_active() && self.applies_to(ctx) && self.matcher.matches(subject)
    }
}

/// The call a rule is evaluated against, derived from the call itself.
#[derive(Debug, Clone, PartialEq)]
pub struct CallSubject {
    pub tool: String,
    /// The argument a person would write a rule about; `None` when the tool
    /// exposes none.
    pub argument: Option<String>,
    pub resources: Vec<ResourceKey>,
    /// Whether `resources` is the whole truth. False when the caller could
    /// not work out what the call touches, which makes a resource-scoped
    /// deny unevaluable - and so makes the call undecidable by any allow.
    pub resources_known: bool,
}

impl CallSubject {
    /// A subject whose resources were derived from the call.
    pub fn new(
        tool: impl Into<String>,
        argument: Option<String>,
        resources: Vec<ResourceKey>,
    ) -> Self {
        Self {
            tool: tool.into(),
            argument,
            resources,
            resources_known: true,
        }
    }

    /// A subject whose resources are real but may not be the whole truth:
    /// a call Aether hands to something it does not run can name what it was
    /// pointed at, never everything it will touch. Rules still match the
    /// keys given, and an unevaluable deny still fails the call closed.
    pub fn partial(
        tool: impl Into<String>,
        argument: Option<String>,
        resources: Vec<ResourceKey>,
    ) -> Self {
        Self {
            tool: tool.into(),
            argument,
            resources,
            resources_known: false,
        }
    }

    /// A subject whose resources could not be derived. A resource-scoped
    /// deny cannot be checked against it, so no allow decides it either.
    pub fn with_unknown_resources(tool: impl Into<String>, argument: Option<String>) -> Self {
        Self {
            tool: tool.into(),
            argument,
            resources: Vec::new(),
            resources_known: false,
        }
    }
}

/// The agent a call belongs to: enough to place it in the scope hierarchy.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleContext {
    pub agent: AgentId,
    /// Nearest parent first.
    pub ancestors: Vec<AgentId>,
    pub run: Option<RunId>,
}

/// The rule that decides a call, or `None` when a person must answer.
///
/// Deny wins: a single matching deny outranks every matching allow, however
/// specific the allow is. Among rules of one decision the most specific
/// matcher is reported, and ties go to the newest rule.
///
/// Deny-wins only holds while both rules can see the same subject. When a
/// deny in scope is narrowed by something the subject cannot supply - its
/// resources are unknown, or it names no argument - that deny cannot be
/// evaluated, so no allow may decide the call either: a person answers.
pub fn decide<'a>(
    rules: &'a [ApprovalRule],
    subject: &CallSubject,
    ctx: &RuleContext,
) -> Option<&'a ApprovalRule> {
    let best = |decision: ApprovalDecision| {
        rules
            .iter()
            .filter(|r| r.decision == decision && r.matches(subject, ctx))
            .max_by_key(|r| (r.matcher.specificity(), r.created_at))
    };
    if let Some(denied) = best(ApprovalDecision::Denied) {
        return Some(denied);
    }
    if has_unevaluable_deny(rules, subject, ctx) {
        return None;
    }
    best(ApprovalDecision::Approved)
}

/// An in-scope deny narrowed by something this subject cannot supply: the
/// resources when they are unknown, or an argument the call does not name.
/// Either way the deny cannot be evaluated, so no allow may decide the call.
fn has_unevaluable_deny(rules: &[ApprovalRule], subject: &CallSubject, ctx: &RuleContext) -> bool {
    let unevaluable = |m: &RuleMatch| {
        (m.resource_prefix.is_some() && !subject.resources_known)
            || (m.argument.is_some() && subject.argument.is_none())
    };
    rules.iter().any(|r| {
        r.decision == ApprovalDecision::Denied
            && r.is_active()
            && r.applies_to(ctx)
            && unevaluable(&r.matcher)
            && r.matcher.tool.as_ref().is_none_or(|t| t == &subject.tool)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::now;

    fn rule(decision: ApprovalDecision, matcher: RuleMatch, scope: RuleScope) -> ApprovalRule {
        ApprovalRule {
            id: ApprovalRuleId::new(),
            decision,
            matcher,
            scope,
            created_at: now(),
            created_by: Actor::user(),
            note: None,
            revoked: None,
        }
    }

    fn subject(tool: &str, argument: Option<&str>, resources: &[&str]) -> CallSubject {
        CallSubject::new(
            tool,
            argument.map(str::to_owned),
            resources.iter().map(|r| ResourceKey((*r).into())).collect(),
        )
    }

    fn ctx(agent: AgentId, ancestors: Vec<AgentId>, run: RunId) -> RuleContext {
        RuleContext {
            agent,
            ancestors,
            run: Some(run),
        }
    }

    #[test]
    fn tool_and_argument_glob_narrow_a_rule() {
        let agent = AgentId::new();
        let c = ctx(agent, vec![], RunId::new());
        let r = rule(
            ApprovalDecision::Approved,
            RuleMatch::tool("bash").with_argument("git status*"),
            RuleScope::Agent { id: agent },
        );
        assert!(r.matches(&subject("bash", Some("git status --short"), &[]), &c));
        assert!(!r.matches(&subject("bash", Some("git push"), &[]), &c));
        assert!(!r.matches(&subject("write_file", Some("git status"), &[]), &c));
        // A rule about an argument never matches a call that has none.
        assert!(!r.matches(&subject("bash", None, &[]), &c));
    }

    #[test]
    fn resource_prefix_narrows_by_path() {
        let agent = AgentId::new();
        let c = ctx(agent, vec![], RunId::new());
        let r = rule(
            ApprovalDecision::Approved,
            RuleMatch::tool("write_file").with_resource_prefix("file:/repo/src"),
            RuleScope::Everywhere,
        );
        assert!(r.matches(&subject("write_file", None, &["file:/repo/src/a.rs"]), &c));
        assert!(!r.matches(&subject("write_file", None, &["file:/repo/README"]), &c));
    }

    #[test]
    fn scopes_place_a_rule_in_the_hierarchy() {
        let parent = AgentId::new();
        let child = AgentId::new();
        let other = AgentId::new();
        let run = RunId::new();
        let s = subject("bash", Some("ls"), &[]);
        let subtree = rule(
            ApprovalDecision::Approved,
            RuleMatch::tool("bash"),
            RuleScope::Subtree { root: parent },
        );
        assert!(subtree.matches(&s, &ctx(parent, vec![], run)));
        assert!(subtree.matches(&s, &ctx(child, vec![parent], run)));
        assert!(!subtree.matches(&s, &ctx(other, vec![], run)));

        let per_run = rule(
            ApprovalDecision::Approved,
            RuleMatch::default(),
            RuleScope::Run { id: run },
        );
        assert!(per_run.matches(&s, &ctx(other, vec![], run)));
        assert!(!per_run.matches(&s, &ctx(other, vec![], RunId::new())));
    }

    #[test]
    fn deny_outranks_a_more_specific_allow() {
        let agent = AgentId::new();
        let c = ctx(agent, vec![], RunId::new());
        let rules = vec![
            rule(
                ApprovalDecision::Approved,
                RuleMatch::tool("bash").with_argument("git *"),
                RuleScope::Agent { id: agent },
            ),
            rule(
                ApprovalDecision::Denied,
                RuleMatch::default(),
                RuleScope::Everywhere,
            ),
        ];
        let decided = decide(&rules, &subject("bash", Some("git push"), &[]), &c).expect("a rule");
        assert_eq!(decided.decision, ApprovalDecision::Denied);
    }

    #[test]
    fn a_revoked_rule_decides_nothing() {
        let agent = AgentId::new();
        let c = ctx(agent, vec![], RunId::new());
        let mut r = rule(
            ApprovalDecision::Approved,
            RuleMatch::tool("bash"),
            RuleScope::Everywhere,
        );
        r.revoked = Some(Revocation {
            at: now(),
            by: Actor::user(),
            reason: Some("no longer safe".into()),
        });
        assert!(decide(&[r], &subject("bash", Some("ls"), &[]), &c).is_none());
    }

    #[test]
    fn an_exact_argument_never_widens_into_a_wildcard() {
        let agent = AgentId::new();
        let c = ctx(agent, vec![], RunId::new());
        let r = rule(
            ApprovalDecision::Approved,
            RuleMatch::tool("bash").with_exact_argument("rm -rf *"),
            RuleScope::Everywhere,
        );
        assert!(r.matches(&subject("bash", Some("rm -rf *"), &[]), &c));
        assert!(!r.matches(&subject("bash", Some("rm -rf /etc"), &[]), &c));
    }

    #[test]
    fn a_broken_glob_is_refused_at_creation_and_never_matches() {
        let bad = RuleMatch::tool("bash").with_argument("a[");
        assert!(matches!(bad.validate(), Err(RuleError::BadGlob { .. })));
        let agent = AgentId::new();
        let r = rule(
            ApprovalDecision::Approved,
            bad,
            RuleScope::Agent { id: agent },
        );
        assert!(!r.matches(
            &subject("bash", Some("a["), &[]),
            &ctx(agent, vec![], RunId::new())
        ));
    }

    #[test]
    fn a_resource_prefix_matches_only_at_a_path_boundary() {
        let agent = AgentId::new();
        let c = ctx(agent, vec![], RunId::new());
        let r = rule(
            ApprovalDecision::Approved,
            RuleMatch::tool("write_file").with_resource_prefix("file:/repo/src"),
            RuleScope::Everywhere,
        );
        assert!(r.matches(&subject("write_file", None, &["file:/repo/src"]), &c));
        assert!(r.matches(&subject("write_file", None, &["file:/repo/src/a.rs"]), &c));
        assert!(
            !r.matches(
                &subject("write_file", None, &["file:/repo/src-backup/a"]),
                &c
            ),
            "a sibling directory is not inside the rule"
        );
        assert!(!r.matches(&subject("write_file", None, &["file:/repo/srcret"]), &c));
    }

    #[test]
    fn an_allow_cannot_decide_a_call_whose_resources_a_deny_needs() {
        let agent = AgentId::new();
        let c = ctx(agent, vec![], RunId::new());
        let rules = vec![
            rule(
                ApprovalDecision::Approved,
                RuleMatch::tool("Write"),
                RuleScope::Subtree { root: agent },
            ),
            rule(
                ApprovalDecision::Denied,
                RuleMatch::tool("Write").with_resource_prefix("file:/etc"),
                RuleScope::Everywhere,
            ),
        ];
        // Resources derived: the deny is evaluable, so the allow may decide
        // a call outside /etc and the deny takes one inside it.
        let outside = subject("Write", None, &["file:/repo/a.rs"]);
        assert_eq!(
            decide(&rules, &outside, &c).map(|r| r.decision),
            Some(ApprovalDecision::Approved)
        );
        let inside = subject("Write", None, &["file:/etc/passwd"]);
        assert_eq!(
            decide(&rules, &inside, &c).map(|r| r.decision),
            Some(ApprovalDecision::Denied)
        );
        // Resources unknown: the deny cannot be checked, so nothing decides.
        let unknown = CallSubject::with_unknown_resources("Write", None);
        assert!(
            decide(&rules, &unknown, &c).is_none(),
            "a broad allow must not outrank a deny that cannot be evaluated"
        );
        // With no resource-scoped deny in play, an unknown subject is fine.
        assert_eq!(
            decide(&rules[..1], &unknown, &c).map(|r| r.decision),
            Some(ApprovalDecision::Approved)
        );
    }

    #[test]
    fn an_allow_cannot_decide_a_call_whose_argument_a_deny_needs() {
        let agent = AgentId::new();
        let c = ctx(agent, vec![], RunId::new());
        let rules = vec![
            rule(
                ApprovalDecision::Approved,
                RuleMatch::tool("WebFetch"),
                RuleScope::Subtree { root: agent },
            ),
            rule(
                ApprovalDecision::Denied,
                RuleMatch::tool("WebFetch").with_argument("*evil.example*"),
                RuleScope::Everywhere,
            ),
        ];
        // Argument present: the deny is evaluable and decides its own calls.
        assert_eq!(
            decide(
                &rules,
                &subject("WebFetch", Some("https://ok.example"), &[]),
                &c
            )
            .map(|r| r.decision),
            Some(ApprovalDecision::Approved)
        );
        assert_eq!(
            decide(
                &rules,
                &subject("WebFetch", Some("https://evil.example/x"), &[]),
                &c
            )
            .map(|r| r.decision),
            Some(ApprovalDecision::Denied)
        );
        // No argument: the deny cannot be checked, so no allow decides it.
        assert!(
            decide(&rules, &subject("WebFetch", None, &[]), &c).is_none(),
            "a broad allow must not outrank an argument deny that cannot be evaluated"
        );
        // A deny about another tool leaves this call decidable.
        assert_eq!(
            decide(&rules[..1], &subject("WebFetch", None, &[]), &c).map(|r| r.decision),
            Some(ApprovalDecision::Approved)
        );
    }
}
