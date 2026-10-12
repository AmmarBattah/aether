//! OpenAI Chat Completions chunks → normalized stream events.

use std::collections::BTreeMap;

use aether_core::model::{BlockStart, ModelError, ModelErrorClass, StopReason, StreamEvent, Usage};
use serde_json::Value;

use crate::adapters::streaming::EventMapper;
use crate::http::{classify_openai_error, interrupted_stream, safe_provider_text};
use crate::sse::SseEvent;

#[derive(Debug, Default)]
pub struct OpenAiChatMapper {
    started: bool,
    next_index: usize,
    text_block: Option<usize>,
    reasoning_block: Option<usize>,
    /// Provider tool-call index → our block index.
    tool_blocks: BTreeMap<u64, usize>,
    open: Vec<usize>,
    usage: Usage,
    finish: Option<StopReason>,
    done: bool,
    /// A `finish_reason` or `[DONE]` arrived, so the body ending is the end of
    /// the answer rather than a cut connection. Either counts: some servers
    /// omit the `[DONE]` sentinel, and a finish reason is the model's own
    /// word that it stopped.
    terminal: bool,
}

impl OpenAiChatMapper {
    pub fn new() -> Self {
        Self::default()
    }

    fn open_block(&mut self, block: BlockStart, out: &mut Vec<StreamEvent>) -> usize {
        let index = self.next_index;
        self.next_index += 1;
        self.open.push(index);
        out.push(StreamEvent::BlockStart { index, block });
        index
    }

    fn close_all(&mut self, out: &mut Vec<StreamEvent>) {
        for index in std::mem::take(&mut self.open) {
            out.push(StreamEvent::BlockStop { index });
        }
        self.text_block = None;
        self.reasoning_block = None;
    }

    fn end_message(&mut self, out: &mut Vec<StreamEvent>) {
        if self.done {
            return;
        }
        self.done = true;
        let had_tools = !self.tool_blocks.is_empty();
        self.close_all(out);
        let reason = self.finish.take().unwrap_or(if had_tools {
            StopReason::ToolUse
        } else {
            StopReason::EndTurn
        });
        out.push(StreamEvent::Stop { reason });
        out.push(StreamEvent::MessageStop);
    }

    fn absorb_usage(&mut self, usage: &Value) -> bool {
        let get = |v: &Value, k: &str| v.get(k).and_then(Value::as_u64);
        let mut changed = false;
        if let Some(v) = get(usage, "prompt_tokens") {
            let cached = usage
                .get("prompt_tokens_details")
                .and_then(|d| get(d, "cached_tokens"))
                .unwrap_or(0);
            self.usage.input_tokens = v.saturating_sub(cached);
            self.usage.cache_read_tokens = cached;
            changed = true;
        }
        if let Some(v) = get(usage, "completion_tokens") {
            self.usage.output_tokens = v;
            changed = true;
        }
        if let Some(r) = usage
            .get("completion_tokens_details")
            .and_then(|d| get(d, "reasoning_tokens"))
        {
            self.usage.reasoning_tokens = Some(r);
            changed = true;
        }
        changed
    }

    fn handle_delta(&mut self, delta: &Value, out: &mut Vec<StreamEvent>) {
        let reasoning = delta
            .get("reasoning_content")
            .or_else(|| delta.get("reasoning"))
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty());
        if let Some(text) = reasoning {
            let index = match self.reasoning_block {
                Some(i) => i,
                None => {
                    let i = self.open_block(
                        BlockStart::Thinking {
                            text: String::new(),
                            redacted: false,
                            signature: None,
                        },
                        out,
                    );
                    self.reasoning_block = Some(i);
                    i
                }
            };
            out.push(StreamEvent::ThinkingDelta {
                index,
                text: text.to_owned(),
            });
        }

