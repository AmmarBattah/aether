//! Find files by glob pattern, newest first.

use std::path::Path;
use std::time::SystemTime;

use aether_core::tool::{ResourceKey, SideEffectClass, ToolDescriptor, ToolOutcome};
use async_trait::async_trait;
use globset::GlobBuilder;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use crate::fs::{resolve_path, resource_paths};
use crate::tool::{Tool, ToolContext, ToolError, ToolExecution, parse_input};

const DEFAULT_MAX_RESULTS: usize = 200;

#[derive(Debug)]
pub struct GlobTool;

#[derive(Debug, Deserialize)]
struct Input {
    pattern: String,
    #[serde(default = "dot")]
    path: String,
    #[serde(default)]
    max_results: Option<usize>,
}

fn dot() -> String {
    ".".into()
}

#[async_trait]
impl Tool for GlobTool {
    fn descriptor(&self) -> ToolDescriptor {
        super::descriptor(
            "glob",
            "Find files matching a glob pattern such as `**/*.rs` or `src/**/test_*.py`, \
             relative to a directory. Results are sorted by modification time, newest first, \
             and honor .gitignore.",
            json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string", "description": "Glob pattern, matched against paths relative to `path`."},
                    "path": {"type": "string", "default": ".", "description": "Directory to search."},
                    "max_results": {"type": "integer", "minimum": 1, "default": 200}
                },
                "required": ["pattern"],
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
            pattern,
            path,
            max_results,
        } = parse_input(input)?;
        let root = resolve_path(ctx, &path)?;
        if !crate::fs::file_type(&root).is_ok_and(|kind| kind == nix::sys::stat::SFlag::S_IFDIR) {
            return Err(ToolError::InvalidInput(format!(
                "{path} is not a directory"
            )));
        }
        let matcher = GlobBuilder::new(&pattern)
            .literal_separator(false)
            .build()
            .map_err(|e| ToolError::InvalidInput(format!("bad glob `{pattern}`: {e}")))?
            .compile_matcher();
        let max_results = max_results.unwrap_or(DEFAULT_MAX_RESULTS).max(1);
        let (files, total, depth_notice) =
            tokio::task::spawn_blocking(move || search(&root, &matcher, max_results))
                .await
                .map_err(|e| ToolError::Internal(e.to_string()))??;
        let mut content = if files.is_empty() {
            format!("No files match `{pattern}`")
        } else {
            let mut c = files.join("\n");
            if total > files.len() {
                c.push_str(&format!("\n[showing {} of {total} matches]", files.len()));
            }
            c
        };
        if let Some(notice) = &depth_notice {
            content.push('\n');
            content.push_str(notice);
        }
        Ok(ToolExecution::Outcome(ToolOutcome {
            content,
            structured: Some(json!({ "matches": total })),
            truncated: total > files.len() || depth_notice.is_some(),
            side_effect_occurred: Some(false),
            ..Default::default()
        }))
    }
}

fn search(
    root: &Path,
    matcher: &globset::GlobMatcher,
    max_results: usize,
) -> Result<(Vec<String>, usize, Option<String>), ToolError> {
    let mut hits: Vec<(SystemTime, String)> = Vec::new();
    let mut walker = crate::walk::Walk::new(root, crate::walk::Options::default())?;
    for entry in walker.by_ref() {
        let entry = entry?;
        if !entry.is_file() {
            continue;
        }
        let rel = entry.path().strip_prefix(root).unwrap_or(entry.path());
        if !matcher.is_match(rel) {
            continue;
        }
        let mtime = entry.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        hits.push((mtime, rel.display().to_string()));
    }
    let total = hits.len();
    hits.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    Ok((
        hits.into_iter().take(max_results).map(|(_, p)| p).collect(),
        total,
        walker.depth_limit_notice(),
    ))
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
        let result = search(
            &root,
            &GlobBuilder::new("**").build().unwrap().compile_matcher(),
            100,
        )
        .map(|(entries, _, _)| entries.join("\n"));
        assert!(result.is_err(), "swapped root must fail: {result:?}");
    }

    #[tokio::test]
    async fn matches_and_sorts_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src/sub")).unwrap();
        std::fs::write(dir.path().join("src/old.rs"), "").unwrap();
        std::fs::write(dir.path().join("src/sub/new.rs"), "").unwrap();
        std::fs::write(dir.path().join("README.md"), "").unwrap();
        let old = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
        std::fs::File::open(dir.path().join("src/old.rs"))
            .unwrap()
            .set_modified(old)
            .unwrap();
        let ctx = ctx_in(dir.path());
        let r = GlobTool
            .execute(
                json!({"pattern": "**/*.rs"}),
                &ctx,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        let ToolExecution::Outcome(o) = r else {
            panic!()
        };
        assert_eq!(o.content, "src/sub/new.rs\nsrc/old.rs");
        let r = GlobTool
            .execute(
                json!({"pattern": "*.rs", "path": "src", "max_results": 1}),
                &ctx,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        let ToolExecution::Outcome(o) = r else {
            panic!()
        };
        assert!(o.content.contains("[showing 1 of 2 matches]"));
    }

    #[tokio::test]
    async fn bad_pattern_and_no_match() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx_in(dir.path());
        assert!(matches!(
            GlobTool
                .execute(
                    json!({"pattern": "[invalid"}),
                    &ctx,
                    CancellationToken::new()
                )
                .await,
            Err(ToolError::InvalidInput(_))
        ));
        let r = GlobTool
            .execute(json!({"pattern": "*.zig"}), &ctx, CancellationToken::new())
            .await
            .unwrap();
        let ToolExecution::Outcome(o) = r else {
            panic!()
        };
        assert!(o.content.starts_with("No files match"));
    }
}
