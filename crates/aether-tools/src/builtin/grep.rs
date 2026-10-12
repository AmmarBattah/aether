//! Regex search across workspace files.

use std::path::Path;

use aether_core::tool::{ResourceKey, SideEffectClass, ToolDescriptor, ToolOutcome};
use async_trait::async_trait;
use globset::GlobBuilder;
use regex::RegexBuilder;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use crate::fs::{resolve_path, resource_paths};
use crate::tool::{Tool, ToolContext, ToolError, ToolExecution, parse_input};

const DEFAULT_MAX_RESULTS: usize = 100;
/// Files larger than this are skipped; they are rarely source and dominate time.
const MAX_FILE_BYTES: u64 = 4 << 20;

#[derive(Debug)]
pub struct GrepTool;

#[derive(Debug, Deserialize)]
struct Input {
    pattern: String,
    #[serde(default = "dot")]
    path: String,
    #[serde(default)]
    glob: Option<String>,
    #[serde(default)]
    ignore_case: bool,
    #[serde(default)]
    context: usize,
    #[serde(default)]
    max_results: Option<usize>,
}

fn dot() -> String {
    ".".into()
}

#[async_trait]
impl Tool for GrepTool {
    fn descriptor(&self) -> ToolDescriptor {
        super::descriptor(
            "grep",
            "Search regular file contents with a regular expression. Output is `path:line:col: text`, \
             with optional context lines. Honors .gitignore and skips binary files.",
            json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string", "description": "Rust-syntax regular expression."},
                    "path": {"type": "string", "default": ".", "description": "Regular file or directory to search."},
                    "glob": {"type": "string", "description": "Only search files matching this glob, e.g. `*.rs`."},
                    "ignore_case": {"type": "boolean", "default": false},
                    "context": {"type": "integer", "minimum": 0, "default": 0, "description": "Lines of context around each match."},
                    "max_results": {"type": "integer", "minimum": 1, "default": 100}
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
            glob,
            ignore_case,
            context,
            max_results,
        } = parse_input(input)?;
        let root = resolve_path(ctx, &path)?;
        let kind = crate::fs::file_type(&root)?;
        if !matches!(
            kind,
            nix::sys::stat::SFlag::S_IFREG | nix::sys::stat::SFlag::S_IFDIR
        ) {
            return Err(ToolError::InvalidInput(format!(
                "{path} is not a regular file or directory"
            )));
        }
        let regex = RegexBuilder::new(&pattern)
            .case_insensitive(ignore_case)
            .build()
            .map_err(|e| ToolError::InvalidInput(format!("bad regex `{pattern}`: {e}")))?;
        let glob = glob
            .map(|g| {
                GlobBuilder::new(&g)
                    .literal_separator(false)
                    .build()
                    .map(|g| g.compile_matcher())
                    .map_err(|e| ToolError::InvalidInput(format!("bad glob `{g}`: {e}")))
            })
            .transpose()?;
        let max_results = max_results.unwrap_or(DEFAULT_MAX_RESULTS).max(1);
        let search = Search {
            regex,
            glob,
            context,
            max_results,
        };
        let (lines, total, capped, depth_notice) =
            tokio::task::spawn_blocking(move || search.run(&root))
                .await
                .map_err(|e| ToolError::Internal(e.to_string()))??;
        let mut content = if lines.is_empty() {
            format!("No matches for `{pattern}`")
        } else {
            lines.join("\n")
        };
        if capped {
            content.push_str(&format!(
                "\n[stopped after {max_results} matches; narrow the pattern or path]"
            ));
        }
        if let Some(notice) = &depth_notice {
            content.push('\n');
            content.push_str(notice);
        }
        Ok(ToolExecution::Outcome(ToolOutcome {
            content,
            structured: Some(json!({ "matches": total, "capped": capped })),
            truncated: capped || depth_notice.is_some(),
            side_effect_occurred: Some(false),
            ..Default::default()
        }))
    }
}

struct Search {
    regex: regex::Regex,
    glob: Option<globset::GlobMatcher>,
    context: usize,
    max_results: usize,
}