        if let Some(text) = delta
            .get("content")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            let index = match self.text_block {
                Some(i) => i,
                None => {
                    let i = self.open_block(
                        BlockStart::Text {
                            text: String::new(),
                        },
                        out,
                    );
                    self.text_block = Some(i);
                    i
                }
            };
            out.push(StreamEvent::TextDelta {
                index,
                text: text.to_owned(),
            });
        }

        if let Some(calls) = delta.get("tool_calls").and_then(Value::as_array) {
            for (position, call) in calls.iter().enumerate() {
                let tool_index = call
                    .get("index")
                    .and_then(Value::as_u64)
                    .unwrap_or(position as u64);
                let function = call.get("function").cloned().unwrap_or(Value::Null);
                let index = match self.tool_blocks.get(&tool_index) {
                    Some(i) => *i,
                    None => {
                        let id = call
                            .get("id")
                            .and_then(Value::as_str)
                            .filter(|s| !s.is_empty())
                            .map(str::to_owned)
                            .unwrap_or_else(|| format!("call_{tool_index}"));
                        let name = function
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_owned();
                        let i = self.open_block(
                            BlockStart::ToolUse {
                                id,
                                name,
                                input: None,
                            },
                            out,
                        );
                        self.tool_blocks.insert(tool_index, i);
                        i
                    }
                };
                if let Some(args) = function.get("arguments") {
                    let partial = match args {
                        Value::String(s) => s.clone(),
                        // Some servers send parsed objects; re-serialize so the
                        // accumulator sees one JSON document.
                        other if !other.is_null() => other.to_string(),
                        _ => String::new(),
                    };
                    if !partial.is_empty() {
                        out.push(StreamEvent::ToolInputDelta {
                            index,
                            partial_json: partial,
                        });
                    }
                }
            }
        }
    }
}

impl EventMapper for OpenAiChatMapper {
    fn map(&mut self, event: &SseEvent) -> Result<Vec<StreamEvent>, ModelError> {
        let mut out = Vec::new();
        if event.data.trim() == "[DONE]" {
            self.terminal = true;
            self.end_message(&mut out);
            return Ok(out);
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
        if let Some(error) = data.get("error") {
            return Err(stream_error(error));
        }
        if !self.started {
            self.started = true;
            out.push(StreamEvent::MessageStart {
                provider_message_id: data.get("id").and_then(Value::as_str).map(str::to_owned),
                model: data.get("model").and_then(Value::as_str).map(str::to_owned),
            });
        }
        if let Some(choices) = data.get("choices").and_then(Value::as_array) {
            for choice in choices {
                if let Some(delta) = choice.get("delta") {
                    self.handle_delta(delta, &mut out);
                }
                if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str) {
                    self.finish = Some(finish_reason(reason));
                    self.terminal = true;
                }
            }
        }
        if let Some(usage) = data.get("usage").filter(|u| !u.is_null())
            && self.absorb_usage(usage)
        {
            out.push(StreamEvent::Usage { usage: self.usage });
        }
        Ok(out)
    }

