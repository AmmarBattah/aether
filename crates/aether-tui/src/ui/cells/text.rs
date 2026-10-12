//! Shared cell layout: sanitising, word wrap, truncation, the `└` gutter and
//! the 5-row output preview (SPEC §3 rules).

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::ui::text_width::{display_width, grapheme_width};

use super::{PREVIEW_HEAD_ROWS, PREVIEW_MAX_ROWS, PREVIEW_TAIL_ROWS};
use crate::app::transcript::{ArtifactLink, Cost};
use crate::format;
use crate::ui::theme::tokens::Theme;

pub type Spans = Vec<Span<'static>>;

pub fn span(text: impl Into<String>, style: Style) -> Span<'static> {
    Span::styled(text.into(), style)
}

pub fn width_of(spans: &[Span<'_>]) -> usize {
    spans.iter().map(|s| display_width(&s.content)).sum()
}

/// Terminal-safe text: tabs become spaces, ANSI escapes and other control
/// characters (tool output carries them) are dropped. Newlines stay.
pub fn clean(text: &str) -> String {
    clean_settled(text).0
}

/// [`clean`] of `text`, and how many bytes of it end outside an escape
/// sequence: a stream holds the rest back until the sequence completes.
pub fn clean_settled(text: &str) -> (String, usize) {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.char_indices();
    let mut settled = text.len();
    while let Some((at, ch)) = chars.next() {
        match ch {
            '\n' => out.push('\n'),
            '\t' => out.push_str("    "),
            '\u{1b}' => {
                let complete = match chars.next() {
                    Some((_, '[')) => chars.by_ref().any(|(_, c)| ('@'..='~').contains(&c)),
                    Some((_, ']')) => control_string_complete(&mut chars, true),
                    Some((_, 'P' | 'X' | '^' | '_')) => control_string_complete(&mut chars, false),
                    Some((_, ' '..='/')) => chars.by_ref().any(|(_, c)| ('0'..='~').contains(&c)),
                    next => next.is_some(),
                };
                if !complete {
                    settled = at;
                }
            }
            '\u{9b}' => {
                if !chars.by_ref().any(|(_, c)| ('@'..='~').contains(&c)) {
                    settled = at;
                }
            }
            c @ ('\u{90}' | '\u{98}' | '\u{9d}'..='\u{9f}') => {
                if !control_string_complete(&mut chars, c == '\u{9d}') {
                    settled = at;
                }
            }
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    (out, settled)
}

fn control_string_complete(chars: &mut std::str::CharIndices<'_>, bell_ends: bool) -> bool {
    let mut escape = false;
    for (_, c) in chars {
        if c == '\u{9c}' || (bell_ends && c == '\u{7}') || (escape && c == '\\') {
            return true;
        }
        escape = c == '\u{1b}';
    }
    false
}

/// `  └ ` for the first child row, `    ` after it.
pub fn child_prefix(theme: &Theme, first: bool) -> Span<'static> {
    if first {
        span(format!("  {} ", theme.glyphs().child), theme.secondary())
    } else {
        span("    ", theme.text())
    }
}

/// Prefix rows with the `└` gutter.
pub fn under(mut rows: Layout, theme: &Theme) -> Layout {
    for (i, (line, source)) in rows.rows_mut().enumerate() {
        let prefix = child_prefix(theme, i == 0);
        source.prefix += prefix.content.width();
        line.spans.insert(0, prefix);
    }
    rows
}

/// Cut `spans` to `width` columns, ending in the theme's ellipsis when cut.
pub fn truncate(spans: Spans, width: usize, theme: &Theme) -> Spans {
    if width_of(&spans) <= width {
        return spans;
    }
    let ellipsis = theme.glyphs().ellipsis;
    let room = width.saturating_sub(ellipsis.width());
    let mut out = Vec::new();
    let mut used = 0;
    let mut last_style = theme.text();
    for s in spans {
        last_style = s.style;
        let sw = display_width(&s.content);
        if used + sw <= room {
            used += sw;
            out.push(s);
            continue;
        }
        let mut cut = String::new();
        for grapheme in s.content.graphemes(true) {
            let cw = grapheme_width(grapheme);
            if used + cw > room {
                break;
            }
            used += cw;
            cut.push_str(grapheme);
        }
        if !cut.is_empty() {
            out.push(Span::styled(cut, s.style));
        }
        break;
    }
    if width >= ellipsis.width() {
        out.push(Span::styled(ellipsis, last_style));
    }
    out
}

/// Cut `spans` to `width` columns, without an ellipsis.
fn clip(spans: Spans, width: usize) -> Spans {
    if width_of(&spans) <= width {
        return spans;
    }
    let mut out = Vec::new();
    let mut used = 0;
    for s in spans {
        let mut cut = String::new();
        for grapheme in s.content.graphemes(true) {
            let cw = grapheme_width(grapheme);
            if used + cw > width {
                break;
            }
            used += cw;
            cut.push_str(grapheme);
        }
        let clipped = cut.len() < s.content.len();
        if !cut.is_empty() {
            out.push(Span::styled(cut, s.style));
        }
        if clipped || used >= width {
            break;
        }
    }
    out
}

/// Where a laid-out row's text sits in its source line, so find can match
/// across a soft wrap without the gutters and indents between the parts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RowSource {
    /// Columns of gutter, indent or marker before the row's text.
    pub prefix: usize,
    /// Set when the row soft-wraps on from the row above: the spaces the break dropped.
    pub continues: Option<usize>,
}

impl RowSource {
    /// A row that starts its own source line behind `prefix` columns.
    pub fn starts(prefix: usize) -> Self {
        Self {
            prefix,
            continues: None,
        }
    }

    /// `line`'s text, and the byte its part of the source line starts at. A
    /// row starting the line keeps its prefix, as the plain text find indexes does.
    pub fn text_of(self, line: &Line<'_>) -> (String, usize) {
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        if self.continues.is_none() {
            return (text, 0);
        }
        let mut width = 0;
        let start = text
            .grapheme_indices(true)
            .find(|&(_, grapheme)| {
                let past = width >= self.prefix;
                width += grapheme_width(grapheme);
                past
            })
            .map_or(text.len(), |(at, _)| at);
        (text, start)
    }
}

/// Laid-out rows, each with its [`RowSource`].
#[derive(Debug, Clone, Default)]
pub struct Layout {
    lines: Vec<Line<'static>>,
    sources: Vec<RowSource>,
}

impl Layout {
    /// Rows whose wrapping is unknown: each is matched as its own source line.
    pub fn unmapped(lines: Vec<Line<'static>>) -> Self {
        let sources = vec![RowSource::default(); lines.len()];
        Self { lines, sources }
    }

    pub fn push(&mut self, line: Line<'static>, source: RowSource) {
        self.lines.push(line);
        self.sources.push(source);
    }

    /// A one-row source line: `spans` cut to `width`, its text after the gutter `spans[0]`.
    pub fn push_row(&mut self, spans: Spans, width: usize, theme: &Theme) {
        let prefix = spans.first().map_or(0, |gutter| gutter.content.width());
        self.push(
            Line::from(truncate(spans, width, theme)),
            RowSource::starts(prefix),
        );
    }

    pub fn append(&mut self, mut other: Layout) {
        self.lines.append(&mut other.lines);
        self.sources.append(&mut other.sources);
    }

    pub fn lines(&self) -> &[Line<'static>] {
        &self.lines
    }

    pub fn into_lines(self) -> Vec<Line<'static>> {
        self.lines
    }

    pub fn source(&self, row: usize) -> Option<RowSource> {
        self.sources.get(row).copied()
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Edit rows in place; `prefix` grows by what an edit puts before the text.
    pub fn rows_mut(&mut self) -> impl Iterator<Item = (&mut Line<'static>, &mut RowSource)> {
        self.lines.iter_mut().zip(&mut self.sources)
    }

    pub fn truncate(&mut self, rows: usize) {
        self.lines.truncate(rows);
        self.sources.truncate(rows);
    }

    /// Source lines rebuilt from rows, soft wraps rejoined over their dropped spaces.
    pub fn rejoined(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for (line, source) in self.lines.iter().zip(&self.sources) {
            let (text, start) = source.text_of(line);
            match (source.continues, out.last_mut()) {
                (Some(dropped), Some(last)) => {
                    last.push_str(&" ".repeat(dropped));
                    last.push_str(&text[start..]);
                }
                _ => out.push(text[start..].to_owned()),
            }
        }
        out
    }

    /// Bytes the two row vectors allocate, spare capacity included.
    pub fn row_bytes(&self) -> usize {
        self.lines.capacity() * size_of::<Line<'static>>()
            + self.sources.capacity() * size_of::<RowSource>()
    }

    /// Drop the last row.
    pub fn pop(&mut self) -> Option<Line<'static>> {
        self.sources.pop();
        self.lines.pop()
    }
}

