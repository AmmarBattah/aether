//! Incremental Server-Sent Events parser.
//!
//! Bytes arrive in arbitrary chunks; the parser buffers partial lines,
//! accepts `\n`, `\r\n` and `\r` line endings, joins multi-line `data:`
//! fields with newlines, and ignores comments and unknown fields.

use std::sync::Arc;

use aether_core::model::{ModelError, ModelErrorClass};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SseEvent {
    pub event: Option<String>,
    pub data: String,
    pub id: Option<Arc<str>>,
}

#[cfg(test)]
const MAX_LINE_BYTES: usize = MAX_EVENT_BYTES;
const MAX_EVENT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Default)]
pub struct SseParser {
    buf: Vec<u8>,
    pending_cr: bool,
    event: Option<String>,
    data: String,
    has_data: bool,
    id: Option<Arc<str>>,
}

impl SseParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed a chunk; a size-limit error terminates the stream.
    #[cfg(test)]
    pub fn feed(&mut self, chunk: &[u8]) -> Result<Vec<SseEvent>, ModelError> {
        let mut out = Vec::new();
        self.feed_into(chunk, &mut out)?;
        Ok(out)
    }

    /// Preserve events completed before a malformed line in the same chunk.
    pub(crate) fn feed_into(
        &mut self,
        chunk: &[u8],
        out: &mut Vec<SseEvent>,
    ) -> Result<(), ModelError> {
        for &byte in chunk {
            if self.pending_cr {
                self.pending_cr = false;
                self.complete_line(out)?;
                if byte == b'\n' {
                    continue;
                }
            }
            match byte {
                b'\r' => self.pending_cr = true,
                b'\n' => self.complete_line(out)?,
                _ => {
                    if self.buffered_bytes() >= MAX_EVENT_BYTES {
                        return Err(limit_error("line and event data"));
                    }
                    self.buf.push(byte);
                }
            }
        }
        Ok(())
    }

    /// Flush a trailing event that was never terminated by a blank line.
    pub fn finish(&mut self) -> Result<Vec<SseEvent>, ModelError> {
        let mut out = Vec::new();
        if self.pending_cr || !self.buf.is_empty() {
            self.pending_cr = false;
            self.complete_line(&mut out)?;
        }
        if let Some(ev) = self.dispatch() {
            out.push(ev);
        }
        Ok(out)
    }

    fn complete_line(&mut self, out: &mut Vec<SseEvent>) -> Result<(), ModelError> {
        let line = String::from_utf8_lossy(&self.buf).into_owned();
        self.buf.clear();
        self.process_line(&line, out)
    }

    fn process_line(&mut self, line: &str, out: &mut Vec<SseEvent>) -> Result<(), ModelError> {
        if line.is_empty() {
            if let Some(ev) = self.dispatch() {
                out.push(ev);
            }
            return Ok(());
        }
        if line.starts_with(':') {
            return Ok(());
        }
        let (field, value) = match line.split_once(':') {
            Some((f, v)) => (f, v.strip_prefix(' ').unwrap_or(v)),
            None => (line, ""),
        };
        match field {
            "event" => self.event = Some(value.to_owned()),
            "data" => {
                let length = self.data.len() + usize::from(self.has_data) + value.len();
                if length > MAX_EVENT_BYTES {
                    return Err(limit_error("event data"));
                }
                if self.has_data {
                    self.data.push('\n');
                }
                self.data.push_str(value);
                self.has_data = true;
            }
            "id" => self.id = Some(Arc::from(value)),
            _ => {}
        }
        if self.buffered_bytes() > MAX_EVENT_BYTES {
            return Err(limit_error("line and event data"));
        }
        Ok(())
    }

    fn buffered_bytes(&self) -> usize {
        self.buf.len()
            + self.data.len()
            + self.event.as_ref().map_or(0, String::len)
            + self.id.as_ref().map_or(0, |id| id.len())
    }

    fn dispatch(&mut self) -> Option<SseEvent> {
        if !self.has_data {
            self.event = None;
            return None;
        }
        self.has_data = false;
        Some(SseEvent {
            event: self.event.take(),
            data: std::mem::take(&mut self.data),
            id: self.id.clone(),
        })
    }
}

