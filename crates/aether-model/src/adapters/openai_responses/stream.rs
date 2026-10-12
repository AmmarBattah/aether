//! OpenAI Responses SSE events → normalized stream events.

use std::collections::BTreeMap;

use aether_core::model::{BlockStart, ModelError, ModelErrorClass, StopReason, StreamEvent, Usage};
use serde_json::Value;

use crate::adapters::streaming::EventMapper;
use crate::http::{classify_openai_error, interrupted_stream, safe_provider_text};
use crate::sse::SseEvent;

/// Where a block sits in the response: which output item, and which content
/// part within it. Unique across items, so one map covers text, refusals,
/// reasoning and tool calls.
type BlockKey = (u64, u64);

#[derive(Debug, Default)]
pub struct OpenAiResponsesMapper {
    started: bool,
    next_index: usize,
    blocks: BTreeMap<BlockKey, usize>,
    open: Vec<usize>,
    usage: Usage,
    stop: Option<StopReason>,
    saw_tool_call: bool,
    refused: Option<String>,
    done: bool,
    /// A `response.failed` error held back one step so the usage it reported
    /// reaches the accumulator first. Surfaced by the next event, or by EOF.
    deferred_error: Option<ModelError>,
    /// A terminal `response.*` event arrived, so the body ending is the end of
    /// the answer rather than a cut connection.
    terminal: bool,
}

impl OpenAiResponsesMapper {
    pub fn new() -> Self {
        Self::default()
    }

    fn block(&mut self, key: BlockKey, block: BlockStart, out: &mut Vec<StreamEvent>) -> usize {
        if let Some(index) = self.blocks.get(&key) {
            return *index;
        }
        let index = self.next_index;
        self.next_index += 1;
        self.blocks.insert(key, index);
        self.open.push(index);
        out.push(StreamEvent::BlockStart { index, block });
        index
    }

    fn close(&mut self, key: BlockKey, out: &mut Vec<StreamEvent>) {
        let Some(index) = self.blocks.get(&key).copied() else {
            return;
        };
        if let Some(at) = self.open.iter().position(|i| *i == index) {
            self.open.remove(at);
            out.push(StreamEvent::BlockStop { index });
        }
    }

    fn end_message(&mut self, out: &mut Vec<StreamEvent>) {
        if self.done {
            return;
        }
        self.done = true;
        for index in std::mem::take(&mut self.open) {
            out.push(StreamEvent::BlockStop { index });
        }
        let reason = self.stop.take().unwrap_or_else(|| {
            if let Some(category) = self.refused.take() {
                StopReason::Refusal {
                    category: Some(category),
                }
            } else if self.saw_tool_call {
                StopReason::ToolUse
            } else {
                StopReason::EndTurn
            }
        });
        out.push(StreamEvent::Stop { reason });
        out.push(StreamEvent::MessageStop);
    }

    fn absorb_usage(&mut self, usage: &Value) -> bool {
        let get = |v: &Value, k: &str| v.get(k).and_then(Value::as_u64);
        let mut changed = false;
        if let Some(v) = get(usage, "input_tokens") {
            let cached = usage
                .get("input_tokens_details")
                .and_then(|d| get(d, "cached_tokens"))
                .unwrap_or(0);
            self.usage.input_tokens = v.saturating_sub(cached);
            self.usage.cache_read_tokens = cached;
            changed = true;
        }
        if let Some(v) = get(usage, "output_tokens") {
            self.usage.output_tokens = v;
            changed = true;
        }
        if let Some(r) = usage
            .get("output_tokens_details")
            .and_then(|d| get(d, "reasoning_tokens"))
        {
            self.usage.reasoning_tokens = Some(r);
            changed = true;
        }
        changed
    }

    /// A terminal `response.*` event carrying the finished response object.
    fn finish_response(&mut self, response: &Value, out: &mut Vec<StreamEvent>) {
        if let Some(usage) = response.get("usage").filter(|u| !u.is_null())
            && self.absorb_usage(usage)
        {
            out.push(StreamEvent::Usage { usage: self.usage });
        }
        if let Some(details) = response
            .get("incomplete_details")
            .and_then(|d| d.get("reason"))
            .and_then(Value::as_str)
        {
            self.stop = Some(match details {
                "max_output_tokens" => StopReason::MaxTokens,
                "content_filter" => StopReason::Refusal {
                    category: Some("content_filter".into()),
                },
                other => StopReason::Other {
                    value: other.to_owned(),
                },
            });
        }
        self.end_message(out);
    }

