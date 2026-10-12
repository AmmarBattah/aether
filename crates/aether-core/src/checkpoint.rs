//! Structured compaction checkpoints.
//!
//! A checkpoint is what an agent keeps of its older context after compaction.
//! It is a schema, not prose, so it can be validated: nothing an earlier
//! checkpoint held open may silently disappear.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::id::{AgentId, ArtifactId, CheckpointId, SnapshotId};
use crate::model::ModelRef;
use crate::time::Timestamp;

/// A reference to an important artifact, kept as a note rather than a copy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ArtifactNote {
    /// Artifact id when the item is a stored artifact; otherwise a path, commit, or URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    /// Why it matters.
    pub note: String,
}

/// A constraint the compactor decided no longer applies, with its reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DroppedItem {
    pub text: String,
    pub reason: String,
}

/// The model-facing checkpoint schema. This is what a compactor model is
/// asked to produce; the runtime wraps it into a [`Checkpoint`].
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckpointDraft {
    /// Current goal and remaining sub-goals.
    pub objective: String,
    /// What is true now, including active and pending child work.
    pub state: String,
    /// Choices already made that still constrain future work.
    #[serde(default)]
    pub decisions: Vec<String>,
    /// User requirements, repository rules, access limits, non-negotiables.
    #[serde(default)]
    pub constraints: Vec<String>,
    /// Constraints from the previous checkpoint that no longer apply, with reasons.
    #[serde(default)]
    pub dropped_constraints: Vec<DroppedItem>,
    /// Important files, commits, results. References, never large content.
    #[serde(default)]
    pub artifacts: Vec<ArtifactNote>,
    /// Questions, blockers, promises and follow-ups that remain unresolved.
    #[serde(default)]
    pub open_loops: Vec<String>,
    /// Open loops from the previous checkpoint that have since been resolved.
    #[serde(default)]
    pub resolved_open_loops: Vec<String>,
    /// Facts that are tentative and must not be promoted to certainty.
    #[serde(default)]
    pub uncertainty: Vec<String>,
}

/// Where a checkpoint came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CheckpointProvenance {
    /// Event sequence range the checkpoint summarizes, inclusive.
    pub from_seq: u64,
    pub to_seq: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_snapshot: Option<SnapshotId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compactor_model: Option<ModelRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous: Option<CheckpointId>,
    /// What asked for this compaction. Absent on checkpoints written before
    /// triggers were recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger: Option<crate::event::CompactionTrigger>,
}

/// A durable, validated checkpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Checkpoint {
    pub id: CheckpointId,
    pub agent_id: AgentId,
    pub version: u32,
    pub draft: CheckpointDraft,
    pub provenance: CheckpointProvenance,
    pub created_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CheckpointError {
    #[error("checkpoint objective is empty")]
    EmptyObjective,
    #[error("checkpoint state is empty")]
    EmptyState,
    #[error("open loop from previous checkpoint was dropped without resolution: {0}")]
    DroppedOpenLoop(String),
    #[error("constraint from previous checkpoint was dropped without reason: {0}")]
    DroppedConstraint(String),
}

impl CheckpointDraft {
    /// Validate against the schema rules and the previous checkpoint.
    ///
    /// Every open loop the previous checkpoint carried must be either still
    /// open or explicitly resolved; every constraint must be carried or
    /// explicitly dropped with a reason. Comparison is by normalized text.
    pub fn validate(&self, previous: Option<&CheckpointDraft>) -> Result<(), CheckpointError> {
        if self.objective.trim().is_empty() {
            return Err(CheckpointError::EmptyObjective);
        }
        if self.state.trim().is_empty() {
            return Err(CheckpointError::EmptyState);
        }
        let Some(prev) = previous else { return Ok(()) };
        let norm = |s: &str| s.trim().to_lowercase();
        for loop_ in &prev.open_loops {
            let key = norm(loop_);
            let carried = self.open_loops.iter().any(|l| norm(l) == key);
            let resolved = self.resolved_open_loops.iter().any(|l| norm(l) == key);
            if !carried && !resolved {
                return Err(CheckpointError::DroppedOpenLoop(loop_.clone()));
            }
        }
        for constraint in &prev.constraints {
            let key = norm(constraint);
            let carried = self.constraints.iter().any(|c| norm(c) == key);
            let dropped = self
                .dropped_constraints
                .iter()
                .any(|d| norm(&d.text) == key && !d.reason.trim().is_empty());
            if !carried && !dropped {
                return Err(CheckpointError::DroppedConstraint(constraint.clone()));
            }
        }
        Ok(())
    }

