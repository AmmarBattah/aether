//! Cell family: question: `◆ tester asks`, the question, `└ reply below`; answered dims the ◆ to `•`. Owner: W2.
//! Handles `CellModel::Question`.

use ratatui::text::Line;

use super::text::{child_prefix, clean, inline_code, plain, span, truncate, wrap};
use super::{Tone, bullet};
use crate::app::transcript::CellModel;
use crate::ui::theme::tokens::Theme;

pub fn display_lines(cell: &CellModel, width: u16, theme: &Theme) -> Vec<Line<'static>> {
    let CellModel::Question {
        asker_name,
        text,
        state,
        ..
    } = cell
    else {
        return Vec::new();
    };
    let width = usize::from(width);
    let mark = if *state != crate::app::transcript::QuestionState::Pending {
        bullet(theme, Tone::Neutral)
    } else {
        span(
            theme.glyphs().needs_you,
            theme.attention().patch(theme.title()),
        )
    };
    let head = vec![
        mark,
        span(" ", theme.text()),
        span(clean(asker_name), theme.title()),
        span(
            if *state != crate::app::transcript::QuestionState::Pending {
                " asked"
            } else {
                " asks"
            },
            theme.text(),
        ),
    ];
    let mut rows = wrap(head, width, Vec::new(), vec![span("  ", theme.text())]);
    let indent = || vec![span("  ", theme.text())];
    let body = inline_code(&clean(text.trim()), theme.text(), theme);
    rows.extend(wrap(body, width, indent(), indent()));
    if *state != crate::app::transcript::QuestionState::Answered {
        let row = vec![
            child_prefix(theme, true),
            span(
                match state {
                    crate::app::transcript::QuestionState::Pending => "reply below",
                    crate::app::transcript::QuestionState::AskerEnded => {
                        "unanswered; this agent has ended"
                    }
                    crate::app::transcript::QuestionState::Answered => unreachable!(),
                },
                theme.secondary(),
            ),
        ];
        rows.push(Line::from(truncate(row, width, theme)));
    }
    rows
}

pub fn detail_lines(cell: &CellModel, width: u16, theme: &Theme) -> Vec<Line<'static>> {
    display_lines(cell, width, theme)
}

pub fn plain_lines(cell: &CellModel, width: u16) -> Vec<String> {
    plain(&display_lines(cell, width, &super::plain_theme()))
}
