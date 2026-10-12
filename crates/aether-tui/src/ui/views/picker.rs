//! Generic filterable list used by every slash view: ↑/↓ or 1–9 to choose,
//! Enter to use, Esc back, typing filters. Owner: W7.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};

use unicode_width::UnicodeWidthStr;

use super::Hint;
use crate::format;
use crate::ui::theme::tokens::Theme;

/// Most rows a picker shows at once; the window scrolls to keep the cursor in view.
pub const MAX_VISIBLE_ROWS: usize = 10;
const PAGE: usize = MAX_VISIBLE_ROWS - 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerRow {
    pub label: String,
    /// Dim text after the label (description, status).
    pub detail: String,
    /// Shown but not choosable (e.g. a provider that is signed out).
    pub disabled: bool,
}

impl PickerRow {
    pub fn new(label: impl Into<String>, detail: impl Into<String>) -> Self {
        PickerRow {
            label: label.into(),
            detail: detail.into(),
            disabled: false,
        }
    }

    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }
}

/// What one key did to the picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerEvent {
    Ignored,
    Moved,
    Filtered,
    /// Enter (or a digit) on the item at this index into `items`.
    Chosen(usize),
    Cancelled,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Picker<T> {
    pub title: String,
    pub items: Vec<(PickerRow, T)>,
    pub filter: String,
    /// Index into the filtered rows.
    pub cursor: usize,
    /// Labels are identifiers (model ids): they keep their width before the
    /// details do, and lose their middle rather than their distinguishing end.
    pub label_first: bool,
}

impl<T> Picker<T> {
    pub fn new(title: impl Into<String>, items: Vec<(PickerRow, T)>) -> Self {
        Picker {
            title: title.into(),
            items,
            filter: String::new(),
            cursor: 0,
            label_first: false,
        }
    }

    /// See [`Picker::label_first`].
    pub fn label_first(mut self) -> Self {
        self.label_first = true;
        self
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> PickerEvent {
        let chord = key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
        let len = self.visible().len();
        let last = len.saturating_sub(1);
        match key.code {
            _ if chord => PickerEvent::Ignored,
            KeyCode::Up => self.move_to(self.cursor.saturating_sub(1)),
            KeyCode::Down => self.move_to(self.cursor.saturating_add(1).min(last)),
            KeyCode::PageUp => self.move_to(self.cursor.saturating_sub(PAGE)),
            KeyCode::PageDown => self.move_to(self.cursor.saturating_add(PAGE).min(last)),
            KeyCode::Home => self.move_to(0),
            KeyCode::End => self.move_to(last),
            KeyCode::Enter => self.choose(self.cursor),
            // Digits pick by number only while nothing is typed; after that they filter.
            KeyCode::Char(c @ '1'..='9') if self.filter.is_empty() => {
                let index = usize::from(c as u8 - b'1');
                if index >= len {
                    return PickerEvent::Ignored;
                }
                self.cursor = index;
                self.choose(index)
            }
            KeyCode::Esc if !self.filter.is_empty() => {
                self.filter.clear();
                self.cursor = 0;
                PickerEvent::Filtered
            }
            KeyCode::Esc => PickerEvent::Cancelled,
            KeyCode::Backspace if !self.filter.is_empty() => {
                self.filter.pop();
                self.cursor = 0;
                PickerEvent::Filtered
            }
            KeyCode::Char(c) => {
                self.filter.push(c);
                self.cursor = 0;
                PickerEvent::Filtered
            }
            _ => PickerEvent::Ignored,
        }
    }

    /// Indices into `items` that match the filter, in display order.
    pub fn visible(&self) -> Vec<usize> {
        let needle = self.filter.to_lowercase();
        self.items
            .iter()
            .enumerate()
            .filter(|(_, (row, _))| {
                format::contains_fold(&row.label, &needle)
                    || format::contains_fold(&row.detail, &needle)
            })
            .map(|(index, _)| index)
            .collect()
    }

    pub fn selected(&self) -> Option<&T> {
        let index = *self.visible().get(self.cursor)?;
        self.items.get(index).map(|(_, value)| value)
    }

    pub fn selected_row(&self) -> Option<&PickerRow> {
        let index = *self.visible().get(self.cursor)?;
        self.items.get(index).map(|(row, _)| row)
    }

    /// Framed height: the visible rows, capped, at least one (for "no matches").
    /// Rows are cut to one line, so the width does not change the height.
    pub fn desired_height(&self, width: u16) -> u16 {
        let _ = width;
        super::frame_height(self.body_len())
    }

    /// Body rows the list wants: the matches, capped, at least one.
    pub fn body_len(&self) -> usize {
        self.visible().len().clamp(1, MAX_VISIBLE_ROWS)
    }

    /// Framed without hints, for callers whose keys show in the global hint row.
    pub fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        self.render_hinted(frame, area, theme, &[]);
    }

