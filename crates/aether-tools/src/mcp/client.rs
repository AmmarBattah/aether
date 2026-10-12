//! The MCP protocol on top of a transport: handshake, tool discovery, tool
//! calls. Nothing here knows about agents, approvals or events.

use std::sync::Arc;

use serde_json::{Map, Value, json};

use super::transport::Transport;
use super::{MAX_DESCRIPTION_CHARS, MAX_SCHEMA_BYTES, MAX_TOOLS, McpError};

/// The preferred revision for the tool surface Aether speaks.
const PROTOCOL_VERSION: &str = "2025-11-25";
const SUPPORTED_PROTOCOL_VERSIONS: &[&str] =
    &["2024-11-05", "2025-03-26", "2025-06-18", "2025-11-25"];

/// A tool as the server describes it.
#[derive(Debug, Clone, PartialEq)]
pub struct RemoteTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    /// The server's own hints, when it sent any (`readOnlyHint` and friends).
    pub annotations: Value,
}

/// A server's tool list, plus what had to be cut to take it. `capped` is
/// `None` when the server stayed inside every limit; otherwise it says which
/// one it passed, and the manager records it as the server's health.
#[derive(Debug, Clone, PartialEq)]
pub struct Discovery {
    pub tools: Vec<RemoteTool>,
    pub capped: Option<String>,
}

/// What a tool call produced.
#[derive(Debug, Clone, PartialEq)]
pub struct McpCallResult {
    pub text: String,
    pub is_error: bool,
    pub structured: Option<Value>,
}

#[derive(Debug)]
pub struct McpClient {
    transport: Arc<dyn Transport>,
}

impl McpClient {
    pub(crate) fn new(transport: Arc<dyn Transport>) -> Self {
        Self { transport }
    }

    /// Handshake. The `initialized` notification is required by the spec
    /// before any other request.
    pub(crate) async fn initialize(&self) -> Result<(), McpError> {
        let result = self
            .transport
            .request(
                "initialize",
                json!({
                    "protocolVersion": PROTOCOL_VERSION,
                    "capabilities": {},
                    "clientInfo": { "name": "aether", "version": env!("CARGO_PKG_VERSION") }
                }),
            )
            .await?;
        let version = result
            .get("protocolVersion")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                McpError::Protocol("initialize did not return a protocol version".into())
            })?;
        if !SUPPORTED_PROTOCOL_VERSIONS.contains(&version) {
            return Err(McpError::Protocol(format!(
                "unsupported protocol version `{version}`"
            )));
        }
        self.transport.set_protocol_version(version)?;
        self.transport
            .notify("notifications/initialized", json!({}))
            .await
    }

    pub(crate) async fn list_tools(&self) -> Result<Discovery, McpError> {
        let mut tools = Vec::new();
        let mut cursor: Option<String> = None;
        // Paginated by the spec; bounded so a server that always returns the
        // same cursor cannot spin here forever.
        for _ in 0..64 {
            let params = match &cursor {
                Some(c) => json!({ "cursor": c }),
                None => json!({}),
            };
            let result = self.transport.request("tools/list", params).await?;
            let page = result
                .get("tools")
                .and_then(Value::as_array)
                .ok_or_else(|| McpError::Protocol("tools/list did not return tools".into()))?;
            for tool in page {
                let Some(name) = tool.get("name").and_then(Value::as_str) else {
                    return Err(McpError::Protocol("a tool has no name".into()));
                };
                if tools.len() == MAX_TOOLS {
                    // Every tool here goes into every turn's tool list, so a
                    // server offering thousands would fill the model context.
                    return Ok(Discovery {
                        tools,
                        capped: Some(format!(
                            "the server offers more than {MAX_TOOLS} tools; only the first \
                             {MAX_TOOLS} are used"
                        )),
                    });
                }
                tools.push(remote_tool(name, tool));
            }
            cursor = result
                .get("nextCursor")
                .and_then(Value::as_str)
                .map(str::to_owned);
            if cursor.is_none() {
                return Ok(Discovery {
                    tools,
                    capped: None,
                });
            }
        }
        Err(McpError::Protocol(
            "tools/list kept paginating without ending".into(),
        ))
    }

    /// `meta` becomes `params._meta`. It is built by the caller apart from
    /// `arguments`, which are sent as they are and never merged into it.
    pub(crate) async fn call_tool(
        &self,
        name: &str,
        arguments: Value,
        meta: Option<Map<String, Value>>,
    ) -> Result<McpCallResult, McpError> {
        let mut params = json!({ "name": name, "arguments": arguments });
        if let Some(meta) = meta.filter(|meta| !meta.is_empty()) {
            params["_meta"] = Value::Object(meta);
        }
        let result = self.transport.request("tools/call", params).await?;
        if !result.is_object() || !result.get("content").is_some_and(Value::is_array) {
            return Err(McpError::Protocol(
                "tools/call must return an object with a content array".into(),
            ));
        }
        if result
            .get("isError")
            .is_some_and(|value| !value.is_boolean())
        {
            return Err(McpError::Protocol(
                "tools/call isError must be a boolean".into(),
            ));
        }
        Ok(McpCallResult {
            text: render_content(result.get("content")),
            is_error: result
                .get("isError")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            structured: result.get("structuredContent").cloned(),
        })
    }
}

