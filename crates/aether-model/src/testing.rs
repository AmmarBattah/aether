//! A scripted model for deterministic runtime tests: canned responses in
//! order, recorded requests, optional delay, honors cancellation.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use aether_core::model::{
    BlockStart, ModelError, ModelErrorClass, ModelRequest, StopReason, StreamEvent, Usage,
};
use futures::StreamExt;
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use crate::client::{ModelClient, ModelStream};

pub type ScriptedResponse = Result<Vec<StreamEvent>, ModelError>;

#[derive(Debug, Default)]
pub struct ScriptedClient {
    responses: Mutex<VecDeque<ScriptedResponse>>,
    by_provider: Mutex<HashMap<String, VecDeque<ScriptedResponse>>>,
    requests: Mutex<Vec<ModelRequest>>,
    delay: Mutex<Duration>,
    /// How many `stream()` calls have been served, and a gate armed for one
    /// specific future call by its 1-based position in that count. One lock
    /// so arming, matching, and the response pop in `stream()` stay atomic
    /// across concurrent callers - never three separately-locked steps that
    /// a race could pull apart.
    gate_state: Mutex<GateState>,
    in_flight: Arc<Mutex<InFlight>>,
}

#[derive(Debug, Default)]
struct GateState {
    call_count: usize,
    pending: Option<(usize, CancellationToken)>,
}

/// Streams open per provider, and the most that were ever open at once, so a
/// test can watch a concurrency cap hold instead of inferring it from timing.
#[derive(Debug, Default)]
struct InFlight {
    now: HashMap<String, usize>,
    peak: HashMap<String, usize>,
}

/// Holds one provider's slot for as long as the stream lives; a dropped or
/// cancelled stream releases it just like a finished one.
struct InFlightSlot {
    counts: Arc<Mutex<InFlight>>,
    provider: String,
}

impl InFlightSlot {
    fn take(counts: &Arc<Mutex<InFlight>>, provider: &str) -> Self {
        {
            let mut c = counts.lock().unwrap_or_else(|p| p.into_inner());
            let now = c.now.entry(provider.to_owned()).or_default();
            *now += 1;
            let now = *now;
            let peak = c.peak.entry(provider.to_owned()).or_default();
            *peak = (*peak).max(now);
        }
        Self {
            counts: counts.clone(),
            provider: provider.to_owned(),
        }
    }
}

impl Drop for InFlightSlot {
    fn drop(&mut self) {
        let mut c = self.counts.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(now) = c.now.get_mut(&self.provider) {
            *now = now.saturating_sub(1);
        }
    }
}

impl ScriptedClient {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_responses(responses: impl IntoIterator<Item = ScriptedResponse>) -> Self {
        let client = Self::new();
        client.push_all(responses);
        client
    }

    pub fn push(&self, response: ScriptedResponse) {
        self.responses
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push_back(response);
    }

    /// Queue a response only this provider may take. Popped before the shared
    /// queue, so a chain test does not depend on which link calls first.
    pub fn push_for(&self, provider: &str, response: ScriptedResponse) {
        self.by_provider
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .entry(provider.to_owned())
            .or_default()
            .push_back(response);
    }

    pub fn push_all(&self, responses: impl IntoIterator<Item = ScriptedResponse>) {
        let mut q = self.responses.lock().unwrap_or_else(|p| p.into_inner());
        q.extend(responses);
    }

    /// Hold the next response (the very next `stream()` call, whichever
    /// provider it is for) until the returned token is cancelled. Later
    /// responses proceed normally, so concurrency tests can control ordering
    /// without depending on a long sequence of timer delays.
    pub fn hold_next_response(&self) -> CancellationToken {
        let release = CancellationToken::new();
        let mut state = self.gate_state.lock().unwrap_or_else(|p| p.into_inner());
        assert!(state.pending.is_none(), "a response gate is already armed");
        // Read and arm under the same lock: a concurrent stream() call
        // advancing call_count between the read and the arm would otherwise
        // target a call number that has already passed.
        state.pending = Some((state.call_count + 1, release.clone()));
        release
    }

