use super::*;
use crate::adapters::streaming::mapped_stream;
use crate::client::collect;
use serde_json::json;

fn ev(name: &str, v: Value) -> SseEvent {
    SseEvent {
        event: Some(name.to_owned()),
        data: v.to_string(),
        id: None,
    }
}

#[tokio::test]
async fn text_reasoning_and_usage_assemble() {
    let events = vec![
        ev(
            "response.created",
            json!({"type":"response.created","response":{"id":"resp_1","model":"gpt-5"}}),
        ),
        ev(
            "response.output_item.added",
            json!({"type":"response.output_item.added","output_index":0,"item":{"type":"reasoning","id":"rs_1"}}),
        ),
        ev(
            "response.reasoning_summary_text.delta",
            json!({"type":"response.reasoning_summary_text.delta","output_index":0,"delta":"weighing it"}),
        ),
        ev(
            "response.output_item.done",
            json!({"type":"response.output_item.done","output_index":0}),
        ),
        ev(
            "response.content_part.added",
            json!({"type":"response.content_part.added","output_index":1,"content_index":0,"part":{"type":"output_text"}}),
        ),
        ev(
            "response.output_text.delta",
            json!({"type":"response.output_text.delta","output_index":1,"content_index":0,"delta":"Hello, "}),
        ),
        ev(
            "response.output_text.delta",
            json!({"type":"response.output_text.delta","output_index":1,"content_index":0,"delta":"world."}),
        ),
        ev(
            "response.completed",
            json!({"type":"response.completed","response":{"id":"resp_1","status":"completed","usage":{"input_tokens":30,"output_tokens":12,"input_tokens_details":{"cached_tokens":10},"output_tokens_details":{"reasoning_tokens":4}}}}),
        ),
    ];
    let resp = collect(mapped_stream(events, OpenAiResponsesMapper::new()), |_| {})
        .await
        .expect("collect");
    assert_eq!(resp.provider_message_id.as_deref(), Some("resp_1"));
    assert_eq!(resp.text(), "Hello, world.");
    assert_eq!(resp.stop_reason, Some(StopReason::EndTurn));
    assert_eq!(resp.usage.input_tokens, 20);
    assert_eq!(resp.usage.cache_read_tokens, 10);
    assert_eq!(resp.usage.output_tokens, 12);
    assert_eq!(resp.usage.reasoning_tokens, Some(4));
    assert!(
        matches!(&resp.content[0], aether_core::model::ContentBlock::Thinking { text, .. } if text == "weighing it")
    );
}

#[tokio::test]
async fn tool_call_arguments_assemble() {
    let events = vec![
        ev(
            "response.created",
            json!({"type":"response.created","response":{"id":"resp_2","model":"gpt-5"}}),
        ),
        ev(
            "response.output_item.added",
            json!({"type":"response.output_item.added","output_index":0,"item":{"type":"function_call","id":"fc_1","call_id":"call_abc","name":"get_weather"}}),
        ),
        ev(
            "response.function_call_arguments.delta",
            json!({"type":"response.function_call_arguments.delta","output_index":0,"delta":"{\"loc"}),
        ),
        ev(
            "response.function_call_arguments.delta",
            json!({"type":"response.function_call_arguments.delta","output_index":0,"delta":"ation\":\"Paris\"}"}),
        ),
        ev(
            "response.output_item.done",
            json!({"type":"response.output_item.done","output_index":0}),
        ),
        ev(
            "response.completed",
            json!({"type":"response.completed","response":{"id":"resp_2","status":"completed","usage":{"input_tokens":5,"output_tokens":9}}}),
        ),
    ];
    let resp = collect(mapped_stream(events, OpenAiResponsesMapper::new()), |_| {})
        .await
        .expect("collect");
    assert_eq!(
        resp.tool_uses().collect::<Vec<_>>(),
        vec![("call_abc", "get_weather", &json!({"location": "Paris"}))]
    );
    assert_eq!(resp.stop_reason, Some(StopReason::ToolUse));
}