impl Search {
    fn run(&self, root: &Path) -> Result<(Vec<String>, usize, bool, Option<String>), ToolError> {
        let mut out = Vec::new();
        let mut total = 0usize;
        let mut walker = crate::walk::Walk::new(root, crate::walk::Options::default())?;
        'files: for entry in walker.by_ref() {
            let entry = entry?;
            if !entry.is_file() {
                continue;
            }
            if entry.size().is_none_or(|size| size > MAX_FILE_BYTES) {
                continue;
            }
            let base = if entry.depth() == 0 {
                root.parent().unwrap_or(root)
            } else {
                root
            };
            let rel = entry.path().strip_prefix(base).unwrap_or(entry.path());
            if self
                .glob
                .as_ref()
                .is_some_and(|g| !g.is_match(rel) && !g.is_match(entry.file_name()))
            {
                continue;
            }
            let Ok(bytes) = entry.read() else {
                continue;
            };
            if super::looks_binary(&bytes) {
                continue;
            }
            let text = String::from_utf8_lossy(&bytes);
            let lines: Vec<&str> = text.lines().collect();
            let rel = rel.display();
            for (i, line) in lines.iter().enumerate() {
                let Some(m) = self.regex.find(line) else {
                    continue;
                };
                total += 1;
                if total > self.max_results {
                    break 'files;
                }
                let col = line[..m.start()].chars().count() + 1;
                let before = i.saturating_sub(self.context);
                for (j, l) in lines.iter().enumerate().take(i).skip(before) {
                    out.push(format!("{rel}-{}- {l}", j + 1));
                }
                out.push(format!("{rel}:{}:{col}: {line}", i + 1));
                for (j, l) in lines.iter().enumerate().skip(i + 1).take(self.context) {
                    out.push(format!("{rel}-{}- {l}", j + 1));
                }
                if self.context > 0 {
                    out.push("--".into());
                }
            }
        }
        let capped = total > self.max_results;
        Ok((
            out,
            total.min(self.max_results),
            capped,
            walker.depth_limit_notice(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::ctx_in;

    async fn grep(ctx: &ToolContext, input: Value) -> Result<ToolOutcome, ToolError> {
        match GrepTool
            .execute(input, ctx, CancellationToken::new())
            .await?
        {
            ToolExecution::Outcome(o) => Ok(o),
            ToolExecution::Action(_) => Err(ToolError::Internal("unexpected".into())),
        }
    }

    #[test]
    fn rejects_a_root_symlink_swapped_after_scope_resolution() {
        let workspace = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::create_dir(workspace.path().join("root")).unwrap();
        std::fs::write(outside.path().join("outside-secret"), "secret").unwrap();
        let root = resolve_path(&ctx_in(workspace.path()), "root").unwrap();
        std::fs::rename(&root, workspace.path().join("moved")).unwrap();
        std::os::unix::fs::symlink(outside.path(), &root).unwrap();
        let result = Search {
            regex: RegexBuilder::new("secret").build().unwrap(),
            glob: None,
            context: 0,
            max_results: 100,
        }
        .run(&root)
        .map(|(lines, _, _, _)| lines.join("\n"));
        assert!(result.is_err(), "swapped root must fail: {result:?}");
    }

    #[tokio::test]
    async fn finds_matches_with_context_and_glob() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/a.rs"), "one\nfn target() {}\nthree\n").unwrap();
        std::fs::write(dir.path().join("notes.md"), "Target here\n").unwrap();
        std::fs::write(dir.path().join("bin"), [0u8, 116, 97, 114, 103, 101, 116]).unwrap();
        let ctx = ctx_in(dir.path());
        let out = grep(&ctx, json!({"pattern": "target", "context": 1}))
            .await
            .unwrap();
        assert!(out.content.contains("src/a.rs:2:4: fn target() {}"));
        assert!(out.content.contains("src/a.rs-1- one"));
        assert!(out.content.contains("src/a.rs-3- three"));
        assert!(
            !out.content.contains("notes.md"),
            "case sensitive by default"
        );
        let out = grep(
            &ctx,
            json!({"pattern": "target", "ignore_case": true, "glob": "*.md"}),
        )
        .await
        .unwrap();
        assert!(out.content.contains("notes.md:1:1: Target here"));
        assert!(!out.content.contains("a.rs"));
        assert_eq!(out.structured.unwrap()["matches"], 1);
    }

    #[tokio::test]
    async fn caps_results_and_rejects_bad_regex() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f"), "x\nx\nx\n").unwrap();
        let ctx = ctx_in(dir.path());
        let out = grep(&ctx, json!({"pattern": "x", "max_results": 2}))
            .await
            .unwrap();
        assert!(out.truncated);
        assert!(out.content.contains("[stopped after 2 matches"));
        assert!(matches!(
            grep(&ctx, json!({"pattern": "("})).await,
            Err(ToolError::InvalidInput(_))
        ));
        let out = grep(&ctx, json!({"pattern": "nomatch"})).await.unwrap();
        assert!(out.content.starts_with("No matches"));
    }
}