/// One tool as described, with the description and schema bounded. Both are
/// the server's own text and both reach every turn's tool list, so the cap is
/// reported where the model reads it rather than applied silently.
fn remote_tool(name: &str, tool: &Value) -> RemoteTool {
    let described = tool
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let mut description = bounded_description(described);
    let schema = tool
        .get("inputSchema")
        .cloned()
        .unwrap_or_else(|| json!({"type": "object"}));
    let schema_bytes = serde_json::to_vec(&schema).map(|v| v.len()).unwrap_or(0);
    let input_schema = if schema_bytes > MAX_SCHEMA_BYTES {
        description.push_str(&format!(
            "\n[the server's input schema is {schema_bytes} bytes, over the \
             {MAX_SCHEMA_BYTES}-byte cap; it was replaced with an open object and arguments \
             are not checked against it]"
        ));
        json!({"type": "object"})
    } else {
        schema
    };
    RemoteTool {
        name: name.to_owned(),
        description,
        input_schema,
        annotations: tool.get("annotations").cloned().unwrap_or(Value::Null),
    }
}

/// The description, cut with the marker the built-in tools use for a bound.
fn bounded_description(text: &str) -> String {
    let total = text.chars().count();
    if total <= MAX_DESCRIPTION_CHARS {
        return text.to_owned();
    }
    let mut kept: String = text.chars().take(MAX_DESCRIPTION_CHARS).collect();
    kept.push_str(&format!(
        "\n[truncated: the description is {total} chars, over the \
         {MAX_DESCRIPTION_CHARS}-char cap]"
    ));
    kept
}

