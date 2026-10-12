//! Create or overwrite a file inside the workspace.

use aether_core::tool::{ResourceKey, SideEffectClass, ToolDescriptor, ToolOutcome};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use crate::fs::{resolve_path, resource_paths};
use crate::tool::{Tool, ToolContext, ToolError, ToolExecution, parse_input};

#[derive(Debug)]
pub struct WriteFileTool;

#[derive(Debug, Deserialize)]
struct Input {
    path: String,
    content: String,
}

#[async_trait]
impl Tool for WriteFileTool {
    fn descriptor(&self) -> ToolDescriptor {
        super::descriptor(
            "write_file",
            "Write a regular file in the workspace, creating parent directories and replacing any \
             existing content. Prefer edit_file for changes to existing files.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "File path, relative to the workspace or absolute."},
                    "content": {"type": "string", "description": "Full file content."}
                },
                "required": ["path", "content"],
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
        let Input { path, content } = parse_input(input)?;
        if content.len() as u64 > crate::fs::MAX_FILE_BYTES {
            return Err(ToolError::InvalidInput(
                "write content exceeds the file-tool size limit of 8 MiB".into(),
            ));
        }
        let resolved = resolve_path(ctx, &path)?;
        let write_path = resolved.clone();
        let write_bytes = content.as_bytes().to_vec();
        let existed = tokio::task::spawn_blocking(move || {
            crate::fs::write_regular_file(&write_path, &write_bytes)
        })
        .await
        .map_err(|error| ToolError::Internal(error.to_string()))??;
        let verb = if existed { "Overwrote" } else { "Created" };
        Ok(ToolExecution::Outcome(ToolOutcome {
            content: format!(
                "{verb} {} ({} bytes)",
                super::display_path(ctx, &resolved),
                content.len()
            ),
            side_effect_occurred: Some(true),
            ..Default::default()
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::ctx_in;

    #[tokio::test]
    async fn writes_and_creates_parents() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx_in(dir.path());
        let r = WriteFileTool
            .execute(
                json!({"path": "a/b/c.txt", "content": "hi"}),
                &ctx,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("a/b/c.txt")).unwrap(),
            "hi"
        );
        match r {
            ToolExecution::Outcome(o) => assert!(o.content.starts_with("Created a/b/c.txt")),
            _ => panic!(),
        }
        let r = WriteFileTool
            .execute(
                json!({"path": "a/b/c.txt", "content": "yo"}),
                &ctx,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        match r {
            ToolExecution::Outcome(o) => assert!(o.content.starts_with("Overwrote")),
            _ => panic!(),
        }
    }

    #[tokio::test]
    async fn writes_through_an_in_scope_symlink_without_replacing_the_alias() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("real");
        let alias = dir.path().join("alias");
        std::fs::write(&target, b"old").unwrap();
        std::os::unix::fs::symlink("real", &alias).unwrap();
        WriteFileTool
            .execute(
                json!({"path": "alias", "content": "new"}),
                &ctx_in(dir.path()),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"new");
        assert_eq!(
            std::fs::read_link(&alias).unwrap(),
            std::path::Path::new("real")
        );
    }

    #[tokio::test]
    async fn refuses_out_of_scope() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx_in(dir.path());
        let r = WriteFileTool
            .execute(
                json!({"path": "../escape.txt", "content": "x"}),
                &ctx,
                CancellationToken::new(),
            )
            .await;
        assert!(matches!(r, Err(ToolError::OutOfScope(_))));
    }

    #[tokio::test]
    async fn refuses_dangling_symlink_escape_without_creating_target() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let target = outside.path().join("new.txt");
        std::os::unix::fs::symlink(&target, dir.path().join("alias")).unwrap();
        let result = WriteFileTool
            .execute(
                json!({"path": "alias", "content": "escape"}),
                &ctx_in(dir.path()),
                CancellationToken::new(),
            )
            .await;
        assert!(
            matches!(result, Err(ToolError::OutOfScope(_))),
            "{result:?}"
        );
        assert!(
            !target.exists(),
            "an out-of-scope target must never be created"
        );
    }
}

#[cfg(test)]
mod size_tests {
    use super::*;
    #[tokio::test]
    async fn oversized_write_does_not_replace_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("file"), b"keep").unwrap();
        let result = WriteFileTool.execute(json!({"path":"file", "content":"x".repeat(crate::fs::MAX_FILE_BYTES as usize + 1)}), &crate::testing::ctx_in(dir.path()), CancellationToken::new()).await;
        assert!(matches!(result, Err(ToolError::InvalidInput(_))));
        assert_eq!(std::fs::read(dir.path().join("file")).unwrap(), b"keep");
        WriteFileTool
            .execute(
                json!({"path":"file", "content":"x".repeat(crate::fs::MAX_FILE_BYTES as usize)}),
                &crate::testing::ctx_in(dir.path()),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(
            std::fs::metadata(dir.path().join("file")).unwrap().len(),
            crate::fs::MAX_FILE_BYTES
        );
    }
}
