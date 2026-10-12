//! Tool execution vocabulary: what the runtime records about every call.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::event::Actor;
use crate::id::{AgentId, ApprovalId, ArtifactId, InvocationId, TaskId, ToolCallId};
use crate::idempotency::IdempotencyKey;
use crate::model::ToolSpec;
use crate::time::Timestamp;

/// What a tool does to the world. Drives retry policy, idempotency tracking
/// and approval requirements.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SideEffectClass {
    /// Safe to retry any number of times.
    ReadOnly,
    /// Repeating with the same input yields the same end state.
    IdempotentMutation,
    /// Repeating may compound the effect (append, send, spend).
    Mutation,
    /// Reaches outside the workspace: network, external services.
    External,
}

impl SideEffectClass {
    pub fn is_mutating(self) -> bool {
        !matches!(self, Self::ReadOnly)
    }

    pub fn safe_to_retry(self) -> bool {
        matches!(self, Self::ReadOnly | Self::IdempotentMutation)
    }
}

/// A resource a call touches, used to decide which calls may run together.
/// Conventional forms: `file:<absolute path>`, `dir:<absolute path>`,
/// `repo:<root>`, `network`, `agent:<id>`.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(transparent)]
pub struct ResourceKey(pub String);

impl ResourceKey {
    pub fn file(path: impl AsRef<std::path::Path>) -> Self {
        Self(format!("file:{}", path.as_ref().display()))
    }

    pub fn dir(path: impl AsRef<std::path::Path>) -> Self {
        Self(format!("dir:{}", path.as_ref().display()))
    }

    pub fn repo(root: impl AsRef<std::path::Path>) -> Self {
        Self(format!("repo:{}", root.as_ref().display()))
    }

    pub fn network() -> Self {
        Self("network".into())
    }

    /// Whether the key is taken as a cross-agent lease. Path keys are;
    /// singleton ordering keys such as `network` only order calls inside one
    /// batch - leasing one would serialize the whole fleet on it.
    pub fn is_leasable(&self) -> bool {
        ["file:", "dir:", "repo:"]
            .iter()
            .any(|p| self.0.starts_with(p))
    }

    /// Two keys conflict when equal or when one is a directory or repo that
    /// contains the other's path.
    pub fn conflicts_with(&self, other: &ResourceKey) -> bool {
        if self == other {
            return true;
        }
        let container = |k: &ResourceKey| {
            k.0.strip_prefix("dir:")
                .or_else(|| k.0.strip_prefix("repo:"))
                .map(str::to_owned)
        };
        let path = |k: &ResourceKey| {
            k.0.strip_prefix("file:")
                .or_else(|| k.0.strip_prefix("dir:"))
                .or_else(|| k.0.strip_prefix("repo:"))
                .map(str::to_owned)
        };
        let contains = |a: &ResourceKey, b: &ResourceKey| match (container(a), path(b)) {
            (Some(root), Some(p)) => std::path::Path::new(&p).starts_with(&root),
            _ => false,
        };
        contains(self, other) || contains(other, self)
    }
}

/// A tool as registered with the runtime: the model-facing spec plus the
/// runtime-facing metadata the model never sees.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ToolDescriptor {
    pub spec: ToolSpec,
    pub side_effect: SideEffectClass,
    #[serde(default)]
    pub needs_network: bool,
}

/// One tool invocation requested by a model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ToolCall {
    pub id: ToolCallId,
    /// Id issued by the provider, echoed in the tool result block.
    pub provider_call_id: String,
    pub name: String,
    pub input: Value,
    pub agent_id: AgentId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<TaskId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invocation_id: Option<InvocationId>,
    /// Resources the call will touch, computed by the tool from its input.
    #[serde(default)]
    pub resources: Vec<ResourceKey>,
    /// What makes this call the same effect as another. Absent for classes
    /// whose outcome is not recorded (see `SideEffectClass::retry_policy`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<IdempotencyKey>,
    pub requested_at: Timestamp,
}

impl ToolCall {
    /// Whether two calls may run concurrently: read-only pairs always may;
    /// anything mutating must not share a conflicting resource.
    pub fn can_run_concurrently_with(
        &self,
        self_class: SideEffectClass,
        other: &ToolCall,
        other_class: SideEffectClass,
    ) -> bool {
        if !self_class.is_mutating() && !other_class.is_mutating() {
            return true;
        }
        !self
            .resources
            .iter()
            .any(|r| other.resources.iter().any(|o| r.conflicts_with(o)))
    }
}

