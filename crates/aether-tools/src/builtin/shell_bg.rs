//! Tools over background shell processes: poll one for new output, list what
//! is running, stop one. Starting is `bash` with `background: true`.

use std::sync::Arc;

use aether_core::id::ProcessId;
use aether_core::tool::{ResourceKey, SideEffectClass, ToolDescriptor, ToolOutcome};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use crate::background::{BackgroundProcesses, ProcessStatus};
use crate::output::OutputPolicy;
use crate::tool::{Tool, ToolContext, ToolError, ToolExecution, parse_input};

/// Conventional resource key for a background process.
pub fn process_resource(id: ProcessId) -> ResourceKey {
    ResourceKey(format!("process:{id}"))
}

#[derive(Debug, Deserialize)]
struct Handle {
    process_id: String,
}

impl Handle {
    fn parse(&self) -> Result<ProcessId, ToolError> {
        self.process_id
            .parse()
            .map_err(|e| ToolError::InvalidInput(format!("process_id: {e}")))
    }
}

fn handle_schema(what: &str) -> Value {
    json!({
        "type": "object",
        "properties": {
            "process_id": {"type": "string", "description": what}
        },
        "required": ["process_id"],
        "additionalProperties": false
    })
}

fn resource_of(input: &Value) -> Vec<ResourceKey> {
    input["process_id"]
        .as_str()
        .and_then(|s| s.parse().ok())
        .map(process_resource)
        .into_iter()
        .collect()
}

