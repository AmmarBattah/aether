//! What a run did, folded out of its events.
//!
//! A report is derived, never stored: the log is the only durable truth
//! (§1.5), so the same events always compose the same document and a report
//! of a replayed or imported log equals the report of the original. Nothing
//! here reads state - if a fact is not in the events it is not in the report.

mod builder;
mod render;
pub use builder::{Builder, build};

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::agent::Role;
use crate::event::Actor;
use crate::id::{AgentId, ApprovalId, ArtifactId, RunId, TaskId};
use crate::model::{ModelRef, Usage};
use crate::task::Priority;
use crate::time::Timestamp;
use crate::tool::ApprovalDecision;

/// The document's shape. Bumped when a field changes meaning; adding a field
/// is additive and does not bump it.
pub const REPORT_SCHEMA: u32 = 1;

/// Where an imported run came from. Absent on a run this daemon ran itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Provenance {
    /// The run's id in the daemon that exported it.
    pub source_run: RunId,
    /// `instance_id` of the store it was exported from.
    pub source_instance: String,
    pub exported_at: Timestamp,
}

/// A run's report: a header that says which run and when, and a body that
/// says what happened. Only the body is comparable across stores - the
/// header carries the local identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RunReport {
    pub schema: u32,
    pub run: RunId,
    pub generated_at: Timestamp,
    /// Sequence range of the events this was folded from, in this store.
    pub from_seq: u64,
    pub to_seq: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
    pub body: RunBody,
}

/// Everything derived from the run's own work. Carries no local sequence
/// numbers, so two stores holding the same run produce equal bodies once
/// their ids are put side by side ([`RunReport::canonical_json`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RunBody {
    /// The root agent's objective, when the run has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objective: Option<String>,
    pub outcome: RunOutcome,
    pub started_at: Timestamp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<Timestamp>,
    /// Wall time from the first event to the last, in milliseconds.
    pub duration_ms: i64,
    /// Every agent's own usage summed: the run's total spend.
    pub usage: Usage,
    pub events: u64,
    pub agents: Vec<AgentLine>,
    pub tasks: Vec<TaskLine>,
    pub tools: Vec<ToolLine>,
    pub approvals: Vec<ApprovalLine>,
    pub failures: Vec<FailureLine>,
    pub artifacts: Vec<ArtifactLine>,
    /// What is still unfinished: live agents, open tasks, pending approvals.
    pub open_loops: Vec<OpenLoop>,
    pub timeline: Vec<Milestone>,
}

/// A run's state, read from its agents rather than declared anywhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RunOutcome {
    /// At least one agent has not reached a terminal state.
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl RunOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

/// How one agent or task ended.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Ended {
    #[default]
    Running,
    Completed {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        summary: Option<String>,
    },
    Failed {
        error: String,
    },
    Cancelled {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
}

