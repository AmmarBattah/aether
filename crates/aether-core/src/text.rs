//! Run content on its way to a terminal or a file.
//!
//! Model text, tool input, tool output and error strings all reach a screen
//! through the CLI and the report. Before run import that content came from
//! the operator's own agents; it now comes from whatever file someone was
//! sent, which makes an escape sequence in a replay frame or a report line
//! fully attacker-chosen. The TUI is safe by its toolkit - ratatui filters
//! graphemes carrying control characters - and the CLI and the report have
//! no equivalent, so they strip before they write.

use std::borrow::Cow;

/// Text with its control characters removed: C0, DEL, the C1 range - which
/// is where ANSI and OSC introducers live - and the bidirectional overrides
/// and isolates, which reorder what a reader sees without any escape at all.
/// Tabs become spaces and newlines stay, because a document is made of them.
///
/// `--json` output is not run through this: `serde_json` escapes C0 but
/// leaves C1 and the bidi marks as literal bytes, so a `--json` consumer that
/// prints a field to a terminal has to strip it itself.
pub fn strip_controls(text: &str) -> Cow<'_, str> {
    if !text.chars().any(is_unsafe) {
        return Cow::Borrowed(text);
    }
    Cow::Owned(
        text.chars()
            .filter_map(|c| match c {
                '\t' => Some(' '),
                '\n' => Some('\n'),
                c if is_unsafe(c) => None,
                c => Some(c),
            })
            .collect(),
    )
}

fn is_unsafe(c: char) -> bool {
    (c.is_control() && c != '\n')
        || matches!(c, '\u{80}'..='\u{9f}')
        // LRE..RLO and LRI..PDI: Cf, so `is_control` does not see them.
        || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
}

#[cfg(test)]
mod tests {
    use super::strip_controls;

    #[test]
    fn plain_text_is_borrowed_untouched() {
        assert!(matches!(
            strip_controls("nothing to do here"),
            std::borrow::Cow::Borrowed(_)
        ));
        assert_eq!(strip_controls("a\nb"), "a\nb", "a document keeps its lines");
    }

    #[test]
    fn an_escape_sequence_cannot_reach_the_terminal() {
        // Clear the screen, move the cursor home, then set a title.
        let hostile = "before\u{1b}[2J\u{1b}[H\u{1b}]0;owned\u{7}after";
        let clean = strip_controls(hostile);
        assert!(!clean.contains('\u{1b}'), "{clean}");
        assert!(!clean.contains('\u{7}'), "{clean}");
        assert_eq!(clean, "before[2J[H]0;ownedafter");
    }

    #[test]
    fn the_c1_range_goes_too() {
        // A single-byte CSI: the same power, without an escape byte to spot.
        let clean = strip_controls("a\u{9b}2Jb\u{0}c");
        assert_eq!(clean, "a2Jbc");
    }

    /// The same class of attack: text that reads as something other than
    /// what it says, with no escape byte in it.
    #[test]
    fn a_bidirectional_override_cannot_reorder_what_is_read() {
        let hostile = "rm -rf \u{202e}gnitset/ ~\u{202c} # safe";
        let clean = strip_controls(hostile);
        assert!(!clean.contains('\u{202e}'), "{clean}");
        assert!(!clean.contains('\u{202c}'), "{clean}");
        assert_eq!(strip_controls("a\u{2066}b\u{2069}c"), "abc");
    }

    #[test]
    fn a_tab_becomes_a_space_rather_than_moving_the_cursor() {
        assert_eq!(strip_controls("a\tb"), "a b");
    }
}
