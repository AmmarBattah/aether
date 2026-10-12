//! Depth-limited directory listing that honors .gitignore.

use std::path::Path;

use aether_core::tool::{ResourceKey, SideEffectClass, ToolDescriptor, ToolOutcome};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use crate::fs::{resolve_path, resource_paths};
use crate::tool::{Tool, ToolContext, ToolError, ToolExecution, parse_input};

const DEFAULT_DEPTH: usize = 2;
const DEFAULT_MAX_ENTRIES: usize = 500;

#[derive(Debug)]
pub struct ListDirTool;

#[derive(Debug, Deserialize)]
struct Input {
    #[serde(default = "dot")]
    path: String,
    #[serde(default)]
    depth: Option<usize>,
    #[serde(default)]
    include_hidden: bool,
    #[serde(default)]
    max_entries: Option<usize>,
}

fn dot() -> String {
    ".".into()
}

#[async_trait]
impl Tool for ListDirTool {
    fn descriptor(&self) -> ToolDescriptor {
        super::descriptor(
            "list_dir",
            "List files and directories under a path, honoring .gitignore. Directories end \
             with a slash. Limited depth; raise depth or list a subdirectory for more.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "default": ".", "description": "Directory to list."},
                    "depth": {"type": "integer", "minimum": 1, "default": 2},
                    "include_hidden": {"type": "boolean", "default": false},
                    "max_entries": {"type": "integer", "minimum": 1, "default": 500}
                },
                "additionalProperties": false
            }),
            SideEffectClass::ReadOnly,
            false,
        )
    }

    fn resources(&self, input: &Value, ctx: &ToolContext) -> Vec<ResourceKey> {
        resource_paths(ctx, input["path"].as_str().unwrap_or("."))
            .into_iter()
            .map(ResourceKey::dir)
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
            depth,
            include_hidden,
            max_entries,
        } = parse_input(input)?;
        let root = resolve_path(ctx, &path)?;
        if !crate::fs::file_type(&root).is_ok_and(|kind| kind == nix::sys::stat::SFlag::S_IFDIR) {
            return Err(ToolError::InvalidInput(format!(
                "{path} is not a directory"
            )));
        }
        let depth = depth.unwrap_or(DEFAULT_DEPTH).max(1);
        let max_entries = max_entries.unwrap_or(DEFAULT_MAX_ENTRIES).max(1);
        let (mut entries, depth_notice) =
            tokio::task::spawn_blocking(move || walk(&root, depth, include_hidden, max_entries))
                .await
                .map_err(|e| ToolError::Internal(e.to_string()))??;
        let capped = entries.len() > max_entries;
        entries.truncate(max_entries);
        let total = entries.len();
        let mut content = entries.join("\n");
        if content.is_empty() {
            content = "(empty)".into();
        }
        if capped {
            content.push_str(&format!("\n[showing first {max_entries} entries]"));
        }
        if let Some(notice) = &depth_notice {
            content.push('\n');
            content.push_str(notice);
        }
        Ok(ToolExecution::Outcome(ToolOutcome {
            content,
            structured: Some(json!({ "entries": total, "capped": capped })),
            truncated: capped || depth_notice.is_some(),
            side_effect_occurred: Some(false),
            ..Default::default()
        }))
    }
}

fn walk(
    root: &Path,
    depth: usize,
    include_hidden: bool,
    max_entries: usize,
) -> Result<(Vec<String>, Option<String>), ToolError> {
    let mut out = Vec::new();
    let mut walker = crate::walk::Walk::new(
        root,
        crate::walk::Options {
            max_depth: Some(depth),
            include_hidden,
            sorted: true,
        },
    )?;
    for entry in walker.by_ref() {
        let entry = entry?;
        if entry.depth() == 0 {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(root)
            .unwrap_or(entry.path())
            .display()
            .to_string();
        let is_dir = entry.is_dir();
        out.push(if is_dir { format!("{rel}/") } else { rel });
        if out.len() > max_entries {
            break;
        }
    }
    Ok((out, walker.depth_limit_notice()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::ctx_in;

    #[test]
    fn rejects_a_root_symlink_swapped_after_scope_resolution() {
        let workspace = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::create_dir(workspace.path().join("root")).unwrap();
        std::fs::write(outside.path().join("outside-secret"), "secret").unwrap();
        let root = resolve_path(&ctx_in(workspace.path()), "root").unwrap();
        std::fs::rename(&root, workspace.path().join("moved")).unwrap();
        std::os::unix::fs::symlink(outside.path(), &root).unwrap();
        let result = walk(&root, 2, true, 100).map(|(entries, _)| entries.join("\n"));
        assert!(result.is_err(), "swapped root must fail: {result:?}");
    }

    #[tokio::test]
    async fn truncation_requires_an_entry_beyond_the_cap() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a"), "").unwrap();
        std::fs::write(dir.path().join("b"), "").unwrap();
        let ctx = ctx_in(dir.path());
        for (cap, truncated) in [(3, false), (2, false), (1, true)] {
            let result = ListDirTool
                .execute(json!({"max_entries": cap}), &ctx, CancellationToken::new())
                .await
                .unwrap();
            let ToolExecution::Outcome(out) = result else {
                panic!()
            };
            assert_eq!(out.truncated, truncated, "cap={cap}");
            assert_eq!(out.structured.as_ref().unwrap()["entries"], cap.min(2));
            assert_eq!(out.structured.as_ref().unwrap()["capped"], truncated);
            assert_eq!(out.content.contains("showing first"), truncated);
        }
    }

    #[tokio::test]
    async fn lists_with_depth_and_gitignore() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src/deep/deeper")).unwrap();
        std::fs::write(dir.path().join("src/main.rs"), "").unwrap();
        std::fs::write(dir.path().join("src/deep/x.rs"), "").unwrap();
        std::fs::write(dir.path().join("src/deep/deeper/y.rs"), "").unwrap();
        std::fs::create_dir_all(dir.path().join("target")).unwrap();
        std::fs::write(dir.path().join("target/junk"), "").unwrap();
        std::fs::write(dir.path().join(".gitignore"), "target/\n").unwrap();
        std::fs::write(dir.path().join(".hidden"), "").unwrap();
        let ctx = ctx_in(dir.path());
        let r = ListDirTool
            .execute(json!({"depth": 2}), &ctx, CancellationToken::new())
            .await
            .unwrap();
        let ToolExecution::Outcome(o) = r else {
            panic!()
        };
        assert!(o.content.contains("src/"));
        assert!(o.content.contains("src/main.rs"));
        assert!(o.content.contains("src/deep/"));
        assert!(
            !o.content.contains("deeper"),
            "depth 2 should not descend to deeper"
        );
        assert!(!o.content.contains("target"), "gitignored");
        assert!(!o.content.contains(".hidden"));
        let r = ListDirTool
            .execute(
                json!({"path": "src", "depth": 5, "include_hidden": true}),
                &ctx,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        let ToolExecution::Outcome(o) = r else {
            panic!()
        };
        assert!(o.content.contains("deep/deeper/y.rs"));
    }

    #[tokio::test]
    async fn rejects_files_and_out_of_scope() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f"), "").unwrap();
        let ctx = ctx_in(dir.path());
        assert!(matches!(
            ListDirTool
                .execute(json!({"path": "f"}), &ctx, CancellationToken::new())
                .await,
            Err(ToolError::InvalidInput(_))
        ));
        assert!(matches!(
            ListDirTool
                .execute(json!({"path": "/"}), &ctx, CancellationToken::new())
                .await,
            Err(ToolError::OutOfScope(_))
        ));
    }
}