/// What a tool produced.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ToolOutcome {
    /// Model-facing text. Large outputs are summarized here and stored as artifacts.
    pub content: String,
    #[serde(default)]
    pub is_error: bool,
    #[serde(default)]
    pub artifacts: Vec<ArtifactId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured: Option<Value>,
    #[serde(default)]
    pub duration_ms: u64,
    #[serde(default)]
    pub truncated: bool,
    /// Whether a side effect is known to have occurred. `None` means unknown,
    /// which recovery treats as "needs inspection" for mutating tools.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub side_effect_occurred: Option<bool>,
}

impl ToolOutcome {
    pub fn ok(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            ..Default::default()
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: message.into(),
            is_error: true,
            ..Default::default()
        }
    }

    pub fn with_duration(mut self, ms: u64) -> Self {
        self.duration_ms = ms;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    Approved,
    Denied,
}

impl ApprovalDecision {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Approved => "approved",
            Self::Denied => "denied",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ApprovalStatus {
    Pending,
    Resolved {
        decision: ApprovalDecision,
        by: Actor,
        at: Timestamp,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    },
}

/// A request for a human (or supervising agent) to allow a tool call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ApprovalRequest {
    pub id: ApprovalId,
    pub agent_id: AgentId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<TaskId>,
    pub call_id: ToolCallId,
    pub tool: String,
    pub description: String,
    /// The argument a remembered decision would be written about: the command
    /// for a shell, the path for a file tool. `None` when the tool has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub argument: Option<String>,
    /// Resources the call touches, so an approval card and a remembered
    /// decision see the same keys the scheduler used.
    #[serde(default)]
    pub resources: Vec<ResourceKey>,
    pub side_effect: SideEffectClass,
    pub requested_at: Timestamp,
    pub status: ApprovalStatus,
}

/// Which registered tools an agent may see.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum ToolExposure {
    #[default]
    All,
    Only {
        names: BTreeSet<String>,
    },
    Except {
        names: BTreeSet<String>,
    },
}

impl ToolExposure {
    pub fn allows(&self, name: &str) -> bool {
        match self {
            Self::All => true,
            Self::Only { names } => names.contains(name),
            Self::Except { names } => !names.contains(name),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::now;

    #[test]
    fn only_path_keys_are_leased_across_agents() {
        assert!(ResourceKey::file("/r/a.rs").is_leasable());
        assert!(ResourceKey::dir("/r/src").is_leasable());
        assert!(ResourceKey::repo("/r").is_leasable());
        assert!(
            !ResourceKey::network().is_leasable(),
            "leasing `network` would serialize every networked shell in the fleet"
        );
    }

    fn call(resources: Vec<ResourceKey>) -> ToolCall {
        ToolCall {
            id: ToolCallId::new(),
            provider_call_id: "p".into(),
            name: "t".into(),
            input: Value::Null,
            agent_id: AgentId::new(),
            task_id: None,
            invocation_id: None,
            resources,
            idempotency_key: None,
            requested_at: now(),
        }
    }

    #[test]
    fn resource_conflicts() {
        let f = ResourceKey::file("/repo/src/a.rs");
        assert!(f.conflicts_with(&ResourceKey::file("/repo/src/a.rs")));
        assert!(!f.conflicts_with(&ResourceKey::file("/repo/src/b.rs")));
        assert!(f.conflicts_with(&ResourceKey::dir("/repo/src")));
        assert!(ResourceKey::repo("/repo").conflicts_with(&f));
        assert!(!ResourceKey::dir("/other").conflicts_with(&f));
        assert!(ResourceKey::network().conflicts_with(&ResourceKey::network()));
    }

    #[test]
    fn concurrency_rules() {
        let a = call(vec![ResourceKey::file("/r/a")]);
        let b = call(vec![ResourceKey::file("/r/a")]);
        let c = call(vec![ResourceKey::file("/r/c")]);
        use SideEffectClass::*;
        assert!(a.can_run_concurrently_with(ReadOnly, &b, ReadOnly));
        assert!(!a.can_run_concurrently_with(Mutation, &b, ReadOnly));
        assert!(a.can_run_concurrently_with(Mutation, &c, Mutation));
        let repo = call(vec![ResourceKey::repo("/r")]);
        assert!(!repo.can_run_concurrently_with(Mutation, &c, ReadOnly));
    }
}
