use super::*;
use crate::testing::ctx_in;
use crate::tool::ToolContext;
use serde_json::json;
use std::os::unix::fs::FileTypeExt;
use tokio_util::sync::CancellationToken;

fn check_special_file(tool: &dyn Tool, input: Value) {
    use std::os::unix::fs::OpenOptionsExt;

    let dir = tempfile::tempdir().unwrap();
    let fifo = dir.path().join("pipe");
    nix::unistd::mkfifo(
        &fifo,
        nix::sys::stat::Mode::S_IRUSR | nix::sys::stat::Mode::S_IWUSR,
    )
    .unwrap();
    let peer = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(nix::libc::O_NONBLOCK)
        .open(&fifo)
        .unwrap();
    let ctx: ToolContext = ctx_in(dir.path());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let result = runtime.block_on(async {
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            tool.execute(input, &ctx, CancellationToken::new()),
        )
        .await
    });
    drop(peer);
    runtime.shutdown_timeout(std::time::Duration::from_millis(100));
    assert!(
        matches!(result, Ok(Err(ToolError::InvalidInput(ref error))) if error.contains("not a regular file")),
        "special files must fail visibly without opening or blocking: {result:?}"
    );
    assert!(std::fs::metadata(fifo).unwrap().file_type().is_fifo());
}

#[test]
fn read_file_rejects_fifo_before_opening() {
    check_special_file(&read_file::ReadFileTool::default(), json!({"path": "pipe"}));
}

#[test]
fn edit_file_rejects_fifo_before_opening() {
    check_special_file(
        &edit_file::EditFileTool::default(),
        json!({"path": "pipe", "old_string": "x", "new_string": "y"}),
    );
}

#[test]
fn write_file_rejects_fifo_before_opening() {
    check_special_file(
        &write_file::WriteFileTool,
        json!({"path": "pipe", "content": "x"}),
    );
}

#[test]
fn grep_rejects_an_explicit_fifo_path() {
    check_special_file(&grep::GrepTool, json!({"path": "pipe", "pattern": "x"}));
}

#[tokio::test]
async fn directory_tools_do_not_open_special_entries() {
    let dir = tempfile::tempdir().unwrap();
    nix::unistd::mkfifo(&dir.path().join("pipe"), nix::sys::stat::Mode::S_IRUSR).unwrap();
    let ctx = ctx_in(dir.path());
    for (tool, input) in [
        (&list_dir::ListDirTool as &dyn Tool, json!({"path": "."})),
        (
            &grep::GrepTool as &dyn Tool,
            json!({"path": ".", "pattern": "x"}),
        ),
    ] {
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            tool.execute(input, &ctx, CancellationToken::new()),
        )
        .await
        .unwrap()
        .unwrap();
    }
    assert!(matches!(
        list_dir::ListDirTool
            .execute(json!({"path": "pipe"}), &ctx, CancellationToken::new())
            .await,
        Err(ToolError::InvalidInput(_))
    ));
}