/// Greedy word wrap of `content` behind `first` (row 1) and `rest` (later
/// rows). `\n` breaks; a word wider than a row is split.
/// Oversized graphemes stay intact and may be clipped by the terminal.
pub fn wrap(content: Spans, width: usize, first: Spans, rest: Spans) -> Vec<Line<'static>> {
    wrap_layout(content, width, first, rest).into_lines()
}

/// [`wrap`], with where each row sits in its source line.
pub fn wrap_layout(content: Spans, width: usize, first: Spans, rest: Spans) -> Layout {
    let mut w = Wrapper::new(width, first, rest);
    for s in content {
        let style = s.style;
        let mut token = String::new();
        let mut kind = None;
        for ch in s.content.chars() {
            let this = Token::of(ch);
            if kind.is_some_and(|k| k != this || this == Token::Newline) {
                w.push(&token, kind.unwrap_or(Token::Word), style);
                token.clear();
            }
            kind = Some(this);
            token.push(ch);
        }
        if let Some(kind) = kind {
            w.push(&token, kind, style);
        }
    }
    w.finish()
}

/// Wrap plain text in one style.
pub fn wrap_text(
    text: &str,
    style: Style,
    width: usize,
    first: Spans,
    rest: Spans,
) -> Vec<Line<'static>> {
    wrap(vec![span(text, style)], width, first, rest)
}