fn limit_error(part: &str) -> ModelError {
    ModelError::new(
        ModelErrorClass::Unknown,
        format!("SSE {part} exceeds 16 MiB limit"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(event: Option<&str>, data: &str) -> SseEvent {
        SseEvent {
            event: event.map(str::to_owned),
            data: data.to_owned(),
            id: None,
        }
    }

    #[test]
    fn parses_basic_events() {
        let mut p = SseParser::new();
        let out = p
            .feed(b"event: ping\ndata: {\"a\":1}\n\ndata: second\n\n")
            .expect("bounded input");
        assert_eq!(out, vec![ev(Some("ping"), "{\"a\":1}"), ev(None, "second")]);
    }

    #[test]
    fn handles_chunk_boundaries_mid_line() {
        let mut p = SseParser::new();
        let mut out = p.feed(b"event: mes").expect("bounded input");
        assert!(out.is_empty());
        out.extend(p.feed(b"sage_start\nda").expect("bounded input"));
        assert!(out.is_empty());
        out.extend(p.feed(b"ta: hel").expect("bounded input"));
        out.extend(p.feed(b"lo\n").expect("bounded input"));
        assert!(out.is_empty());
        out.extend(p.feed(b"\n").expect("bounded input"));
        assert_eq!(out, vec![ev(Some("message_start"), "hello")]);
    }

    #[test]
    fn joins_multiline_data_and_strips_one_space() {
        let mut p = SseParser::new();
        let out = p
            .feed(b"data: line one\ndata:line two\ndata:  spaced\n\n")
            .expect("bounded input");
        assert_eq!(out, vec![ev(None, "line one\nline two\n spaced")]);
    }

    #[test]
    fn ignores_comments_and_unknown_fields() {
        let mut p = SseParser::new();
        let out = p
            .feed(b": keep-alive\nretry: 1000\nfoo: bar\ndata: x\n\n")
            .expect("bounded input");
        assert_eq!(out, vec![ev(None, "x")]);
    }

    #[test]
    fn crlf_and_cr_line_endings() {
        let mut p = SseParser::new();
        let out = p
            .feed(b"data: a\r\n\r\ndata: b\r\rdata: c\n\n")
            .expect("bounded input");
        assert_eq!(out, vec![ev(None, "a"), ev(None, "b"), ev(None, "c")]);
    }

    #[test]
    fn trailing_cr_waits_for_possible_lf() {
        let mut p = SseParser::new();
        assert!(p.feed(b"data: a\r").expect("bounded input").is_empty());
        let out = p.feed(b"\n\n").expect("bounded input");
        assert_eq!(out, vec![ev(None, "a")]);
    }

    #[test]
    fn blank_line_without_data_dispatches_nothing() {
        let mut p = SseParser::new();
        assert!(p.feed(b"event: x\n\n").expect("bounded input").is_empty());
        let out = p.feed(b"data: y\n\n").expect("bounded input");
        assert_eq!(out, vec![ev(None, "y")]);
    }

    #[test]
    fn finish_flushes_unterminated_event() {
        let mut p = SseParser::new();
        assert!(p.feed(b"data: [DONE]").expect("bounded input").is_empty());
        assert_eq!(p.finish().expect("bounded input"), vec![ev(None, "[DONE]")]);
        assert!(p.finish().expect("bounded input").is_empty());
    }

    #[test]
    fn id_field_is_carried() {
        let mut p = SseParser::new();
        let out = p.feed(b"id: 7\ndata: z\n\n").expect("bounded input");
        assert_eq!(out[0].id.as_deref(), Some("7"));
    }

    #[test]
    fn utf8_split_across_chunks() {
        let mut p = SseParser::new();
        let text = "data: héllo wörld\n\n".as_bytes();
        let mut out = Vec::new();
        for b in text {
            out.extend(p.feed(&[*b]).expect("bounded input"));
        }
        assert_eq!(out, vec![ev(None, "héllo wörld")]);
    }

    #[test]
    fn unterminated_lines_are_bounded_even_across_small_chunks() {
        let mut parser = SseParser::new();
        let chunk = vec![b'x'; 1024];
        for _ in 0..MAX_LINE_BYTES / chunk.len() {
            assert!(parser.feed(&chunk).unwrap().is_empty());
        }
        assert_eq!(parser.buf.len(), MAX_LINE_BYTES);
        assert!(parser.feed(b"x").unwrap_err().message.contains("line"));
        assert_eq!(parser.buf.len(), MAX_LINE_BYTES);
    }

    #[test]
    fn multiline_events_include_separators_in_the_size_limit() {
        let mut parser = SseParser::new();
        let value = "x".repeat(MAX_EVENT_BYTES / 2);
        assert!(
            parser
                .feed(format!("data:{value}\n").as_bytes())
                .unwrap()
                .is_empty()
        );
        assert!(
            parser
                .feed(format!("data:{value}\n").as_bytes())
                .unwrap_err()
                .message
                .contains("event data")
        );
    }

    #[test]
    fn event_limit_resets_after_dispatch_and_empty_data_still_dispatches() {
        let mut parser = SseParser::new();
        let value = "x".repeat(MAX_EVENT_BYTES - 5);
        for _ in 0..2 {
            let events = parser.feed(format!("data:{value}\n\n").as_bytes()).unwrap();
            assert_eq!(events[0].data, value);
        }
        assert_eq!(
            parser.feed(b"data:\ndata:\n\n").unwrap(),
            vec![ev(None, "\n")]
        );
    }
}

#[cfg(test)]
mod shared_budget_tests {
    use super::*;
    #[test]
    fn id_event_data_and_partial_line_share_one_budget() {
        let value = "x".repeat(MAX_EVENT_BYTES / 3);
        let mut parser = SseParser::new();
        parser
            .feed(format!("id:{value}\nevent:{value}\n").as_bytes())
            .unwrap();
        assert!(parser.feed(format!("data:{value}\n").as_bytes()).is_err());
        let mut parser = SseParser::new();
        parser
            .feed(format!("id:{value}\ndata:{value}\n").as_bytes())
            .unwrap();
        assert!(parser.feed(value.as_bytes()).is_ok());
        assert!(parser.feed(b"xx").is_err());
    }
    #[test]
    fn persistent_ids_are_shared_across_events() {
        let mut parser = SseParser::new();
        let events = parser
            .feed(b"id:token\ndata:first\n\ndata:second\n\n")
            .unwrap();
        assert!(Arc::ptr_eq(
            events[0].id.as_ref().unwrap(),
            events[1].id.as_ref().unwrap()
        ));
    }
}
