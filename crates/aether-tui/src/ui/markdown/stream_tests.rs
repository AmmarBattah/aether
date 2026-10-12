//! A stream must render exactly what [`render`] gives for the same source,
//! after every delta, however the source is cut into deltas.

use super::*;
use crate::ui::theme::ThemeSpec;

/// Replies built to trip a split in the wrong place: constructs whose kind
/// depends on the line after, partial lines that change kind as they grow,
/// and blocks that reach back across others.
const CORPUS: &[&str] = &[
    "First paragraph with enough words to wrap.\nA soft break.\n\nSecond paragraph  \nwith a hard break and a back\\\nslash break.\n",
    "# Heading\n\nText under it.\n\n#\n\n#x is not a heading\n\n## Second *level*\nParagraph right after.\n",
    "Setext title\n===\n\nSub title\n---\n\nParagraph\n---\n\n\n---\n\nAfter a rule.\n***\n___\n- - -\n",
    "- one\n- two\n- three\n\nBetween.\n\n- loose one\n\n- loose two\n\n  continued under two\n\n1. first\n2. second\n   - nested a\n   - nested b\n3. third\n\n7. starts at seven\n8. eight\n",
    "Paragraph\n1. interrupts it\n\nParagraph\n2. does not interrupt\n\nParagraph\n- a bullet interrupts\n\n* other bullet\n+ third kind\n",
    "Before the fence.\n\n```rust\nfn main() {\n\n    println!(\"hi\");\n}\n```\n\n~~~\ntilde fence\n~~~\n\n````\n```\nnested-looking fence\n```\n````\n\nAfter.\n",
    "Text\n```\nfence interrupts a paragraph\n```\nmore text\n\n    indented code\n\n    still code after a blank\n\nNot code.\n",
    "| a | b |\n|---|:-:|\n| 1 | 2 |\n| 3 | 4 |\nplain row joins the table\n\nAfter the table.\n\nParagraph\n| x | y |\n| - | - |\n| interrupting | table |\n\nDone.\n",
    "> quoted line\nlazy continuation\n\n> second quote\n> > nested\n> - list in quote\n>\n> ```\n> code in quote\n> ```\n\nOut of the quote.\n",
    "<div>\nraw html block\n</div>\n\nText after html.\n\n<!-- a comment\nover lines -->\n\n<dir>\n\nParagraph\n<dirx not a block\n\n<custom-tag>\n\nend\n",
    "See [the docs][foo] and [bar].\n\nMore text here.\n\n[foo]: https://example.com/docs \"Docs\"\n\nTail paragraph.\n\n[bar]: /bar\n",
    "Emphasis *across\nlines* and **strong\ntext**, `code`, ~~struck~~, a [link](https://x.y) and <https://auto.link>.\n\n- [ ] todo\n- [x] done\n\n![image](pic.png)\n",
    "Unicode héllo wörld — 日本語のテキスト and emoji 🚀🚀 wrapping in a long line of text.\n\n\tTabbed\tline\n\nCRLF line\r\nnext line\r\n\r\nNew paragraph\r\n",
    "Trailing blank lines\n\n\n\n\nfollowed by text\n   \n  \nand whitespace-only lines\n\n   # indented heading\n    # four spaces is code\n",
    "```\nan unclosed fence\n\nthat keeps going\n\n# not a heading\n",
    "| h1 | h2 |\n|----|----|\n| still | open",
    "Paragraph\n| x | y |\n| - | -x turns the table back into text\n\nNext.\n\nText\n| a |\n| - |\n| one column |\n",
    "Para\n\n- item\n\n  - deep\n\n    deeper paragraph\n\nend of list\n\n1) paren list\n2) two\n\n10. wide marker\n11. next\n",
];

fn assert_matches_full(stream: &mut MarkdownStream, width: usize, theme: &Theme, context: &str) {
    let base = theme.text();
    let streamed: Vec<Line<'static>> = stream.lines(width, base, theme).iter().cloned().collect();
    let full = render(stream.source(), width, base, theme);
    assert_eq!(
        streamed,
        full,
        "{context}: width {width}, source {:?}",
        stream.source()
    );
}

/// Feed `source` in the given pieces, checking after each one, and that the
/// settled lines only ever grow within a generation.
fn stream_in(pieces: &[&str], width: usize, theme: &Theme, context: &str) -> MarkdownStream {
    let mut stream = MarkdownStream::new();
    let mut before: Option<(u64, Vec<Line<'static>>)> = None;
    for piece in pieces {
        stream.push(piece);
        assert_matches_full(&mut stream, width, theme, context);
        let generation = stream.lines(width, theme.text(), theme).generation();
        if let Some((was, settled)) = &before
            && *was == generation
        {
            assert!(
                stream.cache.settled.starts_with(settled),
                "{context}: settled lines changed without a new generation"
            );
        }
        before = Some((generation, stream.cache.settled.clone()));
    }
    stream
}

fn chars(text: &str) -> Vec<&str> {
    text.char_indices()
        .map(|(i, c)| &text[i..i + c.len_utf8()])
        .collect()
}

#[test]
fn every_prefix_of_every_reply_renders_as_a_full_render() {
    let theme = Theme::default();
    for (n, doc) in CORPUS.iter().enumerate() {
        for width in [12, 37, 80] {
            stream_in(&chars(doc), width, &theme, &format!("corpus {n}"));
        }
    }
}

/// xorshift64: deterministic chunkings without a dependency.
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % n as u64) as usize
    }
}

