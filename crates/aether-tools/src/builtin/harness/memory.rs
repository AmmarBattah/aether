use super::{harness_descriptor, parse_id};
use crate::tool::{HarnessAction, Tool, ToolContext, ToolError, ToolExecution, parse_input};
use aether_core::tool::{ResourceKey, ToolDescriptor};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use aether_core::memory::{MemoryCandidate, MemoryKind, MemoryProvenance, MemoryScope};

// ----- remember ------------------------------------------------------------

#[derive(Debug)]
pub struct Remember;

#[async_trait]
impl Tool for Remember {
    fn descriptor(&self) -> ToolDescriptor {
        harness_descriptor(
            "remember",
            "Propose a durable memory: a stable fact, decision, lesson, constraint or preference \
             likely to matter again. Most tasks need none. Do not record transient details or \
             anything already visible in the repository.",
            json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "The memory, one to three sentences."},
                    "kind": {"type": "string", "enum": ["fact", "decision", "lesson", "constraint", "preference"]},
                    "scope": {
                        "type": "string",
                        "enum": ["global", "repository", "role", "agent", "subtree"],
                        "default": "repository",
                        "description": "Who sees it: everyone, this repository, agents with your role, only you, or you and every agent you delegate to (subtree)."
                    },
                    "importance": {"type": "number", "minimum": 0, "maximum": 1, "default": 0.5},
                    "confidence": {"type": "number", "minimum": 0, "maximum": 1, "default": 0.7},
                    "tags": {"type": "array", "items": {"type": "string"}},
                    "supersedes": {"type": "string", "description": "Memory id (mem_...) this replaces."}
                },
                "required": ["text", "kind"],
                "additionalProperties": false
            }),
        )
    }

    fn resources(&self, _input: &Value, _ctx: &ToolContext) -> Vec<ResourceKey> {
        Vec::new()
    }

    /// A decided, in-memory state transition once the model commits to it
    /// - see `Tool::interruptible`.
    fn interruptible(&self) -> bool {
        false
    }

    async fn execute(
        &self,
        input: Value,
        ctx: &ToolContext,
        _cancel: CancellationToken,
    ) -> Result<ToolExecution, ToolError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Input {
            text: String,
            // A raw `Value`, not `MemoryKind`: see the comment on
            // `DelegateInput::effort`. Parsed below via `parse_field` so the
            // rejection names `remember.kind`.
            kind: Value,
            #[serde(default)]
            scope: Option<String>,
            #[serde(default)]
            importance: Option<f32>,
            #[serde(default)]
            confidence: Option<f32>,
            #[serde(default)]
            tags: Vec<String>,
            #[serde(default)]
            supersedes: Option<String>,
        }
        let input: Input = parse_input(input)?;
        if input.text.trim().is_empty() {
            return Err(ToolError::InvalidInput("text is empty".into()));
        }
        let kind: MemoryKind = crate::builtin::parse_field(&input.kind, "remember.kind")?;
        if let Some(importance) = input.importance
            && !(0.0..=1.0).contains(&importance)
        {
            return Err(ToolError::InvalidInput(format!(
                "importance must be between 0 and 1, got {importance}"
            )));
        }
        if let Some(confidence) = input.confidence
            && !(0.0..=1.0).contains(&confidence)
        {
            return Err(ToolError::InvalidInput(format!(
                "confidence must be between 0 and 1, got {confidence}"
            )));
        }
        let scope = match input.scope.as_deref().unwrap_or("repository") {
            "global" => MemoryScope::Global,
            "repository" => match &ctx.scope.repository {
                Some(id) => MemoryScope::Repository { id: id.clone() },
                None => MemoryScope::Agent { id: ctx.agent_id },
            },
            "role" => MemoryScope::Role {
                role: "unknown".into(),
            },
            "agent" => MemoryScope::Agent { id: ctx.agent_id },
            // Rooted at the caller only: an agent shares with those beneath it,
            // never files notes into another agent's subtree.
            "subtree" => MemoryScope::Subtree { root: ctx.agent_id },
            other => return Err(ToolError::InvalidInput(format!("unknown scope `{other}`"))),
        };
        let supersedes = input
            .supersedes
            .as_deref()
            .map(|s| parse_id(s, "memory id"))
            .transpose()?;
        Ok(ToolExecution::Action(HarnessAction::Remember {
            candidate: MemoryCandidate {
                scope,
                kind,
                text: input.text,
                structured: None,
                // Validated above to already be within [0, 1]: no clamp needed.
                importance: input.importance.unwrap_or(0.5),
                confidence: input.confidence.unwrap_or(0.7),
                tags: input.tags,
                provenance: MemoryProvenance {
                    task: ctx.task_id,
                    agent: Some(ctx.agent_id),
                    ..Default::default()
                },
                supersedes,
            },
        }))
    }
}
