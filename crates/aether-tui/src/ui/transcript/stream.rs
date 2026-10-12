//! The in-flight reply: the one seam onto the streaming markdown. Owner: W2.
//!
//! `stream_lines` is the only function that knows how streamed source becomes
//! rows: a `MarkdownStream` per reply re-renders only its unsettled tail, and
//! the guttered settled rows are kept and lent out, never copied per delta.

use aether_core::id::{AgentId, InvocationId};
use ratatui::text::Line;

use crate::app::transcript::StreamState;
use crate::ui::cells::assistant;
use crate::ui::cells::text::span;
use crate::ui::markdown::MarkdownStream;
use crate::ui::theme::tokens::Theme;

/// Source bytes a reply remembers to recognise its own text: a reply that
/// does not start with them is another one, whatever the bookkeeping says.
const HEAD_BYTES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StreamKey {
    len: usize,
    width: u16,
    theme_epoch: u32,
}

/// One agent's reply being rendered; any other agent, invocation or stream
/// generation starts over.
#[derive(Debug, Default)]
pub struct StreamCache {
    presentation: Option<(u32, bool, bool)>,
    pub(super) visible: super::projection::VisibleIndex,
    reply: Option<Reply>,
}

impl StreamCache {
    pub(super) fn set_presentation(&mut self, key: (u32, bool, bool)) {
        if self.presentation.is_some_and(|old| old != key) {
            self.reply = None;
        }
        self.presentation = Some(key);
    }
}

#[derive(Debug)]
struct Reply {
    agent: AgentId,
    invocation: Option<InvocationId>,
    generation: u64,
    head: String,
    markdown: MarkdownStream,
    /// Bytes of `StreamState::text` pushed into `markdown`.
    fed: usize,
    /// `markdown`'s settled rows with the gutter, valid for `markdown_generation`.
    settled: Vec<Line<'static>>,
    markdown_generation: Option<u64>,
    /// Rows of `settled` shown; the rest are blank rows held back until text follows.
    shown: usize,
    /// The guttered tail, ending in the caret.
    tail: Vec<Line<'static>>,
    key: Option<StreamKey>,
}

impl Reply {
    fn new(agent: AgentId, stream: &StreamState) -> Reply {
        Reply {
            agent,
            invocation: stream.invocation,
            generation: stream.generation,
            head: String::new(),
            markdown: MarkdownStream::new(),
            fed: 0,
            settled: Vec::new(),
            markdown_generation: None,
            shown: 0,
            tail: Vec::new(),
            key: None,
        }
    }

    /// Whether `stream` is this reply grown by appending.
    fn continues(&self, agent: AgentId, stream: &StreamState) -> bool {
        let text = &stream.text;
        self.agent == agent
            && self.invocation == stream.invocation
            && self.generation == stream.generation
            && self.fed <= text.len()
            && text.is_char_boundary(self.fed)
            && text.starts_with(&self.head)
    }

    fn view(&self) -> StreamView<'_> {
        StreamView {
            settled: &self.settled[..self.shown],
            tail: &self.tail,
        }
    }
}

/// The streaming reply's rows, borrowed from the cache: settled rows, then
/// the tail. Copy, so a frame takes it without touching the rows.
#[derive(Debug, Clone, Copy)]
pub struct StreamView<'a> {
    settled: &'a [Line<'static>],
    tail: &'a [Line<'static>],
}

impl<'a> StreamView<'a> {
    pub fn len(&self) -> usize {
        self.settled.len() + self.tail.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get(&self, row: usize) -> Option<&'a Line<'static>> {
        match row.checked_sub(self.settled.len()) {
            None => self.settled.get(row),
            Some(row) => self.tail.get(row),
        }
    }
}

/// Rows for the reply streaming into `agent`'s transcript, ending in the
/// dim `▍` caret; `None` when no reply text is streaming. A delta costs what
/// it added and the tail, however long the reply already is.
pub fn stream_lines<'a>(
    agent: AgentId,
    stream: &StreamState,
    width: u16,
    theme: &Theme,
    theme_epoch: u32,
    cache: &'a mut StreamCache,
) -> Option<StreamView<'a>> {
    if stream.text.trim().is_empty() {
        return None;
    }
    let key = StreamKey {
        len: stream.text.len(),
        width,
        theme_epoch,
    };
    let reply = match cache.reply.take() {
        Some(r) if r.continues(agent, stream) => r,
        _ => Reply::new(agent, stream),
    };
    let reply = cache.reply.insert(reply);
    if reply.key == Some(key) {
        return Some(reply.view());
    }
    reply.key = Some(key);
    reply.markdown.push(&stream.text[reply.fed..]);
    reply.fed = stream.text.len();
    if reply.head.len() < HEAD_BYTES {
        let mut end = stream.text.len().min(HEAD_BYTES);
        while !stream.text.is_char_boundary(end) {
            end -= 1;
        }
        reply.head = stream.text[..end].to_owned();
    }

    // The committed cell's wrap width, so nothing reflows on commit; a caret
    // after a full row falls off the edge rather than taking a column.
    let view = reply
        .markdown
        .lines(assistant::text_width(width), theme.text(), theme);
    if reply.markdown_generation != Some(view.generation()) {
        reply.settled.clear();
        reply.markdown_generation = Some(view.generation());
    }
    // Settled rows only grow within a generation; the view may hold back
    // trailing blank rows, which stay kept here rather than being re-guttered.
    let done = reply.settled.len();
    if let Some(fresh) = view.settled().get(done..) {
        reply
            .settled
            .extend(assistant::gutter_from(done, fresh.to_vec(), theme));
    }
    reply.shown = view.settled().len();
    reply.tail = assistant::gutter_from(reply.shown, view.tail().to_vec(), theme).collect();
    // The caret ends the last row; a settled one is copied into the tail to carry it.
    if reply.tail.is_empty() && reply.shown > 0 {
        reply.shown -= 1;
        reply.tail.push(reply.settled[reply.shown].clone());
    }
    if let Some(last) = reply.tail.last_mut() {
        last.spans
            .push(span(theme.glyphs().caret, theme.secondary()));
    }
    Some(reply.view())
}
