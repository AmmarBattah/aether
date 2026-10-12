//! One visual layout for composer height, painting and cursor navigation.
//! Rows wrap at word boundaries; a word wider than the row breaks mid-word.

use crate::ui::text_width::{display_width, grapheme_width};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InputRow {
    pub text: String,
    /// `(draft offset, column)` at the row start and after each grapheme.
    boundaries: Vec<(usize, usize)>,
    /// Byte length of `text` after each grapheme (parallel to `boundaries[1..]`).
    text_ends: Vec<usize>,
}

impl InputRow {
    fn new(offset: usize) -> Self {
        Self {
            text: String::new(),
            boundaries: vec![(offset, 0)],
            text_ends: Vec::new(),
        }
    }

    /// Whitespace past `width` hangs: it keeps its offsets but paints nothing.
    fn push(&mut self, cell: &Cell, width: usize) {
        let mut column = self.width() + cell.cells;
        if column > width {
            column = width;
        } else {
            self.text.push_str(cell.display);
        }
        self.boundaries.push((cell.offset + cell.len, column));
        self.text_ends.push(self.text.len());
    }

    /// Each grapheme's draft offset with what it paints (empty when hanging).
    pub fn pieces(&self) -> impl Iterator<Item = (usize, &str)> {
        let starts = std::iter::once(0).chain(self.text_ends.iter().copied());
        self.boundaries
            .iter()
            .zip(starts.zip(self.text_ends.iter().copied()))
            .map(|((offset, _), (from, to))| (*offset, &self.text[from..to]))
    }

    fn width(&self) -> usize {
        self.boundaries.last().map_or(0, |(_, column)| *column)
    }

    pub fn selected_range(&self, start: usize, end: usize) -> std::ops::Range<usize> {
        let range = self.range();
        let from = self
            .boundaries
            .windows(2)
            .find(|pair| pair[1].1 > start)
            .map_or(range.end, |pair| pair[0].0);
        let to = if end >= self.width() {
            range.end
        } else {
            self.boundaries
                .windows(2)
                .rfind(|pair| pair[0].1 < end)
                .map_or(from, |pair| pair[1].0)
        };
        from..to.max(from)
    }

    /// The draft bytes this row shows, without its trailing newline.
    pub fn range(&self) -> std::ops::Range<usize> {
        let start = self.boundaries[0].0;
        let end = self.boundaries.last().map_or(start, |(offset, _)| *offset);
        start..end
    }
}

pub(crate) struct InputLayout {
    pub rows: Vec<InputRow>,
    pub cursor_row: usize,
    pub cursor_column: usize,
}

/// One grapheme as painted.
struct Cell<'a> {
    offset: usize,
    len: usize,
    display: &'a str,
    cells: usize,
    space: bool,
}

impl InputLayout {
    #[cfg(test)]
    pub fn new(text: &str, cursor: usize, inner_width: usize) -> Self {
        Self::with_ascii(text, cursor, inner_width, false)
    }

    pub fn with_ascii(text: &str, cursor: usize, inner_width: usize, ascii: bool) -> Self {
        // Keep a cell for the insertion cursor at a full row's end.
        let width = inner_width.saturating_sub(1).max(1);
        let mut rows = Vec::new();
        let mut line = Vec::new();
        let mut line_start = 0;
        for (offset, grapheme) in text.grapheme_indices(true) {
            if matches!(grapheme, "\n" | "\r\n" | "\r") {
                wrap_line(&line, line_start, width, &mut rows);
                line.clear();
                line_start = offset + grapheme.len();
                continue;
            }
            line.push(cell(offset, grapheme, width, ascii));
        }
        wrap_line(&line, line_start, width, &mut rows);
        let (cursor_row, cursor_column) = cursor_position(&rows, cursor);
        Self {
            rows,
            cursor_row,
            cursor_column,
        }
    }

    pub fn move_vertical(&self, direction: isize) -> Option<usize> {
        let target = self.cursor_row.checked_add_signed(direction)?;
        let next_start = self.rows.get(target + 1).map(|row| row.boundaries[0].0);
        self.rows
            .get(target)?
            .boundaries
            .iter()
            .rev()
            .find(|(offset, column)| *column <= self.cursor_column && Some(*offset) != next_start)
            .map(|(offset, _)| *offset)
    }
}

/// `(row, column)` of byte offset `cursor` in `rows` (non-empty). At a soft
/// wrap the cursor belongs to the start of the next row.
pub(crate) fn cursor_position(rows: &[InputRow], cursor: usize) -> (usize, usize) {
    let row = rows
        .iter()
        .rposition(|row| row.boundaries[0].0 <= cursor)
        .unwrap_or(0);
    let column = rows[row]
        .boundaries
        .iter()
        .rev()
        .find(|(offset, _)| *offset <= cursor)
        .map_or(0, |(_, column)| *column);
    (row, column)
}

/// Greedy word wrap of one logical line starting at byte `start`.
fn wrap_line(cells: &[Cell], start: usize, width: usize, rows: &mut Vec<InputRow>) {
    if cells.is_empty() {
        rows.push(InputRow::new(start));
        return;
    }
    let mut row_start = 0;
    while row_start < cells.len() {
        let end = row_start + row_end(&cells[row_start..], width);
        let mut row = InputRow::new(cells[row_start].offset);
        cells[row_start..end]
            .iter()
            .for_each(|c| row.push(c, width));
        rows.push(row);
        row_start = end;
    }
}