/// [`wrap_text`], with where each row sits in its source line.
pub fn wrap_text_layout(
    text: &str,
    style: Style,
    width: usize,
    first: Spans,
    rest: Spans,
) -> Layout {
    wrap_layout(vec![span(text, style)], width, first, rest)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Token {
    Word,
    Space,
    Newline,
}

impl Token {
    fn of(ch: char) -> Self {
        match ch {
            '\n' => Token::Newline,
            ' ' => Token::Space,
            _ => Token::Word,
        }
    }
}

struct Wrapper {
    width: usize,
    rest: Spans,
    rows: Layout,
    line: Spans,
    /// `line`'s source; its prefix is `base`.
    continues: Option<usize>,
    /// Spans of `line` that are prefix, never trimmed or merged into.
    prefix_spans: usize,
    base: usize,
    used: usize,
}

impl Wrapper {
    fn new(width: usize, first: Spans, rest: Spans) -> Self {
        // A prefix wider than the row is clipped to leave one column for text.
        let width = width.max(1);
        let first = clip(first, width - 1);
        let rest = clip(rest, width - 1);
        let base = width_of(&first);
        Self {
            width,
            prefix_spans: first.len(),
            rest,
            rows: Layout::default(),
            line: first,
            continues: None,
            base,
            used: base,
        }
    }

    fn push(&mut self, token: &str, kind: Token, style: Style) {
        match kind {
            Token::Newline => self.break_line(None),
            Token::Space => {
                let tw = display_width(token);
                if self.used == self.base {
                    // Spaces a soft break pushed to the row's start are dropped with it.
                    if let Some(dropped) = &mut self.continues {
                        *dropped += token.len();
                    }
                    return;
                }
                if self.used + tw > self.width {
                    self.soft_break(token.len());
                    return;
                }
                self.append(token, style);
            }
            Token::Word => {
                let tw = display_width(token);
                if self.used + tw > self.width && self.used > self.base {
                    self.soft_break(0);
                }
                if self.used + tw <= self.width {
                    self.append(token, style);
                    return;
                }
                for grapheme in token.graphemes(true) {
                    let cw = grapheme_width(grapheme);
                    if self.used + cw > self.width && self.used > self.base {
                        self.soft_break(0);
                    }
                    self.append(grapheme, style);
                }
            }
        }
    }

    fn append(&mut self, text: &str, style: Style) {
        self.used += display_width(text);
        if self.line.len() > self.prefix_spans
            && let Some(last) = self.line.last_mut()
            && last.style == style
        {
            last.content.to_mut().push_str(text);
            return;
        }
        self.line.push(Span::styled(text.to_owned(), style));
    }

    /// A wrap inside the source line; `dropped` spaces vanish at the break.
    fn soft_break(&mut self, dropped: usize) {
        let trimmed = self.trim_end();
        self.push_line(Some(trimmed + dropped));
    }

    /// End the source line; the next row starts with `next` as its source.
    fn break_line(&mut self, next: Option<usize>) {
        self.end_line();
        self.push_line(next);
    }

    fn push_line(&mut self, next: Option<usize>) {
        let line = std::mem::replace(&mut self.line, self.rest.clone());
        let source = RowSource {
            prefix: self.base,
            continues: std::mem::replace(&mut self.continues, next),
        };
        self.rows.push(Line::from(line), source);
        self.prefix_spans = self.rest.len();
        self.base = width_of(&self.rest);
        self.used = self.base;
    }

    /// Trim the last row of a source line. With no text of its own, the
    /// spaces dropped before it were trailing ones, which every line end trims.
    fn end_line(&mut self) {
        self.trim_end();
        if self.line.len() == self.prefix_spans
            && let Some(dropped) = &mut self.continues
        {
            *dropped = 0;
        }
    }

    /// Drop trailing spaces; how many went.
    fn trim_end(&mut self) -> usize {
        let mut trimmed = 0;
        while self.line.len() > self.prefix_spans {
            let Some(last) = self.line.last_mut() else {
                break;
            };
            let kept = last.content.trim_end_matches(' ').len();
            trimmed += last.content.len() - kept;
            if kept > 0 {
                last.content.to_mut().truncate(kept);
                break;
            }
            self.line.pop();
        }
        trimmed
    }

    fn finish(mut self) -> Layout {
        self.end_line();
        self.push_line(None);
        self.rows
    }
}

/// Keep `max` rows; the last kept row ends in an ellipsis when rows were cut.
pub fn cap_rows(
    mut rows: Vec<Line<'static>>,
    max: usize,
    width: usize,
    theme: &Theme,
) -> Vec<Line<'static>> {
    if rows.len() <= max {
        return rows;
    }
    rows.truncate(max);
    if let Some(last) = rows.pop() {
        let mut spans = last.spans;
        let style = spans.last().map(|s| s.style).unwrap_or_default();
        let ellipsis = theme.glyphs().ellipsis;
        // Force the marker even when the row had room: rows were dropped.
        let room = width.saturating_sub(ellipsis.width());
        spans = truncate(spans, room, theme);
        if spans.last().is_none_or(|s| s.content != ellipsis) {
            spans.push(Span::styled(ellipsis, style));
        }
        rows.push(Line::from(spans));
    }
    rows
}

