//! Exact-match text replacement with a diff in the result.

use aether_core::tool::{ResourceKey, SideEffectClass, ToolDescriptor, ToolOutcome};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use similar::TextDiff;
use tokio_util::sync::CancellationToken;

use crate::fs::{resolve_path, resource_paths};
use crate::output::OutputPolicy;
use crate::tool::{Tool, ToolContext, ToolError, ToolExecution, parse_input};

#[derive(Debug, Default)]
pub struct EditFileTool {
    pub output: OutputPolicy,
}

#[derive(Debug, Deserialize)]
struct Input {
    path: String,
    old_string: String,
    new_string: String,
    #[serde(default)]
    replace_all: bool,
}

#[async_trait]
impl Tool for EditFileTool {
    fn descriptor(&self) -> ToolDescriptor {
        super::descriptor(
            "edit_file",
            "Replace an exact string in a regular UTF-8 text file up to 8 MiB. old_string must match exactly once unless \
             replace_all is true. Returns a unified diff of the change.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "File path, relative to the workspace or absolute."},
                    "old_string": {"type": "string", "description": "Exact text to replace, including indentation."},
                    "new_string": {"type": "string", "description": "Replacement text."},
                    "replace_all": {"type": "boolean", "default": false, "description": "Replace every occurrence."}
                },
                "required": ["path", "old_string", "new_string"],
                "additionalProperties": false
            }),
            SideEffectClass::IdempotentMutation,
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
            old_string,
            new_string,
            replace_all,
        } = parse_input(input)?;
        if old_string.is_empty() {
            return Err(ToolError::InvalidInput("old_string is empty".into()));
        }
        if old_string == new_string {
            return Err(ToolError::InvalidInput(
                "old_string and new_string are identical".into(),
            ));
        }
        let resolved = resolve_path(ctx, &path)?;
        let read_path = resolved.clone();
        let bytes = tokio::task::spawn_blocking(move || crate::fs::read_regular_file(&read_path))
            .await
            .map_err(|error| ToolError::Internal(error.to_string()))??;
        if super::looks_binary(&bytes) {
            return Err(ToolError::InvalidInput(format!("{path} is a binary file")));
        }
        let original = String::from_utf8(bytes).map_err(|error| {
            ToolError::InvalidInput(format!("{path} is not valid UTF-8: {error}"))
        })?;
        let matches = original.matches(&old_string).count();
        let replaced = match (matches, replace_all) {
            (0, _) => {
                return Err(ToolError::InvalidInput(
                    "old_string was not found in the file".into(),
                ));
            }
            (1, _) => 1,
            (n, true) => n,
            (n, false) => {
                return Err(ToolError::InvalidInput(format!(
                    "old_string matches {n} times; include more context to make it unique or set replace_all"
                )));
            }
        };
        let updated_len = original
            .len()
            .saturating_sub(old_string.len().saturating_mul(replaced))
            .saturating_add(new_string.len().saturating_mul(replaced));
        if updated_len as u64 > crate::fs::MAX_FILE_BYTES {
            return Err(ToolError::InvalidInput(format!(
                "edited file exceeds the file-tool size limit of {} MiB; use bash for larger edits",
                crate::fs::MAX_FILE_BYTES / (1024 * 1024)
            )));
        }
        let updated = original.replacen(&old_string, &new_string, replaced);
        let write_path = resolved.clone();
        let write_bytes = updated.as_bytes().to_vec();
        tokio::task::spawn_blocking(move || {
            crate::fs::write_regular_file(&write_path, &write_bytes)
        })
        .await
        .map_err(|error| ToolError::Internal(error.to_string()))??;
        let shown = super::display_path(ctx, &resolved);
        let diff = TextDiff::from_lines(&original, &updated)
            .unified_diff()
            .context_radius(3)
            .header(&format!("a/{shown}"), &format!("b/{shown}"))
            .to_string();
        let bounded = self.output.bound_plain(&diff, "diff elided");
        Ok(ToolExecution::Outcome(ToolOutcome {
            content: format!(
                "Edited {shown} ({replaced} replacement{})\n{}",
                if replaced == 1 { "" } else { "s" },
                bounded.text
            ),
            is_error: false,
            artifacts: Vec::new(),
            structured: Some(json!({ "replacements": replaced })),
            duration_ms: 0,
            truncated: bounded.truncated,
            side_effect_occurred: Some(true),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::ctx_in;

    async fn edit(ctx: &ToolContext, input: Value) -> Result<ToolOutcome, ToolError> {
        match EditFileTool::default()
            .execute(input, ctx, CancellationToken::new())
            .await?
        {
            ToolExecution::Outcome(o) => Ok(o),
            ToolExecution::Action(_) => Err(ToolError::Internal("unexpected".into())),
        }
    }

    #[tokio::test]
    async fn invalid_utf8_is_rejected_without_changing_any_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f.txt");
        let mut bytes = b"replace me\n".to_vec();
        bytes.extend(vec![b'x'; 8192]);
        bytes.push(0xe9);
        std::fs::write(&path, &bytes).unwrap();
        let result = edit(
            &ctx_in(dir.path()),
            json!({
                "path": "f.txt", "old_string": "replace me", "new_string": "changed"
            }),
        )
        .await;
        assert!(
            matches!(result, Err(ToolError::InvalidInput(ref error)) if error.contains("not valid UTF-8"))
        );
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }

    #[tokio::test]
    async fn oversized_file_and_replacement_are_rejected_without_writing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f.txt");
        std::fs::write(&path, b"x").unwrap();
        let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        file.set_len(crate::fs::MAX_FILE_BYTES + 1).unwrap();
        let result = edit(
            &ctx_in(dir.path()),
            json!({
                "path": "f.txt", "old_string": "x", "new_string": "y"
            }),
        )
        .await;
        assert!(
            matches!(result, Err(ToolError::InvalidInput(ref error)) if error.contains("size limit"))
        );
        assert_eq!(
            file.metadata().unwrap().len(),
            crate::fs::MAX_FILE_BYTES + 1
        );
        std::fs::write(&path, b"xx").unwrap();
        let result = edit(&ctx_in(dir.path()), json!({
            "path": "f.txt", "old_string": "x", "new_string": "y".repeat(crate::fs::MAX_FILE_BYTES as usize), "replace_all": true
        })).await;
        assert!(
            matches!(result, Err(ToolError::InvalidInput(ref error)) if error.contains("size limit"))
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"xx");
    }

    #[tokio::test]
    async fn replaces_unique_match_and_returns_diff() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f.rs"), "fn a() {}\nfn b() {}\n").unwrap();
        let ctx = ctx_in(dir.path());
        let out = edit(
            &ctx,
            json!({"path": "f.rs", "old_string": "fn b()", "new_string": "fn c()"}),
        )
        .await
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("f.rs")).unwrap(),
            "fn a() {}\nfn c() {}\n"
        );
        assert!(out.content.contains("-fn b() {}"));
        assert!(out.content.contains("+fn c() {}"));
        assert!(out.content.starts_with("Edited f.rs (1 replacement)"));
    }

    #[tokio::test]
    async fn ambiguous_match_fails_unless_replace_all() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f.txt"), "x x x").unwrap();
        let ctx = ctx_in(dir.path());
        let err = edit(
            &ctx,
            json!({"path": "f.txt", "old_string": "x", "new_string": "y"}),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput(ref m) if m.contains("3 times")));
        let out = edit(
            &ctx,
            json!({"path": "f.txt", "old_string": "x", "new_string": "y", "replace_all": true}),
        )
        .await
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("f.txt")).unwrap(),
            "y y y"
        );
        assert_eq!(out.structured.unwrap()["replacements"], 3);
    }

    #[tokio::test]
    async fn missing_and_identical_are_invalid() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f.txt"), "abc").unwrap();
        let ctx = ctx_in(dir.path());
        assert!(matches!(
            edit(
                &ctx,
                json!({"path": "f.txt", "old_string": "zzz", "new_string": "y"})
            )
            .await,
            Err(ToolError::InvalidInput(_))
        ));
        assert!(matches!(
            edit(
                &ctx,
                json!({"path": "f.txt", "old_string": "a", "new_string": "a"})
            )
            .await,
            Err(ToolError::InvalidInput(_))
        ));
    }
}