/// How many of `cells` (non-empty) go on the next row. Whitespace never
/// forces a break, so a row never starts with the space that ended a word.
fn row_end(cells: &[Cell], width: usize) -> usize {
    let mut column = 0;
    // Index just past the last whitespace run that is followed by a word.
    let mut last_break = None;
    for (i, c) in cells.iter().enumerate() {
        if c.space {
            column += c.cells;
            continue;
        }
        if i > 0 && cells[i - 1].space {
            last_break = Some(i);
        }
        if column > 0 && column + c.cells > width {
            return last_break.unwrap_or(i);
        }
        column += c.cells;
    }
    cells.len()
}

fn cell(offset: usize, grapheme: &str, width: usize, ascii: bool) -> Cell<'_> {
    let display = if grapheme == "\t" {
        &"    "[..width.min(4)]
    } else if grapheme_width(grapheme) > width || grapheme.chars().any(char::is_control) {
        if ascii { "?" } else { "�" }
    } else {
        grapheme
    };
    Cell {
        offset,
        len: grapheme.len(),
        display,
        cells: display_width(display),
        space: grapheme.chars().all(char::is_whitespace),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_without_changing_text_and_keeps_a_cursor_cell() {
        let layout = InputLayout::new("abcdefghijk", 11, 6);
        assert_eq!(
            layout
                .rows
                .iter()
                .map(|row| row.text.as_str())
                .collect::<Vec<_>>(),
            ["abcde", "fghij", "k"]
        );
        assert_eq!((layout.cursor_row, layout.cursor_column), (2, 1));
        assert_eq!(layout.move_vertical(-1), Some(6));
    }

    #[test]
    fn preserves_blank_lines_wide_graphemes_and_combining_sequences() {
        let text = "界e\u{301}👩‍💻\n\nend\n";
        let layout = InputLayout::new(text, text.len(), 5);
        assert_eq!(
            layout
                .rows
                .iter()
                .map(|row| row.text.as_str())
                .collect::<Vec<_>>(),
            ["界e\u{301}", "👩‍💻", "", "end", ""]
        );
        assert_eq!((layout.cursor_row, layout.cursor_column), (4, 0));
        for offset in text.char_indices().map(|(offset, _)| offset) {
            let layout = InputLayout::new(text, offset, 5);
            assert!(layout.cursor_column < 5);
        }
    }

    #[test]
    fn moving_up_at_a_full_row_end_does_not_stay_on_the_same_row() {
        let layout = InputLayout::new("abcdefghij", 10, 6);
        let cursor = layout.move_vertical(-1).expect("previous visual row");
        assert_eq!(InputLayout::new("abcdefghij", cursor, 6).cursor_row, 0);
    }

    #[test]
    fn narrow_input_and_tabs_stay_inside_the_available_width() {
        let layout = InputLayout::new("界\tX", 5, 2);
        assert_eq!(
            layout
                .rows
                .iter()
                .map(|row| row.text.as_str())
                .collect::<Vec<_>>(),
            ["�", "X"]
        );
        assert!(layout.rows.iter().all(|row| display_width(&row.text) <= 1));
    }

    #[test]
    fn composer_width_and_cursor_match_ratatui_graphemes() {
        let layout = InputLayout::new("ｶﾞX", "ｶﾞ".len(), 4);
        assert_eq!(layout.cursor_column, 2);
        let layout = InputLayout::new("ｶﾞX", "ｶﾞX".len(), 3);
        assert_eq!(rows(&layout), ["ｶﾞ", "X"]);
        assert_eq!((layout.cursor_row, layout.cursor_column), (1, 1));
    }

    fn rows(layout: &InputLayout) -> Vec<&str> {
        layout.rows.iter().map(|row| row.text.as_str()).collect()
    }

    #[test]
    fn wraps_at_word_boundaries_and_never_splits_a_fitting_word() {
        let text = "steer the agent towards tests";
        let layout = InputLayout::new(text, text.len(), 13);
        assert_eq!(rows(&layout), ["steer the ", "agent ", "towards ", "tests"]);
        let ranges: Vec<_> = layout.rows.iter().map(InputRow::range).collect();
        assert_eq!(ranges, [0..10, 10..16, 16..24, 24..29]);
        assert_eq!((layout.cursor_row, layout.cursor_column), (3, 5));
    }

    #[test]
    fn a_word_wider_than_the_row_breaks_mid_word() {
        let layout = InputLayout::new("ab abcdefghij", 0, 6);
        assert_eq!(rows(&layout), ["ab ", "abcde", "fghij"]);
    }

    #[test]
    fn whitespace_hangs_at_the_edge_without_starting_a_row() {
        let text = "abcd   efg";
        let layout = InputLayout::new(text, 6, 6);
        assert_eq!(rows(&layout), ["abcd ", "efg"]);
        assert_eq!(layout.rows[0].range(), 0..7);
        // The cursor inside the hanging run stays on the row's last cell.
        assert_eq!((layout.cursor_row, layout.cursor_column), (0, 5));
    }

    #[test]
    fn hard_newlines_and_crlf_split_rows() {
        let text = "one\r\ntwo\nthree";
        let layout = InputLayout::new(text, 0, 20);
        assert_eq!(rows(&layout), ["one", "two", "three"]);
        let pieces: Vec<_> = layout.rows[1].pieces().collect();
        assert_eq!(pieces, [(5, "t"), (6, "w"), (7, "o")]);
        let ranges: Vec<_> = layout.rows.iter().map(InputRow::range).collect();
        assert_eq!(ranges, [0..3, 5..8, 9..14]);
    }
}
