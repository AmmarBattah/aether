//! Read a text file with line numbers and optional windowing.

use aether_core::tool::{ResourceKey, SideEffectClass, ToolDescriptor, ToolOutcome};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use crate::fs::{resolve_path, resource_paths};
use crate::output::OutputPolicy;
use crate::tool::{Tool, ToolContext, ToolError, ToolExecution, parse_input};

const DEFAULT_LIMIT: usize = 2000;

#[derive(Debug, Default)]
pub struct ReadFileTool {
    pub output: OutputPolicy,
}

#[derive(Debug, Deserialize)]
struct Input {
    path: String,
    /// 1-based first line to show.
    #[serde(default)]
    offset: Option<usize>,
    #[serde(default)]
    limit: Option<usize>,
}

#[async_trait]
impl Tool for ReadFileTool {
    fn descriptor(&self) -> ToolDescriptor {
        super::descriptor(
            "read_file",
            "Read a regular text file up to 8 MiB from the workspace; special files are refused. Output has line numbers. Use offset and limit \
             to read a window within that cap. For larger files, use grep or bash with sed -n.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "File path, relative to the workspace or absolute."},
                    "offset": {"type": "integer", "minimum": 1, "description": "First line to read (1-based)."},
                    "limit": {"type": "integer", "minimum": 1, "description": "Maximum number of lines to return."}
                },
                "required": ["path"],
                "additionalProperties": false
            }),
            SideEffectClass::ReadOnly,
            false,
        )
    }

    fn resources(&self, input: &Value, ctx: &ToolContext) -> Vec<ResourceKey> {
        input["path"]
            .as_str()
            .into_iter()
            .flat_map(|p| resource_paths(ctx, p))
            .map(ResourceKey::file)
            .collect()
    }

    async fn execute(
        &self,
        input: Value,
        ctx: &ToolContext,
        _cancel: CancellationToken,
    ) -> Result<ToolExecution, ToolError> {
        let Input {
            path,
            offset,
            limit,
        } = parse_input(input)?;
        let resolved = resolve_path(ctx, &path)?;
        let read_path = resolved.clone();
        let bytes = tokio::task::spawn_blocking(move || crate::fs::read_regular_file(&read_path))
            .await
            .map_err(|error| ToolError::Internal(error.to_string()))??;
        if super::looks_binary(&bytes) {
            return Err(ToolError::InvalidInput(format!("{path} is a binary file")));
        }
        let text = String::from_utf8_lossy(&bytes);
        let total = text.lines().count();
        let start = offset.unwrap_or(1).max(1);
        let limit = limit.unwrap_or(DEFAULT_LIMIT).max(1);
        if start > total && total > 0 {
            return Err(ToolError::InvalidInput(format!(
                "offset {start} is past the end ({total} lines)"
            )));
        }
        let width = total.to_string().len().max(4);
        let mut out = String::new();
        let mut shown = 0usize;
        for (i, line) in text.lines().enumerate().skip(start - 1).take(limit) {
            out.push_str(&format!("{:>width$}\t{line}\n", i + 1));
            shown += 1;
        }
        let end = start + shown.saturating_sub(1);
        if end < total {
            out.push_str(&format!(
                "[{} more lines; continue with offset={}]\n",
                total - end,
                end + 1
            ));
        }
        let bounded = self
            .output
            .bound_plain(&out, "read a smaller window with offset and limit");
        Ok(ToolExecution::Outcome(ToolOutcome {
            content: bounded.text,
            is_error: false,
            artifacts: Vec::new(),
            structured: Some(json!({ "total_lines": total, "start": start, "end": end })),
            duration_ms: 0,
            truncated: bounded.truncated,
            side_effect_occurred: Some(false),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::ctx_in;

    async fn read(ctx: &ToolContext, input: Value) -> Result<ToolOutcome, ToolError> {
        match ReadFileTool::default()
            .execute(input, ctx, CancellationToken::new())
            .await?
        {
            ToolExecution::Outcome(o) => Ok(o),
            ToolExecution::Action(_) => Err(ToolError::Internal("unexpected".into())),
        }
    }

    #[tokio::test]
    async fn oversized_file_is_refused_even_for_one_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("large.txt");
        let file = std::fs::File::create(path).unwrap();
        file.set_len(crate::fs::MAX_FILE_BYTES + 1).unwrap();
        let result = read(
            &ctx_in(dir.path()),
            json!({"path": "large.txt", "limit": 1}),
        )
        .await;
        assert!(
            matches!(result, Err(ToolError::InvalidInput(ref error)) if error.contains("size limit") && error.contains("8 MiB") && error.contains("grep") && error.contains("sed -n"))
        );
    }

    #[tokio::test]
    async fn reads_with_line_numbers_and_window() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f.txt"), "a\nb\nc\nd\n").unwrap();
        let ctx = ctx_in(dir.path());
        let out = read(&ctx, json!({"path": "f.txt"})).await.unwrap();
        assert!(out.content.contains("   1\ta\n"));
        assert!(out.content.contains("   4\td\n"));
        let out = read(&ctx, json!({"path": "f.txt", "offset": 2, "limit": 2}))
            .await
            .unwrap();
        assert!(out.content.contains("   2\tb\n   3\tc\n"));
        assert!(
            out.content
                .contains("[1 more lines; continue with offset=4]")
        );
    }

    #[tokio::test]
    async fn reads_through_an_in_scope_symlink() {
        // The open refuses a symlinked final component; resolution must have followed it.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("real.txt"), "linked\n").unwrap();
        std::os::unix::fs::symlink("real.txt", dir.path().join("alias.txt")).unwrap();
        let out = read(&ctx_in(dir.path()), json!({"path": "alias.txt"}))
            .await
            .unwrap();
        assert!(out.content.contains("   1\tlinked\n"), "{}", out.content);
    }

    #[tokio::test]
    async fn rejects_binary_missing_and_out_of_scope() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("bin"), [0u8, 1, 2, 0]).unwrap();
        let ctx = ctx_in(dir.path());
        assert!(matches!(
            read(&ctx, json!({"path": "bin"})).await,
            Err(ToolError::InvalidInput(_))
        ));
        assert!(matches!(
            read(&ctx, json!({"path": "nope"})).await,
            Err(ToolError::Io(_))
        ));
        assert!(matches!(
            read(&ctx, json!({"path": "/etc/hostname"})).await,
            Err(ToolError::OutOfScope(_))
        ));
        assert!(matches!(
            read(&ctx, json!({"nope": 1})).await,
            Err(ToolError::InvalidInput(_))
        ));
    }
}