/// Flatten MCP content blocks into the text the model sees. Non-text blocks
/// are named rather than dropped, so a call that returned an image is not
/// silently reported as empty.
fn render_content(content: Option<&Value>) -> String {
    let Some(blocks) = content.and_then(Value::as_array) else {
        return String::new();
    };
    let mut parts = Vec::with_capacity(blocks.len());
    for block in blocks {
        let kind = block.get("type").and_then(Value::as_str).unwrap_or("");
        match kind {
            "text" => parts.push(
                block
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            ),
            "resource" => parts.push(format!(
                "[resource {}]",
                block
                    .get("resource")
                    .and_then(|r| r.get("uri"))
                    .and_then(Value::as_str)
                    .unwrap_or("?")
            )),
            other => parts.push(format!("[{other} content]")),
        }
    }
    parts.join("\n")
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;

    use super::*;

    #[derive(Debug)]
    struct Handshake(Value);

    #[async_trait]
    impl Transport for Handshake {
        async fn request(&self, method: &str, _params: Value) -> Result<Value, McpError> {
            assert_eq!(method, "initialize");
            Ok(self.0.clone())
        }

        async fn notify(&self, _method: &str, _params: Value) -> Result<(), McpError> {
            panic!("invalid negotiation must not send initialized");
        }
    }

    #[tokio::test]
    async fn initialization_rejects_missing_or_unsupported_versions() {
        for result in [
            json!({}),
            json!({"protocolVersion": 42}),
            json!({"protocolVersion": "2099-01-01"}),
        ] {
            let client = McpClient::new(Arc::new(Handshake(result)));
            assert!(matches!(
                client.initialize().await,
                Err(McpError::Protocol(_))
            ));
        }
    }

    /// A server that answers `tools/list` with `count` tools, each carrying
    /// `description` and `schema`.
    #[derive(Debug)]
    struct Listing {
        count: usize,
        description: String,
        schema: Value,
    }

    #[async_trait]
    impl Transport for Listing {
        async fn request(&self, method: &str, _params: Value) -> Result<Value, McpError> {
            assert_eq!(method, "tools/list");
            let tools: Vec<Value> = (0..self.count)
                .map(|i| {
                    json!({
                        "name": format!("t{i}"),
                        "description": self.description,
                        "inputSchema": self.schema,
                    })
                })
                .collect();
            Ok(json!({ "tools": tools }))
        }

        async fn notify(&self, _method: &str, _params: Value) -> Result<(), McpError> {
            Ok(())
        }
    }

    fn listing(count: usize, description: String, schema: Value) -> McpClient {
        McpClient::new(Arc::new(Listing {
            count,
            description,
            schema,
        }))
    }

    #[tokio::test]
    async fn a_server_offering_too_many_tools_is_cut_and_says_so() {
        // Every tool here reaches every turn's tool list, so an unbounded
        // count is an unbounded context - and the cut must be visible.
        let client = listing(MAX_TOOLS + 10, "fine".into(), json!({"type": "object"}));
        let discovered = client.list_tools().await.expect("tools");
        assert_eq!(discovered.tools.len(), MAX_TOOLS);
        let capped = discovered.capped.expect("a cut tool list says so");
        assert!(capped.contains(&MAX_TOOLS.to_string()), "{capped}");
    }

    #[tokio::test]
    async fn an_oversized_description_and_schema_are_bounded_where_the_model_reads_them() {
        let long = "d".repeat(MAX_DESCRIPTION_CHARS + 50);
        let fat = json!({
            "type": "object",
            "properties": { "p": { "description": "s".repeat(MAX_SCHEMA_BYTES) } }
        });
        let client = listing(1, long, fat);
        let discovered = client.list_tools().await.expect("tools");
        assert_eq!(discovered.capped, None, "one tool is not a count problem");
        let tool = &discovered.tools[0];
        assert!(
            tool.description.contains("truncated"),
            "a cut description says so: {}",
            &tool.description[..80.min(tool.description.len())]
        );
        assert!(tool.description.chars().count() < MAX_DESCRIPTION_CHARS + 300);
        assert_eq!(
            tool.input_schema,
            json!({"type": "object"}),
            "an oversized schema is replaced, not carried"
        );
        assert!(
            tool.description.contains("input schema"),
            "and the replacement is stated"
        );
    }

    #[tokio::test]
    async fn a_description_and_schema_inside_the_caps_are_untouched() {
        let schema = json!({"type": "object", "properties": {"key": {"type": "string"}}});
        let client = listing(1, "read the note".into(), schema.clone());
        let discovered = client.list_tools().await.expect("tools");
        assert_eq!(discovered.tools[0].description, "read the note");
        assert_eq!(discovered.tools[0].input_schema, schema);
    }

    /// A server that keeps the params of every `tools/call` it is sent.
    #[derive(Debug, Default)]
    struct Recording {
        params: std::sync::Mutex<Vec<Value>>,
    }

    #[async_trait]
    impl Transport for Recording {
        async fn request(&self, method: &str, params: Value) -> Result<Value, McpError> {
            assert_eq!(method, "tools/call");
            self.params.lock().expect("lock").push(params);
            Ok(json!({ "content": [] }))
        }

        async fn notify(&self, _method: &str, _params: Value) -> Result<(), McpError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn meta_is_sent_beside_the_arguments_and_only_when_there_is_some() {
        let server = Arc::new(Recording::default());
        let client = McpClient::new(server.clone());
        let arguments = json!({ "_meta": { "aether/agent_id": "from the model" } });
        let meta = Map::from_iter([("aether/agent_id".to_owned(), json!("agt_real"))]);

        client
            .call_tool("t", arguments.clone(), Some(meta))
            .await
            .expect("call");
        client
            .call_tool("t", arguments.clone(), Some(Map::new()))
            .await
            .expect("call");
        client
            .call_tool("t", arguments.clone(), None)
            .await
            .expect("call");

        let sent = server.params.lock().expect("lock").clone();
        assert_eq!(
            sent[0],
            json!({
                "name": "t",
                "arguments": arguments,
                "_meta": { "aether/agent_id": "agt_real" }
            }),
            "the arguments are never merged into `_meta`"
        );
        let bare = json!({ "name": "t", "arguments": arguments });
        assert_eq!(sent[1], bare, "an empty `_meta` is not sent");
        assert_eq!(sent[2], bare);
    }

    #[test]
    fn content_blocks_render_and_non_text_is_named_not_dropped() {
        let content = json!([
            {"type": "text", "text": "one"},
            {"type": "image", "data": "…"},
            {"type": "resource", "resource": {"uri": "file:///x"}}
        ]);
        assert_eq!(
            render_content(Some(&content)),
            "one\n[image content]\n[resource file:///x]"
        );
        assert_eq!(render_content(None), "");
    }
}

#[cfg(test)]
mod call_result_tests {
    use super::*;
    use async_trait::async_trait;
    #[derive(Debug)]
    struct CallReply(Value);
    #[async_trait]
    impl Transport for CallReply {
        async fn request(&self, method: &str, _params: Value) -> Result<Value, McpError> {
            assert_eq!(method, "tools/call");
            Ok(self.0.clone())
        }
        async fn notify(&self, _method: &str, _params: Value) -> Result<(), McpError> {
            Ok(())
        }
    }
    #[tokio::test]
    async fn malformed_results_are_protocol_errors_instead_of_empty_successes() {
        for result in [
            Value::Null,
            json!("text"),
            json!({}),
            json!({"content": null}),
            json!({"content": [], "isError": "false"}),
            json!({"content": [], "isError": null}),
        ] {
            let client = McpClient::new(Arc::new(CallReply(result)));
            assert!(matches!(
                client.call_tool("test", json!({}), None).await,
                Err(McpError::Protocol(_))
            ));
        }
        for result in [
            json!({"content": []}),
            json!({"content": [], "isError": false}),
            json!({"content": [], "isError": true}),
        ] {
            let error = result
                .get("isError")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let client = McpClient::new(Arc::new(CallReply(result)));
            assert_eq!(
                client
                    .call_tool("test", json!({}), None)
                    .await
                    .unwrap()
                    .is_error,
                error
            );
        }
    }
}