fn random_pieces<'a>(text: &'a str, rng: &mut Rng) -> Vec<&'a str> {
    let mut pieces = Vec::new();
    let mut at = 0;
    while at < text.len() {
        let mut end = (at + 1 + rng.below(40)).min(text.len());
        while !text.is_char_boundary(end) {
            end += 1;
        }
        pieces.push(&text[at..end]);
        at = end;
    }
    pieces
}

#[test]
fn random_chunkings_of_a_long_reply_render_as_a_full_render() {
    let theme = Theme::default();
    let reply = CORPUS.join("\n");
    for seed in 1..=12u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let pieces = random_pieces(&reply, &mut rng);
        stream_in(&pieces, 30 + rng.below(70), &theme, &format!("seed {seed}"));
    }
}

#[test]
fn a_new_width_or_theme_renders_afresh() {
    let reply = CORPUS[3];
    let mut stream = MarkdownStream::new();
    stream.push(reply);
    let theme = Theme::default();
    let first = stream.lines(60, theme.text(), &theme).generation();
    assert_matches_full(&mut stream, 60, &theme, "first width");
    assert_matches_full(&mut stream, 24, &theme, "narrower");
    let ascii = Theme::resolve(ThemeSpec {
        ascii: true,
        ..ThemeSpec::default()
    });
    assert_matches_full(&mut stream, 24, &ascii, "ascii theme");
    assert_ne!(
        stream.lines(24, ascii.text(), &ascii).generation(),
        first,
        "a new key restarts the settled lines"
    );
}

#[test]
fn a_long_reply_settles_all_but_its_last_blocks() {
    let theme = Theme::default();
    let block =
        "A paragraph of streamed text that goes on for a while.\n\n- a list item\n- another\n\n";
    let mut stream = MarkdownStream::new();
    while stream.source().len() < 50_000 {
        for piece in chars(block) {
            stream.push(piece);
            stream.lines(80, theme.text(), &theme);
        }
    }
    assert_matches_full(&mut stream, 80, &theme, "50k reply");
    let unsettled = stream.source().len() - stream.cache.committed;
    assert!(
        unsettled <= block.len(),
        "{unsettled} bytes are re-parsed per delta"
    );
}

#[test]
fn a_reference_definition_renders_the_reply_whole() {
    let theme = Theme::default();
    let reply =
        "Use [the guide].\n\nMore.\n\nAnd more.\n\n[the guide]: https://example.com\n\nAfter.\n";
    let stream = stream_in(&chars(reply), 60, &theme, "late definition");
    assert!(
        stream.cache.whole,
        "the definition switched to whole renders"
    );
}

#[test]
fn an_unchanged_source_is_not_rendered_again() {
    let theme = Theme::default();
    let mut stream = MarkdownStream::new();
    stream.push("Some text.\n\nMore text.\n");
    stream.lines(40, theme.text(), &theme);
    let rendered = stream.cache.rendered;
    stream.cache.tail.clear();
    stream.lines(40, theme.text(), &theme);
    assert_eq!(stream.cache.rendered, rendered);
    assert!(stream.cache.tail.is_empty(), "the cached lines were reused");
}

#[test]
fn an_escape_split_across_deltas_renders_as_if_it_arrived_whole() {
    let theme = Theme::default();
    let mut stream = MarkdownStream::new();
    stream.push("a\x1b[3");
    stream.push("1mb");
    let streamed: Vec<Line<'static>> = stream
        .lines(40, theme.text(), &theme)
        .iter()
        .cloned()
        .collect();
    assert_eq!(
        streamed,
        crate::ui::markdown::render("a\x1b[31mb", 40, theme.text(), &theme)
    );
    assert_eq!(stream.source(), "ab");
}

/// Lines that open, continue, interrupt or close blocks, for random replies.
const LINES: &[&str] = &[
    "",
    "",
    "plain words here",
    "more words",
    "# heading",
    "#",
    "#x",
    "---",
    "===",
    "***",
    "- item",
    "* other",
    "1. first",
    "2. second",
    "  - nested",
    "   three spaces",
    "    four spaces",
    "```",
    "```rust",
    "~~~",
    "> quote",
    "> - quoted item",
    ">",
    "| a | b |",
    "| - | - |",
    "|---|---|",
    "| - | -x",
    "a | b",
    "<div>",
    "</div>",
    "<!-- note",
    "-->",
    "text with *emph",
    "close* and `code`",
    "trailing  ",
    "\ttab",
];

#[test]
fn random_replies_stream_as_they_render() {
    let theme = Theme::default();
    for seed in 1..=300u64 {
        let mut rng = Rng(seed.wrapping_mul(0x2545_F491_4F6C_DD1D));
        let lines: Vec<&str> = (0..24).map(|_| LINES[rng.below(LINES.len())]).collect();
        let reply = lines.join("\n");
        stream_in(
            &chars(&reply),
            24 + rng.below(40),
            &theme,
            &format!("seed {seed}"),
        );
    }
}