/// [`cap_rows`] over a layout.
pub fn cap_layout(rows: Layout, max: usize, width: usize, theme: &Theme) -> Layout {
    let Layout { lines, mut sources } = rows;
    sources.truncate(max);
    Layout {
        lines: cap_rows(lines, max, width, theme),
        sources,
    }
}

/// A preview row: a line of output, or the elision marker for `n` lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewRow<'a> {
    Line(&'a str),
    Hidden(usize),
}

/// Up to 5 lines whole; longer output shows head 2, the marker, tail 3.
/// A marker standing in for one line would take that line's row, so 6 show whole too.
pub fn preview<'a>(lines: &[&'a str]) -> Vec<PreviewRow<'a>> {
    if lines.len() <= PREVIEW_MAX_ROWS + 1 {
        return lines.iter().map(|l| PreviewRow::Line(l)).collect();
    }
    let hidden = lines.len() - PREVIEW_HEAD_ROWS - PREVIEW_TAIL_ROWS;
    let mut rows: Vec<_> = lines[..PREVIEW_HEAD_ROWS]
        .iter()
        .map(|l| PreviewRow::Line(l))
        .collect();
    rows.push(PreviewRow::Hidden(hidden));
    rows.extend(
        lines[lines.len() - PREVIEW_TAIL_ROWS..]
            .iter()
            .map(|l| PreviewRow::Line(l)),
    );
    rows
}

/// Where the rest of a cut cell is; plain mode, without Ctrl+T, drops it.
pub const DETAILS_HINT: &str = " (ctrl+t to view)";

/// `… +34 lines (ctrl+t to view)`.
pub fn hidden_marker(n: usize, theme: &Theme) -> Span<'static> {
    let noun = if n == 1 { "line" } else { "lines" };
    span(
        format!("{} +{n} {noun}{DETAILS_HINT}", theme.glyphs().ellipsis),
        theme.secondary(),
    )
}