/// One line per process, in start order.
pub fn render(statuses: &[ProcessStatus]) -> String {
    if statuses.is_empty() {
        return "no background processes".into();
    }
    statuses
        .iter()
        .map(|s| {
            format!(
                "{} {} · {} pending · {} · output log: {}{}",
                s.id,
                if s.running {
                    "running".to_owned()
                } else {
                    match s.exit_code {
                        Some(code) => format!("exited {code}"),
                        None => "terminated".to_owned(),
                    }
                },
                s.pending_bytes,
                s.command,
                s.output_path.display(),
                s.output_error
                    .as_ref()
                    .map(|e| format!(" · output capture failed: {e}"))
                    .unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Debug)]
pub struct ShellOutputTool {
    processes: Arc<BackgroundProcesses>,
    output: OutputPolicy,
}

impl ShellOutputTool {
    pub fn new(processes: Arc<BackgroundProcesses>) -> Self {
        Self {
            processes,
            output: OutputPolicy::default(),
        }
    }
}

#[async_trait]
impl Tool for ShellOutputTool {
    fn descriptor(&self) -> ToolDescriptor {
        super::descriptor(
            "shell_output",
            "Read what a background shell process has produced since the last read, and whether \
             it is still running. Each call returns a bounded page; poll again while pending_bytes is nonzero, even after exit. The output log preserves up to 8 MiB for read_file or bash and is removed on settlement, stopped-log eviction, or agent end.",
            handle_schema("Handle returned by `bash` with background: true."),
            SideEffectClass::ReadOnly,
            false,
        )
    }

    fn resources(&self, input: &Value, _ctx: &ToolContext) -> Vec<ResourceKey> {
        resource_of(input)
    }

    async fn execute(
        &self,
        input: Value,
        ctx: &ToolContext,
        _cancel: CancellationToken,
    ) -> Result<ToolExecution, ToolError> {
        let id = parse_input::<Handle>(input)?.parse()?;
        let out = self.processes.read(ctx.agent_id, id)?;
        let mut text = if out.text.is_empty() {
            "(no new output)".to_owned()
        } else {
            out.text
        };
        if out.dropped_bytes > 0 {
            text.push_str(&format!(
                "\n[{} bytes dropped: capture byte limit reached]",
                out.dropped_bytes
            ));
        }
        text.push_str(&format!(
            "\n[{}]",
            render(std::slice::from_ref(&out.status))
        ));
        let bounded = self
            .output
            .bound(
                text,
                "background output",
                aether_core::artifact::ArtifactKind::CommandOutput,
                ctx,
            )
            .await?;
        Ok(ToolExecution::Outcome(ToolOutcome {
            content: bounded.text,
            is_error: out.status.output_error.is_some(),
            artifacts: bounded.artifacts,
            truncated: bounded.truncated || out.dropped_bytes > 0,
            structured: Some(json!({
                "running": out.status.running,
                "exit_code": out.status.exit_code,
                "output_path": out.status.output_path,
                "pending_bytes": out.status.pending_bytes,
                "dropped_bytes": out.dropped_bytes,
                "output_error": out.status.output_error,
            })),
            ..Default::default()
        }))
    }
}

#[derive(Debug)]
pub struct ShellListTool {
    processes: Arc<BackgroundProcesses>,
}

impl ShellListTool {
    pub fn new(processes: Arc<BackgroundProcesses>) -> Self {
        Self { processes }
    }
}

#[async_trait]
impl Tool for ShellListTool {
    fn descriptor(&self) -> ToolDescriptor {
        super::descriptor(
            "shell_processes",
            "List the background shell processes this agent started, with their state.",
            json!({"type": "object", "properties": {}, "additionalProperties": false}),
            SideEffectClass::ReadOnly,
            false,
        )
    }

    fn resources(&self, _input: &Value, _ctx: &ToolContext) -> Vec<ResourceKey> {
        Vec::new()
    }

    async fn execute(
        &self,
        _input: Value,
        ctx: &ToolContext,
        _cancel: CancellationToken,
    ) -> Result<ToolExecution, ToolError> {
        let statuses = self.processes.list(ctx.agent_id)?;
        Ok(ToolExecution::Outcome(ToolOutcome::ok(render(&statuses))))
    }
}

#[derive(Debug)]
pub struct ShellStopTool {
    processes: Arc<BackgroundProcesses>,
}

impl ShellStopTool {
    pub fn new(processes: Arc<BackgroundProcesses>) -> Self {
        Self { processes }
    }
}

#[async_trait]
impl Tool for ShellStopTool {
    fn descriptor(&self) -> ToolDescriptor {
        super::descriptor(
            "shell_stop",
            "Stop a background shell process and forget its handle. Its output log remains readable.",
            handle_schema("Handle of the process to stop."),
            SideEffectClass::Mutation,
            false,
        )
    }

    fn resources(&self, input: &Value, _ctx: &ToolContext) -> Vec<ResourceKey> {
        resource_of(input)
    }

    async fn execute(
        &self,
        input: Value,
        ctx: &ToolContext,
        _cancel: CancellationToken,
    ) -> Result<ToolExecution, ToolError> {
        let id = parse_input::<Handle>(input)?.parse()?;
        let status = self.processes.stop(ctx.agent_id, id).await?;
        Ok(ToolExecution::Outcome(ToolOutcome {
            content: format!(
                "stopped {id} ({}) · {} pending bytes in {} · {} dropped bytes",
                status.command,
                status.pending_bytes,
                status.output_path.display(),
                status.dropped_bytes
            ),
            structured: Some(
                json!({"pending_bytes": status.pending_bytes, "dropped_bytes": status.dropped_bytes, "output_path": status.output_path}),
            ),
            side_effect_occurred: Some(true),
            ..Default::default()
        }))
    }
}

#[derive(Debug)]
pub struct WriteStdinTool {
    processes: Arc<BackgroundProcesses>,
}
impl WriteStdinTool {
    pub fn new(processes: Arc<BackgroundProcesses>) -> Self {
        Self { processes }
    }
}
#[async_trait]
impl Tool for WriteStdinTool {
    fn descriptor(&self) -> ToolDescriptor {
        super::descriptor(
            "write_stdin",
            "Send text or control characters to your interactive bash session. Include a newline to submit a line. Use shell_output to read the response and shell_stop to terminate. Input may execute commands and requires the same scope/approval as shell execution.",
            json!({"type":"object","properties":{"process_id":{"type":"string"},"chars":{"type":"string","maxLength":8192}},"required":["process_id","chars"],"additionalProperties":false}),
            SideEffectClass::Mutation,
            false,
        )
    }
    fn resources(&self, input: &Value, ctx: &ToolContext) -> Vec<ResourceKey> {
        let mut resources = resource_of(input);
        resources.extend(crate::builtin::bash::shell_resources(ctx));
        resources
    }
    fn rule_argument(&self, input: &Value) -> Option<String> {
        input
            .get("chars")
            .and_then(Value::as_str)
            .map(str::to_owned)
    }
    async fn execute(
        &self,
        input: Value,
        ctx: &ToolContext,
        cancel: CancellationToken,
    ) -> Result<ToolExecution, ToolError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Input {
            process_id: ProcessId,
            chars: String,
        }
        let input: Input = parse_input(input)?;
        if input.chars.len() > 8192 {
            return Err(ToolError::InvalidInput(
                "chars must be at most 8192 UTF-8 bytes".into(),
            ));
        }
        let written = self
            .processes
            .write_stdin(ctx.agent_id, input.process_id, &input.chars, cancel)
            .await?;
        Ok(ToolExecution::Outcome(ToolOutcome {
            content: format!("sent {written} bytes; read the response with shell_output"),
            structured: Some(json!({"bytes_written":written})),
            side_effect_occurred: Some(written > 0),
            ..Default::default()
        }))
    }
}

#[cfg(test)]
mod tests;