    /// Hold the response to the Nth `stream()` call (1-indexed, counted
    /// across all providers) until the returned token is cancelled, or until
    /// that call observes its own cancellation token fire. Lets a test target
    /// one specific agent's turn deterministically by call order - e.g. a
    /// delegated child's first turn, which shares its parent's provider and
    /// so cannot be targeted by `push_for` alone.
    pub fn hold_response_at(&self, call_number: usize) -> CancellationToken {
        assert!(call_number >= 1, "call numbers are 1-indexed");
        let release = CancellationToken::new();
        let mut state = self.gate_state.lock().unwrap_or_else(|p| p.into_inner());
        assert!(state.pending.is_none(), "a response gate is already armed");
        state.pending = Some((call_number, release.clone()));
        release
    }

    /// Delay applied between each streamed event of subsequent responses.
    pub fn set_delay(&self, delay: Duration) {
        *self.delay.lock().unwrap_or_else(|p| p.into_inner()) = delay;
    }

    pub fn requests(&self) -> Vec<ModelRequest> {
        self.requests
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    /// The most streams this provider ever had open at the same time.
    pub fn peak_in_flight(&self, provider: &str) -> usize {
        self.in_flight
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .peak
            .get(provider)
            .copied()
            .unwrap_or(0)
    }

    pub fn remaining(&self) -> usize {
        self.responses
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .len()
    }

    /// A well-formed text-only response.
    pub fn text(text: &str) -> ScriptedResponse {
        Ok(vec![
            StreamEvent::MessageStart {
                provider_message_id: Some("scripted".into()),
                model: Some("scripted".into()),
            },
            StreamEvent::BlockStart {
                index: 0,
                block: BlockStart::Text {
                    text: String::new(),
                },
            },
            StreamEvent::TextDelta {
                index: 0,
                text: text.to_owned(),
            },
            StreamEvent::BlockStop { index: 0 },
            StreamEvent::Usage {
                usage: usage_for(text.len(), 0),
            },
            StreamEvent::Stop {
                reason: StopReason::EndTurn,
            },
            StreamEvent::MessageStop,
        ])
    }

    /// A response that calls one tool, with optional leading text.
    pub fn tool_call(name: &str, input: Value) -> ScriptedResponse {
        Self::tool_calls(&[(name, input)], None)
    }

    /// A response that calls several tools in one message.
    pub fn tool_calls(calls: &[(&str, Value)], text: Option<&str>) -> ScriptedResponse {
        let mut events = vec![StreamEvent::MessageStart {
            provider_message_id: Some("scripted".into()),
            model: Some("scripted".into()),
        }];
        let mut index = 0;
        if let Some(t) = text {
            events.push(StreamEvent::BlockStart {
                index,
                block: BlockStart::Text {
                    text: String::new(),
                },
            });
            events.push(StreamEvent::TextDelta {
                index,
                text: t.to_owned(),
            });
            events.push(StreamEvent::BlockStop { index });
            index += 1;
        }
        for (n, (name, input)) in calls.iter().enumerate() {
            events.push(StreamEvent::BlockStart {
                index,
                block: BlockStart::ToolUse {
                    id: format!("call_{n}"),
                    name: (*name).to_owned(),
                    input: None,
                },
            });
            events.push(StreamEvent::ToolInputDelta {
                index,
                partial_json: input.to_string(),
            });
            events.push(StreamEvent::BlockStop { index });
            index += 1;
        }
        events.push(StreamEvent::Usage {
            usage: usage_for(0, calls.len()),
        });
        events.push(StreamEvent::Stop {
            reason: StopReason::ToolUse,
        });
        events.push(StreamEvent::MessageStop);
        Ok(events)
    }

    pub fn error(class: ModelErrorClass, message: &str) -> ScriptedResponse {
        Err(ModelError::new(class, message))
    }
}

fn usage_for(text_len: usize, calls: usize) -> Usage {
    Usage {
        input_tokens: 10,
        output_tokens: (text_len / 4) as u64 + calls as u64 * 8 + 1,
        ..Default::default()
    }
}

#[async_trait::async_trait]
impl ModelClient for ScriptedClient {
    fn adapter(&self) -> &str {
        "scripted/1"
    }