/// `    full output saved · 84 KB · /artifacts`, dim with the command in accent:
/// where an output too big to keep inline went. Details add the name.
pub fn artifact_rows(
    links: &[ArtifactLink],
    width: usize,
    detail: bool,
    theme: &Theme,
) -> Vec<Line<'static>> {
    let dot = theme.glyphs().dot;
    links
        .iter()
        .map(|link| {
            let mut text = "full output saved".to_owned();
            if let Some(name) = link.name.as_ref().filter(|_| detail) {
                text.push_str(&format!(" {dot} {}", clean(name)));
            }
            if let Some(size) = link.size_bytes {
                text.push_str(&format!(" {dot} {}", format::bytes(size)));
            }
            text.push_str(&format!(" {dot} "));
            let row = vec![
                child_prefix(theme, false),
                span(text, theme.secondary()),
                span("/artifacts", theme.accent()),
            ];
            Line::from(truncate(row, width, theme))
        })
        .collect()
}

/// Output lines without trailing blank lines.
pub fn output_lines(text: &str) -> Vec<&str> {
    let mut lines: Vec<&str> = text.lines().collect();
    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }
    lines
}

/// [`format::amount`], with `~` in front when estimated: `$0.21`, `~$1.00`.
pub fn money(cost: Cost) -> String {
    let tilde = if cost.estimated { "~" } else { "" };
    format::cost_with_basis(format!("{tilde}{}", format::amount(cost.usd)), cost.basis)
}

/// Inline text where `code` spans take the code hue (user prompts, objectives).
pub fn inline_code(text: &str, base: Style, theme: &Theme) -> Spans {
    let mut out = Vec::new();
    for (i, part) in text.split('`').enumerate() {
        if part.is_empty() {
            continue;
        }
        let style = if i % 2 == 1 { theme.code() } else { base };
        out.push(span(part, style));
    }
    out
}