impl Ended {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed { .. } => "completed",
            Self::Failed { .. } => "failed",
            Self::Cancelled { .. } => "cancelled",
        }
    }

    fn is_terminal(&self) -> bool {
        !matches!(self, Self::Running)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AgentLine {
    pub id: AgentId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<AgentId>,
    pub name: String,
    pub role: Role,
    pub model: ModelRef,
    /// Depth below the run's root, so a text rendering can indent the tree.
    pub depth: u32,
    pub turns: u32,
    pub tool_calls: u32,
    /// This agent's own spend.
    pub usage: Usage,
    /// This agent's spend plus every descendant's.
    pub subtree_usage: Usage,
    pub outcome: Ended,
    pub created_at: Timestamp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<Timestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaskLine {
    pub id: TaskId,
    pub objective: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<AgentId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_task: Option<TaskId>,
    #[serde(default)]
    pub dependencies: Vec<TaskId>,
    pub priority: Priority,
    pub outcome: Ended,
    pub created_at: Timestamp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<Timestamp>,
}

/// One tool's tally across the run.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema,
)]
pub struct ToolTally {
    pub requested: u32,
    pub succeeded: u32,
    pub failed: u32,
    pub denied: u32,
    pub cancelled: u32,
    /// Answered from a committed outcome; the call did not run again.
    pub replayed: u32,
    pub retries: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ToolLine {
    pub tool: String,
    #[serde(flatten)]
    pub tally: ToolTally,
    /// Time spent inside calls of this tool, successful or failed. A failed
    /// call that never actually ran (`ToolFailed.duration_ms == None`)
    /// contributes nothing, since it has no real time to report.
    pub duration_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ApprovalLine {
    pub id: ApprovalId,
    pub tool: String,
    pub description: String,
    pub requested_at: Timestamp,
    /// Absent while the person has not answered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision: Option<ApprovalDecision>,
    /// Who answered. The actor itself rather than a label: a remembered
    /// decision names a rule by id, and only a structural id is remapped on
    /// import - flattened into `rule:rule_…` it survives as another
    /// machine's id and the two reports stop matching.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decided_by: Option<Actor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decided_at: Option<Timestamp>,
}

/// What failed, and what it was. Every failure in the run, in order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FailureLine {
    pub at: Timestamp,
    /// The event type that carried it, e.g. `tool.failed`.
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<AgentId>,
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ArtifactLine {
    pub id: ArtifactId,
    pub name: String,
    pub kind: String,
    pub summary: String,
    pub size_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<AgentId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<TaskId>,
    pub created_at: Timestamp,
    /// Retention dropped the bytes; the record still says what was produced.
    #[serde(default)]
    pub evicted: bool,
}

/// Something the run left unfinished.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "open", rename_all = "snake_case")]
pub enum OpenLoop {
    Agent {
        id: AgentId,
        name: String,
    },
    Task {
        id: TaskId,
        objective: String,
    },
    Approval {
        id: ApprovalId,
        tool: String,
        description: String,
    },
}

impl OpenLoop {
    /// One line for a person, without the id: the document lists that.
    pub fn line(&self) -> String {
        match self {
            Self::Agent { name, .. } => format!("agent {name} has not finished"),
            Self::Task { objective, .. } => format!("task open: {objective}"),
            Self::Approval {
                tool, description, ..
            } => format!("approval unanswered: {tool} {description}"),
        }
    }
}

/// One line of the run's story: the events a person would want to see.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Milestone {
    pub at: Timestamp,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<AgentId>,
    pub detail: String,
}

// ---------------------------------------------------------------------------
// Comparing two reports of the same run
// ---------------------------------------------------------------------------

/// Ids are minted per store, so a run imported elsewhere carries different
/// ones. Replacing each with the order it is first met leaves exactly what
/// the run did, which is what two stores must agree on.
fn ordinalize(value: &mut serde_json::Value, seen: &mut BTreeMap<String, String>) {
    match value {
        serde_json::Value::String(s) => {
            if let Some(prefix) = crate::id::prefix_of(s) {
                let next = format!("{prefix}#{}", seen.len());
                let ordinal = seen.entry(s.clone()).or_insert(next);
                *s = ordinal.clone();
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                ordinalize(item, seen);
            }
        }
        serde_json::Value::Object(map) => {
            for (_, v) in map.iter_mut() {
                ordinalize(v, seen);
            }
        }
        _ => {}
    }
}

impl RunReport {
    /// The body with every typed id replaced by the order it is first met,
    /// so the report of an imported run compares equal to the report of the
    /// run it was exported from. Ordering is the document's own, which is
    /// stable because every collection in it is sorted or append-ordered.
    /// Fallible on purpose: degrading a serialization failure to
    /// `Value::Null` would make two unrelated reports compare equal, and
    /// silently pass the one comparison this function exists for.
    pub fn canonical_json(&self) -> Result<serde_json::Value, serde_json::Error> {
        let mut value = serde_json::to_value(&self.body)?;
        ordinalize(&mut value, &mut BTreeMap::new());
        Ok(value)
    }
}

#[cfg(test)]
mod tests;
