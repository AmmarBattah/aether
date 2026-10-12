//! Markdown that arrives in pieces, rendered so each update costs what is
//! still open rather than the whole reply.
//!
//! An update re-parses only from the last settled block start. Everything
//! before such a start renders the same however the text continues, so those
//! lines are kept. An unclosed fence, a table, or the block still being
//! written stays in the tail and is rendered again until a later block settles it.

use pulldown_cmark::{Event, Parser};
use ratatui::style::Style;
use ratatui::text::Line;

use super::{Renderer, options, render_clean as render, trim_blank_end};
use crate::ui::cells::text::clean_settled;
use crate::ui::theme::Theme;

/// A markdown source that grows by deltas, with its rendering cached.
/// [`MarkdownStream::lines`] always equals [`super::render`] of everything pushed.
#[derive(Debug, Default)]
pub struct MarkdownStream {
    /// The pushed text through `clean`.
    source: String,
    /// Pushed text inside an escape sequence that has not ended yet.
    open_escape: String,
    finished: bool,
    cache: Cache,
}

impl MarkdownStream {
    pub fn new() -> MarkdownStream {
        MarkdownStream::default()
    }

    /// Append text to the source.
    pub fn push(&mut self, delta: &str) {
        debug_assert!(!self.finished, "a finished stream takes no more text");
        self.open_escape.push_str(delta);
        let (cleaned, settled) = clean_settled(&self.open_escape);
        self.source.push_str(&cleaned);
        self.open_escape.drain(..settled);
    }

    /// Mark the source complete. Rendering is unchanged: a finished reply
    /// renders as it would have had it all arrived at once.
    pub fn finish(&mut self) {
        self.finished = true;
        self.open_escape.clear();
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// The text pushed so far, cleaned of terminal controls.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// The source rendered at `width` with body style `base`. Re-renders
    /// only the unsettled tail, and nothing when the source has not grown.
    pub fn lines(&mut self, width: usize, base: Style, theme: &Theme) -> StreamLines<'_> {
        let key = (width.max(8), base, *theme);
        if self.cache.key != Some(key) {
            self.cache = Cache {
                key: Some(key),
                generation: self.cache.generation + 1,
                ..Cache::default()
            };
        }
        if self.cache.rendered != Some(self.source.len()) {
            self.cache.update(&self.source, key.0, base, theme);
        }
        self.cache.view()
    }
}

/// Rendered lines: the settled ones, then the tail rendered on the last update.
#[derive(Debug, Clone, Copy)]
pub struct StreamLines<'a> {
    generation: u64,
    settled: &'a [Line<'static>],
    tail: &'a [Line<'static>],
}

impl<'a> StreamLines<'a> {
    /// Changes whenever earlier settled lines may differ (new width, style or
    /// theme, or a reply rendered whole). Otherwise settled lines only grow,
    /// except that blank lines at their end are left out while no tail follows.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn settled(&self) -> &'a [Line<'static>] {
        self.settled
    }

    pub fn tail(&self) -> &'a [Line<'static>] {
        self.tail
    }

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

    pub fn iter(&self) -> impl Iterator<Item = &'a Line<'static>> + 'a {
        self.settled.iter().chain(self.tail)
    }
}

#[derive(Debug, Default)]
struct Cache {
    key: Option<(usize, Style, Theme)>,
    generation: u64,
    /// Source length the lines were rendered from.
    rendered: Option<usize>,
    /// Bytes of source `settled` covers; always a line start.
    committed: usize,
    /// Lines of `source[..committed]`, ending on the blank line the next block
    /// would have added, which `view` drops when nothing follows.
    settled: Vec<Line<'static>>,
    tail: Vec<Line<'static>>,
    /// A reference definition resolves links anywhere in the reply, so
    /// nothing is settled for it: it renders whole on every update.
    whole: bool,
}

impl Cache {
    fn update(&mut self, source: &str, width: usize, base: Style, theme: &Theme) {
        self.rendered = Some(source.len());
        if self.whole {
            self.tail = render(source, width, base, theme);
            return;
        }
        let pass = Pass::run(&source[self.committed..], width, base, theme);
        if pass.defines_references {
            self.whole = true;
            self.generation += 1;
            self.committed = 0;
            self.settled.clear();
            self.tail = render(source, width, base, theme);
            return;
        }
        let mut lines = pass.lines;
        if let Some(split) = pass.split {
            self.settled.extend(lines.drain(..split.lines));
            self.committed += split.offset;
        }
        trim_blank_end(&mut lines);
        self.tail = lines;
    }

    fn view(&self) -> StreamLines<'_> {
        let mut settled = self.settled.as_slice();
        // The whole render trims blank lines at its end; with no tail, that end is here.
        if self.tail.is_empty() {
            while let [rest @ .., last] = settled
                && last.spans.is_empty()
            {
                settled = rest;
            }
        }
        StreamLines {
            generation: self.generation,
            settled,
            tail: &self.tail,
        }
    }
}

/// One render of the unsettled text, and the last place it could be split.
struct Pass {
    lines: Vec<Line<'static>>,
    split: Option<Split>,
    defines_references: bool,
}

/// A settled block start: `offset` into the text, and how many lines the
/// blocks before it rendered.
struct Split {
    offset: usize,
    lines: usize,
}

impl Pass {
    fn run(text: &str, width: usize, base: Style, theme: &Theme) -> Pass {
        let parser = Parser::new_ext(text, options());
        let defines_references = parser.reference_definitions().iter().next().is_some();
        let settled_until = second_last_newline(text);
        let mut renderer = Renderer::new(width, base, theme);
        let mut depth = 0usize;
        let mut split = None;
        for (event, range) in parser.into_offset_iter() {
            let block_start = depth == 0 && matches!(event, Event::Start(_) | Event::Rule);
            if block_start
                && renderer.at_rest()
                && let Some(offset) = settled_line_start(text, range.start, settled_until)
            {
                split = Some(Split {
                    offset,
                    lines: renderer.out.len(),
                });
            }
            match event {
                Event::Start(_) => depth += 1,
                Event::End(_) => depth = depth.saturating_sub(1),
                _ => {}
            }
            renderer.event(event);
        }
        Pass {
            lines: renderer.into_lines(),
            split,
            defines_references,
        }
    }
}

impl Renderer<'_> {
    /// Nothing open and nothing pending, the last line blank: a fresh renderer
    /// would draw what follows identically, since `blank` reads an empty
    /// output as it reads a blank last line.
    fn at_rest(&self) -> bool {
        self.runs.is_empty()
            && self.styles.is_empty()
            && self.lists.is_empty()
            && self.quote == 0
            && self.code.is_none()
            && self.table.is_none()
            && self.link.is_none()
            && self.out.lines().last().is_none_or(|l| l.spans.is_empty())
    }
}

/// Where the line of a block starting at `at` begins, if the text may be split
/// there: the block opens its line, and that line and the next are complete.
/// A partial line can still change kind (`#` to `#x`), and so can a complete
/// one until the next arrives (a table header needs its delimiter row).
fn settled_line_start(text: &str, at: usize, settled_until: Option<usize>) -> Option<usize> {
    if settled_until.is_none_or(|until| at > until) {
        return None;
    }
    let start = text[..at].rfind('\n').map_or(0, |i| i + 1);
    let indent_only = text[start..at].bytes().all(|b| b == b' ' || b == b'\t');
    (start > 0 && indent_only).then_some(start)
}

fn second_last_newline(text: &str) -> Option<usize> {
    let last = text.rfind('\n')?;
    text[..last].rfind('\n')
}

#[cfg(test)]
#[path = "stream_tests.rs"]
mod tests;