    fn start_item(&mut self, data: &Value, out: &mut Vec<StreamEvent>) {
        let output_index = index_of(data, "output_index");
        let item = &data["item"];
        match item.get("type").and_then(Value::as_str).unwrap_or("") {
            "function_call" => {
                self.saw_tool_call = true;
                let id = item
                    .get("call_id")
                    .or_else(|| item.get("id"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned();
                let name = item
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned();
                self.block(
                    (output_index, 0),
                    BlockStart::ToolUse {
                        id,
                        name,
                        input: None,
                    },
                    out,
                );
            }
            "reasoning" => {
                self.block(
                    (output_index, 0),
                    BlockStart::Thinking {
                        text: String::new(),
                        redacted: false,
                        signature: None,
                    },
                    out,
                );
            }
            // A message item opens its blocks per content part instead, so the
            // part's own type decides between text and refusal.
            _ => {}
        }
    }
}

impl EventMapper for OpenAiResponsesMapper {
    fn map(&mut self, event: &SseEvent) -> Result<Vec<StreamEvent>, ModelError> {
        if let Some(err) = self.deferred_error.take() {
            return Err(err);
        }
        let data: Value = serde_json::from_str(&event.data).map_err(|e| {
            ModelError::new(
                ModelErrorClass::Unknown,
                format!(
                    "malformed SSE data: {e}: {}",
                    safe_provider_text(&event.data, 512)
                ),
            )
        })?;
        let kind = data
            .get("type")
            .and_then(Value::as_str)
            .or(event.event.as_deref())
            .unwrap_or("");
        let mut out = Vec::new();
        match kind {
            "response.created" | "response.in_progress" => {
                if !self.started {
                    self.started = true;
                    let response = &data["response"];
                    out.push(StreamEvent::MessageStart {
                        provider_message_id: response
                            .get("id")
                            .and_then(Value::as_str)
                            .map(str::to_owned),
                        model: response
                            .get("model")
                            .and_then(Value::as_str)
                            .map(str::to_owned),
                    });
                }
            }
            "response.output_item.added" => self.start_item(&data, &mut out),
            "response.output_item.done" => {
                let key = (index_of(&data, "output_index"), 0);
                self.close(key, &mut out);
            }
            "response.content_part.added" => {
                let key = (
                    index_of(&data, "output_index"),
                    index_of(&data, "content_index"),
                );
                // A refusal streams as text like any other part; the only
                // difference is that it decides the stop reason.
                if data["part"].get("type").and_then(Value::as_str) == Some("refusal") {
                    self.refused = Some("refusal".into());
                }
                self.block(
                    key,
                    BlockStart::Text {
                        text: String::new(),
                    },
                    &mut out,
                );
            }
            "response.content_part.done" => {
                let key = (
                    index_of(&data, "output_index"),
                    index_of(&data, "content_index"),
                );
                self.close(key, &mut out);
            }
            "response.output_text.delta" | "response.refusal.delta" => {
                if kind == "response.refusal.delta" {
                    self.refused.get_or_insert_with(|| "refusal".into());
                }
                let key = (
                    index_of(&data, "output_index"),
                    index_of(&data, "content_index"),
                );
                let index = self.block(
                    key,
                    BlockStart::Text {
                        text: String::new(),
                    },
                    &mut out,
                );
                if let Some(text) = delta_str(&data) {
                    out.push(StreamEvent::TextDelta { index, text });
                }
            }
            "response.reasoning_summary_text.delta" | "response.reasoning_text.delta" => {
                let index = self.block(
                    (index_of(&data, "output_index"), 0),
                    BlockStart::Thinking {
                        text: String::new(),
                        redacted: false,
                        signature: None,
                    },
                    &mut out,
                );
                if let Some(text) = delta_str(&data) {
                    out.push(StreamEvent::ThinkingDelta { index, text });
                }
            }
            "response.function_call_arguments.delta" => {
                self.saw_tool_call = true;
                let index = self.block(
                    (index_of(&data, "output_index"), 0),
                    BlockStart::ToolUse {
                        id: String::new(),
                        name: String::new(),
                        input: None,
                    },
                    &mut out,
                );
                if let Some(partial_json) = delta_str(&data) {
                    out.push(StreamEvent::ToolInputDelta {
                        index,
                        partial_json,
                    });
                }
            }
            "response.completed" | "response.incomplete" => {
                self.terminal = true;
                let response = data.get("response").cloned().unwrap_or(Value::Null);
                self.finish_response(&response, &mut out);
            }
            "response.failed" => {
                self.terminal = true;
                let response = &data["response"];
                // The bridge drains pending events before the deferred error,
                // so the tokens the failed turn burned are still accounted.
                if let Some(usage) = response.get("usage").filter(|u| !u.is_null())
                    && self.absorb_usage(usage)
                {
                    out.push(StreamEvent::Usage { usage: self.usage });
                }
                self.deferred_error = Some(stream_error(&response["error"]));
                return Ok(out);
            }
            "error" => return Err(stream_error(&data)),
            _ => {}
        }
        Ok(out)
    }

    fn finish(&mut self) -> Result<Vec<StreamEvent>, ModelError> {
        if let Some(err) = self.deferred_error.take() {
            return Err(err);
        }
        if self.started && !self.terminal {
            return Err(interrupted_stream());
        }
        let mut out = Vec::new();
        if self.started {
            self.end_message(&mut out);
        }
        Ok(out)
    }
}

fn index_of(data: &Value, field: &str) -> u64 {
    data.get(field).and_then(Value::as_u64).unwrap_or(0)
}

fn delta_str(data: &Value) -> Option<String> {
    data.get("delta")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

/// The Responses error object, shared by `error` events and `response.failed`.
/// `param` is deliberately not a fallback for `code`: it names the offending
/// request field, not an error code, and it surfaces to the operator as one.
fn stream_error(error: &Value) -> ModelError {
    let raw = error
        .get("message")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| error.to_string());
    let message = safe_provider_text(&raw, 2000);
    let code = error
        .get("code")
        .and_then(Value::as_str)
        .map(|c| safe_provider_text(c, 200));
    let kind = error.get("type").and_then(Value::as_str).unwrap_or("");
    let class = classify_openai_error(kind, &message, code.as_deref());
    let mut err = ModelError::new(class, message);
    if let Some(code) = code {
        err = err.with_provider_code(code);
    }
    err
}

#[cfg(test)]
mod tests;