    async fn stream(
        &self,
        request: ModelRequest,
        cancel: CancellationToken,
    ) -> Result<ModelStream, ModelError> {
        let provider = request.model.provider.clone();
        let slot = InFlightSlot::take(&self.in_flight, &provider);
        self.requests
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(request);
        let (next, gate) = {
            // One lock for the call number, the gate match, and the queue
            // pop: a concurrent caller must never see a call number this
            // call already claimed, or pop the response this call is about
            // to take. `armed_for_this_call` is read (not taken) before the
            // pop and only consumed after, so the lock provably spans both.
            let mut state = self.gate_state.lock().unwrap_or_else(|p| p.into_inner());
            state.call_count += 1;
            let call_number = state.call_count;
            let armed_for_this_call = matches!(&state.pending, Some((n, _)) if *n == call_number);
            let next = self
                .by_provider
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .get_mut(&provider)
                .and_then(VecDeque::pop_front)
                .or_else(|| {
                    self.responses
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .pop_front()
                });
            let gate = armed_for_this_call
                .then(|| state.pending.take())
                .flatten()
                .map(|(_, token)| token);
            (next, gate)
        };
        // A cancellation racing an empty queue must surface as Cancelled, not
        // as a scripting mistake, so check it before the empty-queue error.
        if cancel.is_cancelled() {
            return Err(crate::http::cancelled());
        }
        let events = match next {
            Some(Ok(events)) => events,
            Some(Err(e)) => return Err(e),
            None => {
                return Err(ModelError::new(
                    ModelErrorClass::Unknown,
                    "scripted client has no responses left",
                ));
            }
        };
        let delay = *self.delay.lock().unwrap_or_else(|p| p.into_inner());
        let stream = futures::stream::iter(events).then(move |event| {
            let cancel = cancel.clone();
            let gate = gate.clone();
            async move {
                if let Some(gate) = gate {
                    tokio::select! {
                        _ = gate.cancelled() => {}
                        _ = cancel.cancelled() => return Err(crate::http::cancelled()),
                    }
                }
                if delay > Duration::ZERO {
                    tokio::select! {
                        _ = tokio::time::sleep(delay) => {}
                        _ = cancel.cancelled() => return Err(crate::http::cancelled()),
                    }
                } else if cancel.is_cancelled() {
                    return Err(crate::http::cancelled());
                }
                Ok(event)
            }
        });
        // Stop after the first error so a cancelled stream ends cleanly. The
        // slot rides along, so the provider frees when the stream is done.
        let stream = stream.scan(false, move |failed, item| {
            let _slot = &slot;
            if *failed {
                return futures::future::ready(None);
            }
            *failed = item.is_err();
            futures::future::ready(Some(item))
        });
        Ok(Box::pin(stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::collect;
    use aether_core::model::{Message, ModelRef};
    use serde_json::json;

    fn request() -> ModelRequest {
        ModelRequest::new(
            ModelRef::new("scripted", "m"),
            vec![Message::user_text("hi")],
        )
    }

    #[tokio::test]
    async fn a_held_response_does_not_hold_later_responses() {
        use futures::FutureExt;
        let client = ScriptedClient::with_responses([
            ScriptedClient::text("first"),
            ScriptedClient::text("second"),
        ]);
        let release = client.hold_next_response();
        let mut first = client
            .stream(request(), CancellationToken::new())
            .await
            .expect("first stream");
        let second = client
            .stream(request(), CancellationToken::new())
            .await
            .expect("second stream");
        assert!(
            first.next().now_or_never().is_none(),
            "first response must remain held"
        );
        assert_eq!(
            collect(second, |_| {})
                .await
                .expect("second response")
                .text(),
            "second"
        );
        release.cancel();
        assert_eq!(
            collect(first, |_| {})
                .await
                .expect("released response")
                .text(),
            "first"
        );
        assert_eq!(client.remaining(), 0);
    }

    #[tokio::test]
    async fn a_held_response_still_honors_cancellation() {
        let client = ScriptedClient::with_responses([ScriptedClient::text("held")]);
        let _release = client.hold_next_response();
        let cancel = CancellationToken::new();
        let stream = client
            .stream(request(), cancel.clone())
            .await
            .expect("stream");
        cancel.cancel();
        let error = collect(stream, |_| {})
            .await
            .expect_err("cancelled while held");
        assert_eq!(error.class, ModelErrorClass::Cancelled);
    }

    #[tokio::test]
    async fn a_cancelled_call_against_an_empty_queue_yields_cancelled_not_the_scripting_error() {
        let client = ScriptedClient::with_responses([]);
        let cancel = CancellationToken::new();
        cancel.cancel();
        let result = client.stream(request(), cancel).await;
        let error = match result {
            Ok(_) => panic!("cancelled before the queue is even checked"),
            Err(e) => e,
        };
        assert_eq!(error.class, ModelErrorClass::Cancelled);
    }

    #[tokio::test]
    async fn plays_responses_in_order_and_records_requests() {
        let client = ScriptedClient::with_responses([
            ScriptedClient::text("hello"),
            ScriptedClient::tool_call("echo", json!({"x": 1})),
            ScriptedClient::error(ModelErrorClass::RateLimit, "slow"),
        ]);
        let r1 = collect(
            client
                .stream(request(), CancellationToken::new())
                .await
                .expect("s"),
            |_| {},
        )
        .await
        .expect("c");
        assert_eq!(r1.text(), "hello");
        assert_eq!(r1.stop_reason, Some(StopReason::EndTurn));
        let r2 = collect(
            client
                .stream(request(), CancellationToken::new())
                .await
                .expect("s"),
            |_| {},
        )
        .await
        .expect("c");
        assert_eq!(
            r2.tool_uses().collect::<Vec<_>>(),
            vec![("call_0", "echo", &json!({"x": 1}))]
        );
        let e = client
            .stream(request(), CancellationToken::new())
            .await
            .err()
            .expect("expected an error");
        assert_eq!(e.class, ModelErrorClass::RateLimit);
        assert_eq!(client.requests().len(), 3);
        assert_eq!(client.remaining(), 0);
        let e = client
            .stream(request(), CancellationToken::new())
            .await
            .err()
            .expect("expected an error");
        assert!(e.message.contains("no responses left"));
    }

    #[tokio::test]
    async fn cancellation_ends_the_stream() {
        let client = ScriptedClient::with_responses([ScriptedClient::text("a long answer")]);
        client.set_delay(Duration::from_millis(50));
        let cancel = CancellationToken::new();
        let stream = client
            .stream(request(), cancel.clone())
            .await
            .expect("stream");
        cancel.cancel();
        let err = collect(stream, |_| {})
            .await
            .expect_err("expected an error");
        assert_eq!(err.class, ModelErrorClass::Cancelled);
    }

    #[tokio::test]
    async fn multi_tool_response_with_text() {
        let client = ScriptedClient::with_responses([ScriptedClient::tool_calls(
            &[("a", json!({})), ("b", json!({"k": "v"}))],
            Some("doing both"),
        )]);
        let r = collect(
            client
                .stream(request(), CancellationToken::new())
                .await
                .expect("s"),
            |_| {},
        )
        .await
        .expect("c");
        assert_eq!(r.text(), "doing both");
        assert_eq!(r.tool_uses().count(), 2);
        assert_eq!(r.stop_reason, Some(StopReason::ToolUse));
    }
}