    /// JSON Schema for asking a model to produce a draft.
    pub fn json_schema() -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(CheckpointDraft)).unwrap_or_default()
    }

    /// Render for inclusion in a system prompt.
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("## Objective\n");
        out.push_str(&self.objective);
        out.push_str("\n\n## State\n");
        out.push_str(&self.state);
        let section = |out: &mut String, title: &str, items: &[String]| {
            if items.is_empty() {
                return;
            }
            out.push_str("\n\n## ");
            out.push_str(title);
            for item in items {
                out.push_str("\n- ");
                out.push_str(item);
            }
        };
        section(&mut out, "Decisions", &self.decisions);
        section(&mut out, "Constraints", &self.constraints);
        if !self.artifacts.is_empty() {
            out.push_str("\n\n## Artifacts");
            for a in &self.artifacts {
                out.push_str("\n- ");
                if let Some(r) = &a.reference {
                    out.push_str(r);
                    out.push_str(": ");
                }
                out.push_str(&a.note);
            }
        }
        section(&mut out, "Open loops", &self.open_loops);
        section(&mut out, "Uncertain", &self.uncertainty);
        out
    }
}

impl ArtifactNote {
    pub fn artifact_id(&self) -> Option<ArtifactId> {
        self.reference.as_deref().and_then(|r| r.parse().ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft(open: &[&str], constraints: &[&str]) -> CheckpointDraft {
        CheckpointDraft {
            objective: "ship".into(),
            state: "working".into(),
            open_loops: open.iter().map(|s| s.to_string()).collect(),
            constraints: constraints.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn rejects_empty_objective() {
        let d = CheckpointDraft {
            state: "s".into(),
            ..Default::default()
        };
        assert_eq!(d.validate(None), Err(CheckpointError::EmptyObjective));
    }

    #[test]
    fn open_loops_must_be_carried_or_resolved() {
        let prev = draft(&["ask user about auth"], &[]);
        let dropped = draft(&[], &[]);
        assert!(matches!(
            dropped.validate(Some(&prev)),
            Err(CheckpointError::DroppedOpenLoop(_))
        ));
        let mut resolved = draft(&[], &[]);
        resolved
            .resolved_open_loops
            .push("Ask user about auth".into());
        assert_eq!(resolved.validate(Some(&prev)), Ok(()));
        let carried = draft(&["ask user about auth"], &[]);
        assert_eq!(carried.validate(Some(&prev)), Ok(()));
    }

    #[test]
    fn constraints_must_be_carried_or_dropped_with_reason() {
        let prev = draft(&[], &["no force push"]);
        let missing = draft(&[], &[]);
        assert!(matches!(
            missing.validate(Some(&prev)),
            Err(CheckpointError::DroppedConstraint(_))
        ));
        let mut dropped = draft(&[], &[]);
        dropped.dropped_constraints.push(DroppedItem {
            text: "no force push".into(),
            reason: "user lifted it".into(),
        });
        assert_eq!(dropped.validate(Some(&prev)), Ok(()));
    }

    #[test]
    fn schema_has_required_fields() {
        let schema = CheckpointDraft::json_schema();
        let required = schema["required"].as_array().expect("required");
        assert!(required.iter().any(|v| v == "objective"));
        assert!(required.iter().any(|v| v == "state"));
    }

    #[test]
    fn dropped_constraints_require_a_nonblank_reason() {
        let prev = draft(&[], &["no force push"]);
        let mut next = draft(&[], &[]);
        for reason in ["", " \n\t"] {
            next.dropped_constraints = vec![DroppedItem {
                text: " No Force Push ".into(),
                reason: reason.into(),
            }];
            assert_eq!(
                next.validate(Some(&prev)),
                Err(CheckpointError::DroppedConstraint("no force push".into()))
            );
        }
        next.dropped_constraints[0].reason = "user lifted it".into();
        assert_eq!(next.validate(Some(&prev)), Ok(()));
        next.constraints = prev.constraints.clone();
        next.dropped_constraints[0].reason.clear();
        assert_eq!(next.validate(Some(&prev)), Ok(()));
    }
}