/// Plain rendering of styled lines: text only, trailing spaces trimmed.
pub fn plain(lines: &[Line<'_>]) -> Vec<String> {
    lines
        .iter()
        .map(|line| {
            let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            text.trim_end().to_owned()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_of(lines: &[Line<'_>]) -> Vec<String> {
        plain(lines)
    }

    #[test]
    fn clipping_and_truncation_keep_complete_graphemes() {
        let theme = Theme::default();
        for cluster in ["👩‍💻", "❤️", "🇩🇰", "👍🏽", "e\u{301}"] {
            let text = format!("A{cluster}ZZ");
            let width = 1 + cluster.width();
            assert_eq!(
                plain(&[Line::from(truncate(
                    vec![span(&text, theme.text())],
                    width,
                    &theme
                ))]),
                ["A…"]
            );
            assert_eq!(
                plain(&[Line::from(clip(vec![span(&text, theme.text())], width - 1))]),
                ["A"]
            );
            assert_eq!(
                plain(&[Line::from(clip(vec![span(&text, theme.text())], width))]),
                [format!("A{cluster}")]
            );
        }
        assert_eq!(
            plain(&[Line::from(truncate(
                vec![span("A👩‍💻ZZ", theme.text())],
                4,
                &theme
            ))]),
            ["A👩‍💻…"]
        );
        assert!(clip(vec![span("👩‍💻", theme.text()), span("x", theme.text())], 1).is_empty());
        let spans = truncate(vec![span("❤️".repeat(6), theme.text())], 5, &theme);
        assert_eq!(plain(&[Line::from(spans)]), ["❤️❤️…"]);
    }

    #[test]
    fn overlong_cell_words_wrap_on_grapheme_boundaries() {
        let theme = Theme::default();
        assert_eq!(
            text_of(&wrap_text("a👩‍💻X", theme.text(), 3, vec![], vec![])),
            ["a👩‍💻", "X"]
        );
        assert_eq!(
            text_of(&wrap_text(
                "❤️".repeat(6).as_str(),
                theme.text(),
                4,
                vec![],
                vec![]
            )),
            ["❤️❤️", "❤️❤️", "❤️❤️"]
        );
        assert_eq!(
            text_of(&wrap_text("a👩‍💻X", theme.text(), 1, vec![], vec![])),
            ["a", "👩‍💻", "X"]
        );
    }

    #[test]
    fn wrap_breaks_on_words_and_splits_words_wider_than_a_row() {
        let theme = Theme::default();
        let lines = wrap_text(
            "alpha beta gamma abcdefghijkl",
            theme.text(),
            10,
            vec![span("• ", theme.secondary())],
            vec![span("  ", theme.text())],
        );
        assert_eq!(
            text_of(&lines),
            vec!["• alpha", "  beta", "  gamma", "  abcdefgh", "  ijkl"]
        );
        assert!(lines.iter().all(|l| l.width() <= 10));
    }

    #[test]
    fn wrap_honours_newlines_and_drops_leading_spaces_on_continuations() {
        let theme = Theme::default();
        let lines = wrap_text("a\n\nb   c", theme.text(), 4, vec![], vec![]);
        assert_eq!(text_of(&lines), vec!["a", "", "b", "c"]);
    }

    #[test]
    fn wrapped_rows_rejoin_into_their_source_lines_at_any_width() {
        let theme = Theme::default();
        let style = theme.text();
        let content = || {
            vec![
                span(
                    "alpha  beta gamma abcdefghijklmnop q\n\n  lead   and ",
                    style,
                ),
                span("  split spaces", theme.secondary()),
                span(" trailing   \nx", style),
            ]
        };
        let lead = || vec![span("> ", style)];
        let rest = || vec![span("  ", style)];
        let wide = wrap_layout(content(), 1_000, lead(), rest()).rejoined();
        assert_eq!(wide[0], "> alpha  beta gamma abcdefghijklmnop q");
        for width in 4..40 {
            let narrow = wrap_layout(content(), width, lead(), rest());
            assert_eq!(narrow.rejoined(), wide, "width {width}");
        }
    }

    #[test]
    fn truncate_ends_in_the_theme_ellipsis() {
        let theme = Theme::default();
        let spans = truncate(vec![span("abcdefgh", theme.text())], 5, &theme);
        assert_eq!(width_of(&spans), 5);
        assert_eq!(plain(&[Line::from(spans)]), vec!["abcd…"]);
    }

    #[test]
    fn preview_keeps_head_and_tail_around_the_marker() {
        let lines = ["1", "2", "3", "4", "5", "6", "7"];
        assert_eq!(
            preview(&lines),
            vec![
                PreviewRow::Line("1"),
                PreviewRow::Line("2"),
                PreviewRow::Hidden(2),
                PreviewRow::Line("5"),
                PreviewRow::Line("6"),
                PreviewRow::Line("7"),
            ]
        );
        assert_eq!(preview(&lines[..5]).len(), 5);
        assert!(
            preview(&lines[..6])
                .iter()
                .all(|row| matches!(row, PreviewRow::Line(_))),
            "a marker never hides a single line"
        );
    }

    #[test]
    fn clean_drops_escapes_and_control_characters() {
        assert_eq!(clean("\u{1b}[31mred\u{1b}[0m\tx\r\n"), "red    x\n");
    }

    #[test]
    fn clean_strips_terminal_control_strings_and_c1_sequences() {
        for sequence in [
            "\u{1b}]0;hidden title\u{7}",
            "\u{1b}]8;;https://example.invalid\u{1b}\\",
            "\u{1b}Pignored data\u{1b}\\",
            "\u{9d}0;hidden title\u{9c}",
            "\u{9b}31m",
            "\u{1b}(B",
        ] {
            assert_eq!(clean(&format!("a{sequence}b")), "ab");
        }
        for sequence in [
            "\u{1b}]unfinished",
            "\u{1b}Punfinished",
            "\u{9b}31",
            "\u{9d}unfinished",
        ] {
            assert_eq!(clean_settled(&format!("ab{sequence}")), ("ab".into(), 2));
        }
    }

    #[test]
    fn clean_settled_stops_before_an_open_escape() {
        assert_eq!(clean_settled("ab\u{1b}[3"), ("ab".into(), 2));
        assert_eq!(clean_settled("ab\u{1b}"), ("ab".into(), 2));
        assert_eq!(clean_settled("a\u{1b}[1;2\u{1b}"), ("a".into(), 1));
        assert_eq!(clean_settled("a\u{1b}[0mb"), ("ab".into(), 6));
    }

    #[test]
    fn money_marks_estimates_and_tiny_amounts() {
        let cost = |usd, estimated| Cost {
            usd,
            estimated,
            basis: Default::default(),
        };
        assert_eq!(money(cost(0.21, false)), "$0.21");
        assert_eq!(money(cost(1.0, true)), "~$1.00");
        assert_eq!(money(cost(0.0, false)), "$0");
        assert_eq!(money(cost(0.004, false)), "$0.0040");
    }
}