    /// Framed, with `hints` on the frame's own hint row.
    pub fn render_hinted(&self, frame: &mut Frame, area: Rect, theme: &Theme, hints: &[Hint]) {
        self.render_titled(frame, area, self.title_spans(theme), theme, hints);
    }

    /// [`Self::render_hinted`] under a title the view built, e.g. one with meta.
    pub fn render_titled(
        &self,
        frame: &mut Frame,
        area: Rect,
        title: Vec<Span<'static>>,
        theme: &Theme,
        hints: &[Hint],
    ) {
        let room = super::body_room(area).min(MAX_VISIBLE_ROWS);
        // Digits choose only the first nine, so longer lists go unnumbered.
        let numbered = self.items.len() <= 9;
        let body = self.lines(area.width, room, theme, true, numbered);
        super::render_frame(frame, area, title, body, hints, theme);
    }

    /// The title, then the filter once something is typed.
    pub fn title_spans(&self, theme: &Theme) -> Vec<Span<'static>> {
        let mut spans = vec![Span::styled(self.title.clone(), theme.title())];
        spans.extend(self.filter_spans(theme));
        spans
    }

    /// `  filter abc`, or nothing while nothing is typed.
    pub fn filter_spans(&self, theme: &Theme) -> Vec<Span<'static>> {
        if self.filter.is_empty() {
            return Vec::new();
        }
        vec![
            Span::styled("  filter ", theme.secondary()),
            Span::styled(self.filter.clone(), theme.accent()),
        ]
    }

    /// The rows that fit in `room` (a `↓ N more` cue included), labels in one
    /// column. Unfocused, the cursor row keeps a dim marker but no weight.
    pub fn lines(
        &self,
        width: u16,
        room: usize,
        theme: &Theme,
        focused: bool,
        numbered: bool,
    ) -> Vec<Line<'static>> {
        let visible = self.visible();
        if visible.is_empty() {
            let text = if self.items.is_empty() {
                "nothing to show"
            } else {
                "no matches"
            };
            return vec![super::row(text, theme.secondary())];
        }
        let rows: Vec<&PickerRow> = visible.iter().map(|&i| &self.items[i].0).collect();
        let lead = 2 + if numbered { 3 } else { 0 };
        let columns = usize::from(width).saturating_sub(lead + 2);
        let label_width = if self.label_first {
            let longest = rows.iter().map(|r| r.label.width()).max().unwrap_or(0);
            longest.min(columns)
        } else {
            label_column(&rows, columns)
        };
        let lines = rows
            .iter()
            .enumerate()
            .map(|(offset, &row)| {
                let cut = (self.label_first && row.label.width() > label_width).then(|| {
                    let ellipsis = theme.glyphs().ellipsis.chars().next().unwrap_or('~');
                    let label =
                        format::truncate_middle_with_marker(&row.label, label_width, ellipsis);
                    PickerRow {
                        label,
                        ..row.clone()
                    }
                });
                let row = cut.as_ref().unwrap_or(row);
                let state = match (offset == self.cursor, focused) {
                    (false, _) => RowState::Plain,
                    (true, true) => RowState::Selected,
                    (true, false) => RowState::Marked,
                };
                let number = numbered.then_some(offset);
                picker_line(theme, row, number, label_width, state)
            })
            .collect();
        super::Window::following(rows.len(), room, self.cursor).lines(lines, theme)
    }

    /// Replace the items from fresh app data, keeping the cursor on the same
    /// value when it is still listed.
    pub fn with_items(&self, items: Vec<(PickerRow, T)>) -> Picker<T>
    where
        T: PartialEq,
    {
        let held = self.selected();
        let mut next = Picker {
            title: self.title.clone(),
            items,
            filter: self.filter.clone(),
            cursor: 0,
            label_first: self.label_first,
        };
        let visible = next.visible();
        next.cursor = held
            .and_then(|held| visible.iter().position(|&i| next.items[i].1 == *held))
            .unwrap_or(self.cursor.min(visible.len().saturating_sub(1)));
        next
    }

    fn move_to(&mut self, cursor: usize) -> PickerEvent {
        self.cursor = cursor;
        PickerEvent::Moved
    }

    fn choose(&self, cursor: usize) -> PickerEvent {
        let visible = self.visible();
        match visible.get(cursor) {
            Some(&index) if !self.items[index].0.disabled => PickerEvent::Chosen(index),
            _ => PickerEvent::Ignored,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowState {
    Plain,
    /// The cursor of a list that does not have focus: a dim marker only.
    Marked,
    Selected,
}

/// Label column width: the longest label, but never so wide that the
/// details lose their room, nor narrower than half of `room`.
pub fn label_column(rows: &[&PickerRow], room: usize) -> usize {
    let longest = rows.iter().map(|r| r.label.width()).max().unwrap_or(0);
    let detail = rows.iter().map(|r| r.detail.width()).max().unwrap_or(0);
    let floor = room / 2;
    longest.min(room.saturating_sub(detail + 2).max(floor))
}

/// One row: `› 1. label  detail`, selected as accent `›` plus bold label;
/// the label padded to `label_width` so details line up.
pub fn picker_line(
    theme: &Theme,
    row: &PickerRow,
    number: Option<usize>,
    label_width: usize,
    state: RowState,
) -> Line<'static> {
    let prompt = theme.glyphs().prompt;
    let marker = match state {
        RowState::Plain => Span::raw("  "),
        RowState::Marked => Span::styled(format!("{prompt} "), theme.secondary()),
        RowState::Selected => {
            Span::styled(format!("{prompt} "), theme.accent().patch(theme.title()))
        }
    };
    let number = match number {
        Some(n) if n < 9 => format!("{}. ", n + 1),
        Some(_) => "   ".to_owned(),
        None => String::new(),
    };
    let label_style = if row.disabled {
        theme.secondary()
    } else if state == RowState::Selected {
        theme.text().patch(theme.title())
    } else {
        theme.text()
    };
    let ellipsis = theme.glyphs().ellipsis.chars().next().unwrap_or('~');
    let label = format::truncate_with_marker(&row.label, label_width.max(1), ellipsis);
    let mut spans = vec![
        marker,
        Span::styled(number, theme.secondary()),
        Span::styled(label.clone(), label_style),
    ];
    if !row.detail.is_empty() {
        let pad = label_width.saturating_sub(label.width()) + 2;
        spans.push(Span::raw(" ".repeat(pad)));
        spans.push(Span::styled(row.detail.clone(), theme.secondary()));
    }
    Line::from(spans)
}

/// Cut a styled line to `width` columns, ending with the theme's ellipsis.
pub fn clip_line(line: Line<'static>, width: u16, theme: &Theme) -> Line<'static> {
    let width = usize::from(width);
    if line.width() <= width {
        return line;
    }
    let marker = theme.glyphs().ellipsis.chars().next().unwrap_or('~');
    let mut left = width;
    let mut spans = Vec::new();
    for span in line.spans {
        if left == 0 {
            break;
        }
        let span_width = span.width();
        if span_width < left {
            left -= span_width;
            spans.push(span);
            continue;
        }
        let text = format::truncate_with_marker(&span.content, left, marker);
        spans.push(Span::styled(text, span.style));
        left = 0;
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::tokens::ThemeSpec;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn picker() -> Picker<&'static str> {
        Picker::new(
            "Pick one",
            vec![
                (PickerRow::new("alpha", "first"), "a"),
                (PickerRow::new("beta", "second"), "b"),
                (PickerRow::new("gamma", "third").disabled(), "g"),
                (PickerRow::new("delta", "fourth"), "d"),
            ],
        )
    }

    #[test]
    fn arrows_move_and_clamp() {
        let mut p = picker();
        assert_eq!(p.handle_key(key(KeyCode::Up)), PickerEvent::Moved);
        assert_eq!(p.cursor, 0);
        for _ in 0..10 {
            p.handle_key(key(KeyCode::Down));
        }
        assert_eq!(p.cursor, 3);
        assert_eq!(p.selected(), Some(&"d"));
        p.handle_key(key(KeyCode::Home));
        assert_eq!(p.selected(), Some(&"a"));
        p.handle_key(key(KeyCode::End));
        assert_eq!(p.selected(), Some(&"d"));
    }

    #[test]
    fn enter_and_digits_choose_enabled_rows_only() {
        let mut p = picker();
        assert_eq!(p.handle_key(key(KeyCode::Enter)), PickerEvent::Chosen(0));
        assert_eq!(
            p.handle_key(key(KeyCode::Char('2'))),
            PickerEvent::Chosen(1)
        );
        assert_eq!(p.cursor, 1);
        assert_eq!(p.handle_key(key(KeyCode::Char('3'))), PickerEvent::Ignored);
        assert_eq!(p.handle_key(key(KeyCode::Char('9'))), PickerEvent::Ignored);
        assert_eq!(
            p.handle_key(key(KeyCode::Char('4'))),
            PickerEvent::Chosen(3)
        );
    }

    #[test]
    fn typing_filters_and_esc_clears_before_cancelling() {
        let mut p = picker();
        assert_eq!(p.handle_key(key(KeyCode::Char('E'))), PickerEvent::Filtered);
        assert_eq!(p.visible(), vec![1, 3]);
        // A digit after a filter is filter text, not a choice.
        assert_eq!(p.handle_key(key(KeyCode::Char('1'))), PickerEvent::Filtered);
        assert!(p.visible().is_empty());
        assert_eq!(p.selected(), None);
        assert_eq!(p.handle_key(key(KeyCode::Enter)), PickerEvent::Ignored);
        assert_eq!(p.handle_key(key(KeyCode::Backspace)), PickerEvent::Filtered);
        assert_eq!(p.handle_key(key(KeyCode::Down)), PickerEvent::Moved);
        assert_eq!(p.selected(), Some(&"d"));
        assert_eq!(p.handle_key(key(KeyCode::Esc)), PickerEvent::Filtered);
        assert_eq!(p.visible().len(), 4);
        assert_eq!(p.handle_key(key(KeyCode::Esc)), PickerEvent::Cancelled);
    }

    #[test]
    fn filter_matches_detail_text_too() {
        let mut p = picker();
        for c in "four".chars() {
            p.handle_key(key(KeyCode::Char(c)));
        }
        assert_eq!(p.visible(), vec![3]);
    }

    #[test]
    fn chords_are_not_typed() {
        let mut p = picker();
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(p.handle_key(ctrl_c), PickerEvent::Ignored);
        assert!(p.filter.is_empty());
    }

    #[test]
    fn with_items_keeps_the_selected_value() {
        let mut p = picker();
        p.handle_key(key(KeyCode::Down));
        let reloaded = p.with_items(vec![
            (PickerRow::new("zeta", ""), "z"),
            (PickerRow::new("alpha", ""), "a"),
            (PickerRow::new("beta", ""), "b"),
        ]);
        assert_eq!(reloaded.selected(), Some(&"b"));
        let shrunk = p.with_items(vec![(PickerRow::new("zeta", ""), "z")]);
        assert_eq!(shrunk.selected(), Some(&"z"));
    }

    #[test]
    fn height_is_frame_plus_capped_rows() {
        // Six rows of frame: padding, title, gap, gap, hints, padding.
        let mut p = picker();
        assert_eq!(p.desired_height(80), 6 + 4);
        p.filter = "nothing".into();
        assert_eq!(p.desired_height(80), 6 + 1);
        let many: Vec<_> = (0..40)
            .map(|i| (PickerRow::new(format!("row {i}"), ""), i))
            .collect();
        assert_eq!(Picker::new("t", many).desired_height(80), 6 + 10);
    }

    fn draw(p: &Picker<&'static str>, theme: &Theme, width: u16) -> Terminal<TestBackend> {
        let height = p.desired_height(width);
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
        terminal
            .draw(|frame| p.render_hinted(frame, frame.area(), theme, &[("esc", "back")]))
            .expect("draw");
        terminal
    }

    #[test]
    fn snapshot_marks_the_selected_row_with_theme_tokens() {
        for ascii in [false, true] {
            let theme = Theme::resolve(ThemeSpec {
                ascii,
                ..Default::default()
            });
            for width in [80, 120] {
                let mut p = picker();
                p.handle_key(key(KeyCode::Down));
                let terminal = draw(&p, &theme, width);
                let text = crate::ui::tests_support::screen_text(&terminal);
                let rows: Vec<&str> = text.lines().collect();
                assert_eq!(rows[1].trim_end(), "  Pick one");
                let prompt = theme.glyphs().prompt;
                assert_eq!(rows[3].trim_end(), "  1. alpha  first");
                assert_eq!(rows[4].trim_end(), format!("{prompt} 2. beta   second"));
                assert_eq!(rows[8].trim_end(), "  esc back");
                let buffer = terminal.backend().buffer();
                assert_eq!(buffer[(2, 1)].modifier, theme.title().add_modifier);
                // The approval card's selection: accent `›`, the label bold.
                let marker = theme.accent().patch(theme.title());
                assert_eq!(buffer[(0, 4)].fg, marker.fg.unwrap_or_default());
                assert!(buffer[(0, 4)].modifier.contains(marker.add_modifier));
                assert!(buffer[(5, 4)].modifier.contains(theme.title().add_modifier));
                assert!(!buffer[(5, 3)].modifier.contains(theme.title().add_modifier));
                // No row fill: the far edge of the cursor row is plain.
                let edge = &buffer[(width - 1, 4)];
                assert_eq!(edge.bg, theme.tint().bg.unwrap_or_default());
                assert!(
                    !edge.modifier.contains(theme.selected().add_modifier)
                        || theme.selected().add_modifier.is_empty()
                );
                // Disabled rows are secondary.
                let gamma = rows[5].find("gamma").expect("gamma row") as u16;
                assert!(
                    buffer[(gamma, 5)]
                        .modifier
                        .contains(theme.secondary().add_modifier)
                );
            }
        }
    }

    #[test]
    fn long_rows_are_cut_with_the_theme_ellipsis() {
        for ascii in [false, true] {
            let theme = Theme::resolve(ThemeSpec {
                ascii,
                ..Default::default()
            });
            let p = Picker::new("t", vec![(PickerRow::new("x".repeat(200), "detail"), "x")]);
            let terminal = draw(&p, &theme, 80);
            let text = crate::ui::tests_support::screen_text(&terminal);
            let row = text.lines().nth(3).expect("row").trim_end();
            // The label gives way so its detail still shows, inside the margin.
            let cut = format!("x{}  detail", theme.glyphs().ellipsis);
            assert!(row.ends_with(&cut), "{row}");
            assert!(row.width() <= 78, "{row}");
        }
    }
}
