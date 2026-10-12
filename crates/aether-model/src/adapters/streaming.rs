//! Bridge from an HTTP body to a [`ModelStream`]: bytes → SSE events →
//! adapter-specific mapping → normalized events, with cancellation.

use std::collections::VecDeque;
use std::pin::Pin;

use aether_core::model::{ModelError, StreamEvent};
use bytes::Bytes;
use futures::{Stream, StreamExt};
use tokio_util::sync::CancellationToken;

use crate::client::ModelStream;
use crate::http::{cancelled, map_transport_error};
use crate::sse::{SseEvent, SseParser};

/// Adapter-specific translation of SSE events.
pub trait EventMapper: Send + 'static {
    /// Map one SSE event to zero or more stream events, or a terminal error.
    fn map(&mut self, event: &SseEvent) -> Result<Vec<StreamEvent>, ModelError>;

    /// The body ended. Emit whatever closes the message (block stops, final
    /// stop reason) so consumers always see a complete shape.
    fn finish(&mut self) -> Result<Vec<StreamEvent>, ModelError>;
}

type BodyStream = Pin<Box<dyn Stream<Item = reqwest::Result<Bytes>> + Send>>;

struct State<M> {
    body: BodyStream,
    parser: SseParser,
    mapper: M,
    pending: VecDeque<StreamEvent>,
    /// Error to emit once `pending` drains; events mapped before a failure
    /// are still delivered so partial content is not lost.
    deferred: Option<ModelError>,
    done: bool,
    cancel: CancellationToken,
}

impl<M: EventMapper> State<M> {
    /// Map a batch of SSE events, stopping at the first failure.
    fn map_all(&mut self, events: Vec<SseEvent>) {
        for sse in events {
            match self.mapper.map(&sse) {
                Ok(mapped) => self.pending.extend(mapped),
                Err(e) => {
                    self.deferred = Some(e);
                    self.done = true;
                    return;
                }
            }
        }
    }

    fn fail(&mut self, err: ModelError) {
        self.deferred = Some(err);
        self.done = true;
    }
}

/// Turn a streaming response into a `ModelStream`. An `Err` item is
/// terminal: the stream yields nothing after it.
pub fn sse_stream<M: EventMapper>(
    response: reqwest::Response,
    mapper: M,
    cancel: CancellationToken,
) -> ModelStream {
    let state = State {
        body: Box::pin(response.bytes_stream()),
        parser: SseParser::new(),
        mapper,
        pending: VecDeque::new(),
        deferred: None,
        done: false,
        cancel,
    };
    Box::pin(futures::stream::unfold(state, |mut st| async move {
        loop {
            if let Some(ev) = st.pending.pop_front() {
                return Some((Ok(ev), st));
            }
            if let Some(err) = st.deferred.take() {
                st.done = true;
                st.pending.clear();
                return Some((Err(err), st));
            }
            if st.done {
                return None;
            }
            let chunk = tokio::select! {
                _ = st.cancel.cancelled() => {
                    st.fail(cancelled());
                    continue;
                }
                chunk = st.body.next() => chunk,
            };
            match chunk {
                Some(Ok(bytes)) => {
                    let mut events = Vec::new();
                    let result = st.parser.feed_into(&bytes, &mut events);
                    st.map_all(events);
                    if st.deferred.is_none()
                        && let Err(err) = result
                    {
                        st.fail(err);
                    }
                }
                Some(Err(e)) => st.fail(map_transport_error(e)),
                None => {
                    match st.parser.finish() {
                        Ok(events) => st.map_all(events),
                        Err(err) => st.fail(err),
                    }
                    if st.deferred.is_none() {
                        match st.mapper.finish() {
                            Ok(events) => st.pending.extend(events),
                            Err(e) => st.deferred = Some(e),
                        }
                    }
                    st.done = true;
                }
            }
        }
    }))
}

/// Build a `ModelStream` from a fixed list of SSE events, for adapter tests
/// and for the scripted client. Shares the mapping path with live streams.
pub fn mapped_stream<M: EventMapper>(events: Vec<SseEvent>, mut mapper: M) -> ModelStream {
    let mut out: Vec<Result<StreamEvent, ModelError>> = Vec::new();
    let mut failed = false;
    for sse in events {
        match mapper.map(&sse) {
            Ok(mapped) => out.extend(mapped.into_iter().map(Ok)),
            Err(e) => {
                out.push(Err(e));
                failed = true;
                break;
            }
        }
    }
    if !failed {
        match mapper.finish() {
            Ok(mapped) => out.extend(mapped.into_iter().map(Ok)),
            Err(e) => out.push(Err(e)),
        }
    }
    Box::pin(futures::stream::iter(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct UnexpectedMapper;

    impl EventMapper for UnexpectedMapper {
        fn map(&mut self, _: &SseEvent) -> Result<Vec<StreamEvent>, ModelError> {
            panic!("oversized input must not reach the mapper")
        }

        fn finish(&mut self) -> Result<Vec<StreamEvent>, ModelError> {
            panic!("parser failure must not synthesize a successful finish")
        }
    }

    #[tokio::test]
    async fn oversized_sse_is_a_terminal_error_without_waiting_for_more_body() {
        let chunks = futures::stream::iter([Ok::<_, std::io::Error>(Bytes::from(vec![
            b'x';
            16 * 1024
                * 1024
                + 1
        ]))])
        .chain(futures::stream::pending());
        let response = ::http::Response::builder()
            .body(reqwest::Body::wrap_stream(chunks))
            .unwrap()
            .into();
        let mut stream = sse_stream(response, UnexpectedMapper, CancellationToken::new());
        let err = tokio::time::timeout(std::time::Duration::from_secs(5), stream.next())
            .await
            .expect("size failure must not await EOF")
            .unwrap()
            .unwrap_err();
        assert!(err.message.contains("SSE line"));
        assert!(stream.next().await.is_none());
    }

    struct TextMapper;

    impl EventMapper for TextMapper {
        fn map(&mut self, event: &SseEvent) -> Result<Vec<StreamEvent>, ModelError> {
            Ok(vec![StreamEvent::TextDelta {
                index: 0,
                text: event.data.clone(),
            }])
        }

        fn finish(&mut self) -> Result<Vec<StreamEvent>, ModelError> {
            panic!("parser failure must not synthesize a successful finish")
        }
    }

    #[tokio::test]
    async fn completed_content_precedes_size_error_in_the_same_chunk() {
        let mut body = b"data:partial content\n\n".to_vec();
        body.extend(vec![b'x'; 16 * 1024 * 1024 + 1]);
        let response = ::http::Response::builder().body(body).unwrap().into();
        let mut stream = sse_stream(response, TextMapper, CancellationToken::new());
        assert!(
            matches!(stream.next().await.unwrap().unwrap(), StreamEvent::TextDelta { text, .. } if text == "partial content")
        );
        assert!(
            stream
                .next()
                .await
                .unwrap()
                .unwrap_err()
                .message
                .contains("SSE line")
        );
        assert!(stream.next().await.is_none());
    }
}
