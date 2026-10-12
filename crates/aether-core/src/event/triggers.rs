use super::Actor;
use crate::{
    id::{TaskId, ToolCallId},
    model::ModelRef,
};
use serde::{Deserialize, Serialize};

/// How an interrupted turn should be handled after a restart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryClass {
    /// No mutating tool was in flight; the agent can simply take another turn.
    Resume,
    /// Retry from the last committed step; in-flight work was safe to repeat.
    Retry,
    /// A side effect may have partially occurred; a human must look.
    Inspect,
}

/// What triggered a compaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "trigger", rename_all = "snake_case")]
pub enum CompactionTrigger {
    TokenThreshold {
        tokens: u32,
        threshold: u32,
    },
    TaskBoundary {
        task: TaskId,
    },
    LargeToolOutput {
        call: ToolCallId,
    },
    LongWait,
    Explicit {
        by: Actor,
    },
    /// The agent's model moved across the engine/API boundary. `from` is the
    /// model that produced the context being folded, so it can compact it.
    Handoff {
        from: ModelRef,
        to: ModelRef,
        /// The engine session the move abandoned, captured as it was cleared.
        /// The fold runs turns later, from a row where the field is already
        /// gone, so this is the only place the loss can still be read.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        session: Option<String>,
    },
}

impl CompactionTrigger {
    /// Short, stable name for logs, tables and `--json` filters.
    pub fn name(&self) -> &'static str {
        match self {
            Self::TokenThreshold { .. } => "token_threshold",
            Self::TaskBoundary { .. } => "task_boundary",
            Self::LargeToolOutput { .. } => "large_tool_output",
            Self::LongWait => "long_wait",
            Self::Explicit { .. } => "explicit",
            Self::Handoff { .. } => "handoff",
        }
    }
}
