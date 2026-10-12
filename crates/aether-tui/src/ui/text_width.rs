use ratatui::buffer::CellWidth;
use unicode_segmentation::UnicodeSegmentation;

// Ratatui lays out individual graphemes; whole-string widths can discount
// ligatures across graphemes and omit halfwidth sound marks that occupy cells.
pub(crate) fn display_width(s: &str) -> usize {
    if s.is_ascii() {
        return s.bytes().filter(|byte| !byte.is_ascii_control()).count();
    }
    s.graphemes(true).map(grapheme_width).sum()
}

pub(crate) fn grapheme_width(grapheme: &str) -> usize {
    // Span::styled_graphemes omits control-containing graphemes before rendering.
    if grapheme.contains(char::is_control) {
        0
    } else {
        usize::from(grapheme.cell_width())
    }
}