    fn finish(&mut self) -> Result<Vec<StreamEvent>, ModelError> {
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

pub fn finish_reason(reason: &str) -> StopReason {
    match reason {
        "stop" => StopReason::EndTurn,
        "tool_calls" | "function_call" => StopReason::ToolUse,
        "length" => StopReason::MaxTokens,
        "content_filter" => StopReason::Refusal {
            category: Some("content_filter".into()),
        },
        other => StopReason::Other {
            value: other.to_owned(),
        },
    }
}

fn stream_error(error: &Value) -> ModelError {
    let raw = error
        .as_str()
        .map(str::to_owned)
        .or_else(|| {
            error
                .get("message")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| error.to_string());
    let message = safe_provider_text(&raw, 2000);
    let code = error
        .get("code")
        .and_then(|c| {
            c.as_str()
                .map(str::to_owned)
                .or_else(|| c.as_i64().map(|n| n.to_string()))
        })
        .map(|c| safe_provider_text(&c, 200));
    let kind = error.get("type").and_then(Value::as_str).unwrap_or("");
    let class = classify_openai_error(kind, &message, code.as_deref());
    let mut err = ModelError::new(class, message);
    if let Some(code) = code {
        err = err.with_provider_code(code);
    }
    err
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::streaming::mapped_stream;
    use crate::client::collect;
    use serde_json::json;

    fn chunk(v: Value) -> SseEvent {
        SseEvent {
            event: None,
            data: v.to_string(),
            id: None,
        }
    }

    fn done() -> SseEvent {
        SseEvent {
            event: None,
            data: "[DONE]".into(),
            id: None,
        }
    }

    #[tokio::test]
    async fn tool_call_deltas_assemble() {
        let events = vec![
            chunk(
                json!({"id":"chatcmpl-1","model":"gpt-5","choices":[{"index":0,"delta":{"role":"assistant","content":""},"finish_reason":null}]}),
            ),
            chunk(
                json!({"id":"chatcmpl-1","choices":[{"index":0,"delta":{"content":"Let me "},"finish_reason":null}]}),
            ),
            chunk(
                json!({"id":"chatcmpl-1","choices":[{"index":0,"delta":{"content":"look."},"finish_reason":null}]}),
            ),
            chunk(
                json!({"id":"chatcmpl-1","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_abc","type":"function","function":{"name":"get_weather","arguments":""}}]},"finish_reason":null}]}),
            ),
            chunk(
                json!({"id":"chatcmpl-1","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"loc"}}]},"finish_reason":null}]}),
            ),
            chunk(
                json!({"id":"chatcmpl-1","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"ation\":\"Paris\"}"}}]},"finish_reason":null}]}),
            ),
            chunk(
                json!({"id":"chatcmpl-1","choices":[{"index":0,"delta":{"tool_calls":[{"index":1,"id":"call_def","type":"function","function":{"name":"get_time","arguments":"{}"}}]},"finish_reason":null}]}),
            ),
            chunk(
                json!({"id":"chatcmpl-1","choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}),
            ),
            chunk(
                json!({"id":"chatcmpl-1","choices":[],"usage":{"prompt_tokens":30,"completion_tokens":12,"prompt_tokens_details":{"cached_tokens":10},"completion_tokens_details":{"reasoning_tokens":4}}}),
            ),
            done(),
        ];
        let resp = collect(mapped_stream(events, OpenAiChatMapper::new()), |_| {})
            .await
            .expect("collect");
        assert_eq!(resp.provider_message_id.as_deref(), Some("chatcmpl-1"));
        assert_eq!(resp.text(), "Let me look.");
        assert_eq!(resp.stop_reason, Some(StopReason::ToolUse));
        let calls: Vec<_> = resp.tool_uses().collect();
        assert_eq!(
            calls,
            vec![
                ("call_abc", "get_weather", &json!({"location": "Paris"})),
                ("call_def", "get_time", &json!({}))
            ]
        );
        assert_eq!(resp.usage.input_tokens, 20);
        assert_eq!(resp.usage.cache_read_tokens, 10);
        assert_eq!(resp.usage.output_tokens, 12);
        assert_eq!(resp.usage.reasoning_tokens, Some(4));
    }

    #[tokio::test]
    async fn reasoning_content_becomes_thinking_and_missing_done_still_closes() {
        let events = vec![
            chunk(
                json!({"id":"x","choices":[{"delta":{"reasoning_content":"think"},"finish_reason":null}]}),
            ),
            chunk(
                json!({"id":"x","choices":[{"delta":{"content":"answer"},"finish_reason":"stop"}]}),
            ),
        ];
        let resp = collect(mapped_stream(events, OpenAiChatMapper::new()), |_| {})
            .await
            .expect("collect");
        assert_eq!(resp.stop_reason, Some(StopReason::EndTurn));
        assert!(
            matches!(&resp.content[0], aether_core::model::ContentBlock::Thinking { text, .. } if text == "think")
        );
        assert_eq!(resp.text(), "answer");
    }

    #[tokio::test]
    async fn error_chunk_maps() {
        let events = vec![chunk(
            json!({"error":{"message":"Rate limit reached","type":"rate_limit_error","code":"rate_limit_exceeded"}}),
        )];
        let err = collect(mapped_stream(events, OpenAiChatMapper::new()), |_| {})
            .await
            .unwrap_err();
        assert_eq!(err.class, ModelErrorClass::RateLimit);
        let events = vec![chunk(
            json!({"error":{"message":"This model's maximum context length is 8192 tokens","code":"context_length_exceeded"}}),
        )];
        let err = collect(mapped_stream(events, OpenAiChatMapper::new()), |_| {})
            .await
            .unwrap_err();
        assert_eq!(err.class, ModelErrorClass::ContextLimit);
    }

    #[tokio::test]
    async fn content_filter_is_a_refusal() {
        let events = vec![
            chunk(
                json!({"id":"x","choices":[{"delta":{"content":"no"},"finish_reason":"content_filter"}]}),
            ),
            done(),
        ];
        let resp = collect(mapped_stream(events, OpenAiChatMapper::new()), |_| {})
            .await
            .expect("collect");
        assert_eq!(
            resp.stop_reason,
            Some(StopReason::Refusal {
                category: Some("content_filter".into())
            })
        );
    }
}
