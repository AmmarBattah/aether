//! The MCP bridge: a stdio JSON-RPC server an engine spawns, forwarding
//! every tool call to the daemon as an authenticated engine tool request.
//!
//! stdout is the protocol channel; nothing else may be written to it.

use std::path::PathBuf;

use aether_client::Client;
use aether_core::id::AgentId;
use aether_protocol::{Command, Reply};
use serde_json::{Value, json};
use tokio::io::{AsyncBufRead, AsyncWrite, AsyncWriteExt as _};
use tokio::task::JoinSet;

const PROTOCOL_VERSION: &str = "2025-06-18";
const SUPPORTED_PROTOCOL_VERSIONS: &[&str] =
    &["2024-11-05", "2025-03-26", "2025-06-18", "2025-11-25"];

/// The permission prompt handler Claude Code calls through
/// `--permission-prompt-tool`. It is the bridge's own, not the daemon's.
fn permission_tool() -> Value {
    json!({
        "name": "permission",
        "description": "Ask Aether whether a tool call may run. Used by the engine's permission prompts.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "tool_name": { "type": "string" },
                "input": { "type": "object" },
                "tool_use_id": { "type": "string" }
            },
            "required": ["tool_name", "input"]
        }
    })
}

/// What the bridge advertises for this turn: the tools the daemon says this
/// agent may use through Aether - its harness tools and any MCP server's -
/// plus the permission handler. Asked per call so a server discovered after
/// the turn started is offered too.
async fn tool_list(client: &Client, agent: AgentId, token: &str) -> anyhow::Result<Vec<Value>> {
    let Reply::EngineTools { tools } = client
        .request(Command::EngineTools {
            agent,
            token: token.to_owned(),
        })
        .await?
    else {
        anyhow::bail!("daemon did not answer with a tool list");
    };
    let mut list: Vec<Value> = tools
        .into_iter()
        .map(|spec| {
            json!({
                "name": spec.name,
                "description": spec.description,
                "inputSchema": spec.input_schema
            })
        })
        .collect();
    list.push(permission_tool());
    Ok(list)
}

// Reject excess daemon work rather than queueing behind indefinite approval waits.
const MAX_IN_FLIGHT: usize = 32;

/// Serve MCP on stdin/stdout until the engine closes the pipe.
pub async fn run(socket: PathBuf, agent: AgentId, token: String) -> anyhow::Result<()> {
    Client::connect(&socket).await?;
    serve(
        socket,
        agent,
        token,
        tokio::io::BufReader::new(tokio::io::stdin()),
        tokio::io::stdout(),
    )
    .await
}

async fn serve(
    socket: PathBuf,
    agent: AgentId,
    token: String,
    input: impl AsyncBufRead + Unpin,
    mut output: impl AsyncWrite + Unpin,
) -> anyhow::Result<()> {
    let mut lines = crate::output::BoundedLines::new(input);
    let mut pending = JoinSet::new();
    loop {
        let response = tokio::select! {
            Some(completed) = pending.join_next(), if !pending.is_empty() => completed?,
            line = lines.next_line() => {
                let Some(line) = line? else {
                    pending.shutdown().await;
                    return Ok(());
                };
                if line.trim().is_empty() {
                    continue;
                }
                let req: Value = match serde_json::from_str(&line) {
                    Ok(req) => req,
                    Err(error) => {
                        tracing::warn!(%error, "bridge: malformed request");
                        continue;
                    }
                };
                let Some(id) = req.get("id") else { continue };
                if matches!(req["method"].as_str(), Some("tools/call" | "tools/list")) {
                    if pending.len() >= MAX_IN_FLIGHT {
                        json!({ "jsonrpc": "2.0", "id": id, "error": {
                            "code": -32603, "message": "bridge has too many in-flight daemon requests; retry after one completes"
                        } })
                    } else {
                        let socket = socket.clone();
                        let token = token.clone();
                        pending.spawn(async move { handle_request(&socket, agent, &token, req).await });
                        continue;
                    }
                } else {
                    handle_request(&socket, agent, &token, req).await
                }
            }
        };
        let mut text = serde_json::to_string(&response)?;
        text.push('\n');
        output.write_all(text.as_bytes()).await?;
        output.flush().await?;
    }
}

async fn handle_request(
    socket: &std::path::Path,
    agent: AgentId,
    token: &str,
    req: Value,
) -> Value {
    let id = req["id"].clone();
    let method = req.get("method").and_then(Value::as_str).unwrap_or("");
    match method {
        "initialize" => tracing::info!(
            client = %req["params"]["clientInfo"]["name"].as_str().unwrap_or("?"),
            %agent,
            "bridge: engine connected"
        ),
        "tools/call" => tracing::info!(
            tool = %req["params"]["name"].as_str().unwrap_or("?"),
            %agent,
            "bridge: tool call"
        ),
        _ => tracing::debug!(method, %agent, "bridge: request"),
    }
    match method {
        "initialize" => json!({
            "jsonrpc": "2.0", "id": id,
            "result": {
                "protocolVersion": req["params"].get("protocolVersion")
                    .and_then(Value::as_str)
                    .filter(|version| SUPPORTED_PROTOCOL_VERSIONS.contains(version))
                    .unwrap_or(PROTOCOL_VERSION),
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "aether", "version": env!("CARGO_PKG_VERSION") }
            }
        }),
        "ping" => json!({ "jsonrpc": "2.0", "id": id, "result": {} }),
        "tools/list" => match async {
            let client = Client::connect(socket).await?;
            tool_list(&client.without_request_timeout(), agent, token).await
        }
        .await
        {
            Ok(tools) => json!({ "jsonrpc": "2.0", "id": id, "result": { "tools": tools } }),
            Err(e) => {
                json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32603, "message": e.to_string() } })
            }
        },
        "tools/call" => {
            let name = req["params"]
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            let input = req["params"].get("arguments").cloned().unwrap_or(json!({}));
            match async {
                // Approval and native wait replies deliberately have no deadline.
                let client = Client::connect(socket).await?.without_request_timeout();
                client
                    .request(Command::EngineTool {
                        agent,
                        token: token.to_owned(),
                        name,
                        input,
                    })
                    .await
            }
            .await
            {
                Ok(Reply::EngineTool { content, is_error }) => json!({
                    "jsonrpc": "2.0", "id": id,
                    "result": { "content": [{ "type": "text", "text": content }], "isError": is_error }
                }),
                Ok(other) => {
                    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32603, "message": format!("unexpected reply {other:?}") } })
                }
                Err(e) => {
                    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32603, "message": e.to_string() } })
                }
            }
        }
        other => {
            json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": format!("unknown method {other}") } })
        }
    }
}

#[cfg(test)]
mod tests;
