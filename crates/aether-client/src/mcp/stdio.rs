//! Serve MCP over a pair of streams: newline-delimited JSON-RPC, one message
//! per line. On the daemon's socket this is stdin and stdout, and nothing
//! else may be written to stdout.
//!
//! The peer here is whoever started this process, so it holds the operator's
//! surface. It is still read under a cap: every other read from a peer in
//! Aether is bounded, and a peer that never sends a newline would otherwise
//! grow the process without limit.

use tokio::io::{
    AsyncBufRead, AsyncBufReadExt as _, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _,
};

use super::{Answer, CommandSink, Surface};

/// Longest request line accepted. A JSON-RPC call is small; a longer one is
/// a mistake or an attack, and reading it would be the vulnerability.
pub const MAX_LINE_BYTES: usize = 1 << 20;

/// Answer requests until the peer closes the stream.
pub async fn serve(
    sink: &dyn CommandSink,
    reader: impl AsyncBufRead + Unpin,
    mut writer: impl AsyncWrite + Unpin,
) -> anyhow::Result<()> {
    // Capped rather than `lines()`, which reads to the next newline however
    // far away that is.
    let mut lines = reader.take(MAX_LINE_BYTES as u64 + 1).lines();
    while let Some(line) = lines.next_line().await? {
        if line.len() > MAX_LINE_BYTES {
            anyhow::bail!("a request line passed the {MAX_LINE_BYTES}-byte cap");
        }
        // `take` bounds the whole stream, so the budget is restored per line.
        lines.get_mut().set_limit(MAX_LINE_BYTES as u64 + 1);
        if line.trim().is_empty() {
            continue;
        }
        let request: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(error = %e, "mcp server: malformed request");
                continue;
            }
        };
        let response = match super::handle(sink, &Surface::Fleet, &request).await {
            Answer::Reply(response) => response,
            Answer::None => continue, // a notification takes no reply
            // The operator's surface offers every tool it accepts, so this
            // is unreachable; refusing loudly beats answering as if it ran.
            Answer::Denied(why) => anyhow::bail!("the operator surface refused a call: {why}"),
        };
        let mut text = serde_json::to_string(&response)?;
        text.push('\n');
        writer.write_all(text.as_bytes()).await?;
        writer.flush().await?;
    }
    Ok(())
}