#[tokio::test]
async fn a_refusal_part_stops_the_turn_as_a_refusal() {
    let events = vec![
        ev(
            "response.created",
            json!({"type":"response.created","response":{"id":"r","model":"gpt-5"}}),
        ),
        ev(
            "response.content_part.added",
            json!({"type":"response.content_part.added","output_index":0,"content_index":0,"part":{"type":"refusal"}}),
        ),
        ev(
            "response.refusal.delta",
            json!({"type":"response.refusal.delta","output_index":0,"content_index":0,"delta":"I can't help with that."}),
        ),
        ev(
            "response.completed",
            json!({"type":"response.completed","response":{"id":"r","status":"completed"}}),
        ),
    ];
    let resp = collect(mapped_stream(events, OpenAiResponsesMapper::new()), |_| {})
        .await
        .expect("collect");
    assert_eq!(
        resp.stop_reason,
        Some(StopReason::Refusal {
            category: Some("refusal".into())
        })
    );
    assert_eq!(resp.text(), "I can't help with that.");
}

#[tokio::test]
async fn incomplete_and_failed_map_to_reason_and_error() {
    let events = vec![
        ev(
            "response.created",
            json!({"type":"response.created","response":{"id":"r","model":"gpt-5"}}),
        ),
        ev(
            "response.incomplete",
            json!({"type":"response.incomplete","response":{"status":"incomplete","incomplete_details":{"reason":"max_output_tokens"}}}),
        ),
    ];
    let resp = collect(mapped_stream(events, OpenAiResponsesMapper::new()), |_| {})
        .await
        .expect("collect");
    assert_eq!(resp.stop_reason, Some(StopReason::MaxTokens));

    let events = vec![ev(
        "response.failed",
        json!({"type":"response.failed","response":{"error":{"code":"rate_limit_exceeded","message":"Rate limit reached"}}}),
    )];
    let err = collect(mapped_stream(events, OpenAiResponsesMapper::new()), |_| {})
        .await
        .unwrap_err();
    assert_eq!(err.class, ModelErrorClass::RateLimit);
    assert_eq!(err.provider_code.as_deref(), Some("rate_limit_exceeded"));
}

/// The shape the Responses API actually sends for an internal fault: a
/// `type`, no `code`, and a message that matches no substring rule. It
/// must still be retryable, or the turn neither retries nor falls back.
#[tokio::test]
async fn an_untyped_server_failure_is_retryable_and_keeps_its_usage() {
    let events = vec![
        ev(
            "response.created",
            json!({"type":"response.created","response":{"id":"r","model":"gpt-5"}}),
        ),
        ev(
            "response.output_text.delta",
            json!({"type":"response.output_text.delta","output_index":0,"content_index":0,"delta":"Half an ans"}),
        ),
        ev(
            "response.failed",
            json!({"type":"response.failed","response":{"status":"failed","usage":{"input_tokens":30,"output_tokens":12,"input_tokens_details":{"cached_tokens":10}},"error":{"type":"server_error","message":"An internal error occurred"}}}),
        ),
    ];
    let failure =
        crate::client::collect_partial(mapped_stream(events, OpenAiResponsesMapper::new()), |_| {})
            .await
            .expect_err("a failed response is not an answer");
    assert_eq!(failure.error.class, ModelErrorClass::Server);
    assert!(failure.error.is_retryable());
    // `param` is not a code, and this error has none.
    assert_eq!(failure.error.provider_code, None);
    // The tokens the failed turn burned are still accounted for.
    assert_eq!(failure.partial.usage.input_tokens, 20);
    assert_eq!(failure.partial.usage.cache_read_tokens, 10);
    assert_eq!(failure.partial.usage.output_tokens, 12);
    assert_eq!(failure.partial.text(), "Half an ans");
}

/// A stream cut at an event boundary reached no `response.completed`, so
/// the mapper has positive evidence the answer never finished. Reporting
/// EndTurn there would keep a truncated answer as a finished turn and skip
/// both the retry and the fallback chain.
#[tokio::test]
async fn a_stream_cut_at_an_event_boundary_is_interrupted_not_a_finished_turn() {
    let events = vec![
        ev(
            "response.created",
            json!({"type":"response.created","response":{"id":"r","model":"gpt-5"}}),
        ),
        ev(
            "response.output_text.delta",
            json!({"type":"response.output_text.delta","output_index":0,"content_index":0,"delta":"half"}),
        ),
    ];
    let failure =
        crate::client::collect_partial(mapped_stream(events, OpenAiResponsesMapper::new()), |_| {})
            .await
            .expect_err("a cut stream is not a finished turn");
    assert_eq!(failure.error.class, ModelErrorClass::Network);
    assert!(failure.error.is_retryable());
    // What did arrive is still kept, so the retry has the context.
    assert_eq!(failure.partial.text(), "half");
    assert_eq!(failure.partial.stop_reason, None);
}
