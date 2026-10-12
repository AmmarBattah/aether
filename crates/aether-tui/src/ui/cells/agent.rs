//! Cell family: child-agent cells: `• Spawned name [role] (model effort)`, `• Waiting for …`,
//! `• name [role] finished · 1m 03s · $0.21`, `• Sent to …`. Owner: W2.
//! Handles `CellModel::{Spawned, Waiting, ChildResult, Sent}`.

use ratatui::text::Line;

use super::text::{
    Spans, cap_rows, child_prefix, clean, inline_code, money, plain, span, truncate, wrap,
};
use super::{Tone, bullet};
use crate::app::transcript::{CellModel, ChildOutcome, DescendantSummary};
use crate::format;
use crate::ui::theme::tokens::{Theme, ToolKind};

/// Rows an objective or result takes in the compact view.
const SUMMARY_ROWS: usize = 3;
/// Columns kept free on a live wait's row for ` · since 41s`.
pub const SINCE_RESERVE: usize = 14;

pub fn display_lines(cell: &CellModel, width: u16, theme: &Theme) -> Vec<Line<'static>> {
    lines(cell, usize::from(width), theme, false)
}

/// Details: whole objectives, results and errors.
pub fn detail_lines(cell: &CellModel, width: u16, theme: &Theme) -> Vec<Line<'static>> {
    lines(cell, usize::from(width), theme, true)
}

pub fn plain_lines(cell: &CellModel, width: u16) -> Vec<String> {
    plain(&display_lines(cell, width, &super::plain_theme()))
}

fn lines(cell: &CellModel, width: usize, theme: &Theme, detail: bool) -> Vec<Line<'static>> {
    let cap = |rows: Vec<Line<'static>>| {
        if detail {
            rows
        } else {
            cap_rows(rows, SUMMARY_ROWS, width, theme)
        }
    };
    match cell {
        CellModel::Spawned {
            name,
            role,
            model,
            effort,
            objective,
            ..
        } => {
            let label = match effort {
                Some(effort) => format!("({model} {effort})"),
                None => format!("({model})"),
            };
            let mut head = vec![
                bullet(theme, Tone::Neutral),
                span(" ", theme.text()),
                span("Spawned", agent_style(theme)),
                span(" ", theme.text()),
            ];
            head.extend(who(name, role, theme));
            head.push(span(" ", theme.text()));
            head.push(span(label, theme.secondary()));
            let mut rows = wrap(head, width, Vec::new(), vec![child_prefix(theme, false)]);
            if !objective.trim().is_empty() {
                rows.extend(cap(under_text(objective, theme.text(), width, theme)));
            }
            rows
        }
        CellModel::Waiting { on, since, ended } => {
            let names: Vec<&str> = on.iter().map(|(_, name)| name.as_str()).collect();
            match ended {
                Some(ended) => {
                    let waited = u64::try_from((*ended - *since).num_milliseconds()).unwrap_or(0);
                    let text = format!("Waited {} for {}", format::took(waited), names.join(", "));
                    let row = vec![
                        bullet(theme, Tone::Neutral),
                        span(" ", theme.text()),
                        span(clean(&text), theme.secondary()),
                    ];
                    wrap(row, width, Vec::new(), vec![span("  ", theme.text())])
                }
                None => {
                    let mut row = vec![
                        bullet(theme, Tone::Active),
                        span(" ", theme.text()),
                        span("Waiting for ", theme.text()),
                    ];
                    for (i, name) in names.iter().enumerate() {
                        if i > 0 {
                            row.push(span(", ", theme.secondary()));
                        }
                        row.push(span(clean(name), agent_style(theme)));
                    }
                    let room = width.saturating_sub(SINCE_RESERVE);
                    wrap(row, room, Vec::new(), vec![span("  ", theme.text())])
                }
            }
        }
        CellModel::ChildResult {
            name,
            role,
            outcome,
            elapsed_ms,
            cost_with_children,
            result,
            descendants,
            ..
        } => {
            let (tone, word, word_style) = match outcome {
                ChildOutcome::Finished => (Tone::Success, "finished", theme.success()),
                ChildOutcome::Failed { .. } => (Tone::Failure, "failed", theme.failure()),
                ChildOutcome::Cancelled => (Tone::Neutral, "cancelled", theme.paused()),
            };
            let mut head = vec![bullet(theme, tone), span(" ", theme.text())];
            head.extend(who(name, role, theme));
            head.push(span(format!(" {word}"), word_style));
            let dot = theme.glyphs().dot;
            let mut meta = Vec::new();
            if let ChildOutcome::Failed { quota: Some(q), .. } = outcome {
                meta.push(clean(q));
            }
            meta.push(format::took(*elapsed_ms));
            if let Some(cost) = cost_with_children {
                let with = if descendants.is_empty() {
                    ""
                } else {
                    " with children"
                };
                meta.push(format!("{}{with}", money(*cost)));
            }
            head.push(span(
                format!(" {dot} {}", meta.join(&format!(" {dot} "))),
                theme.secondary(),
            ));
            let mut rows = wrap(head, width, Vec::new(), vec![child_prefix(theme, false)]);
            match outcome {
                ChildOutcome::Finished if !result.trim().is_empty() => {
                    rows.extend(cap(under_text(result, theme.text(), width, theme)));
                }
                ChildOutcome::Failed { error, .. } => {
                    rows.extend(cap(under_text(error, theme.failure(), width, theme)));
                    let hint = format!("/wake {name} {dot} /restart {name}");
                    rows.push(Line::from(truncate(
                        vec![child_prefix(theme, false), span(hint, theme.secondary())],
                        width,
                        theme,
                    )));
                }
                _ => {}
            }
            for d in descendants {
                rows.push(Line::from(truncate(descendant(d, theme), width, theme)));
            }
            rows
        }
        CellModel::Sent { to, text } => {
            let mut head = vec![
                bullet(theme, Tone::Neutral),
                span(" ", theme.text()),
                span("Sent", agent_style(theme)),
                span(" to ", theme.text()),
                span(clean(to), agent_style(theme)),
            ];
            head = truncate(head, width, theme);
            let mut rows = vec![Line::from(head)];
            if !text.trim().is_empty() {
                let body = under_text(
                    if detail {
                        text
                    } else {
                        text.lines().next().unwrap_or("")
                    },
                    theme.secondary(),
                    width,
                    theme,
                );
                rows.extend(if detail {
                    body
                } else {
                    cap_rows(body, 1, width, theme)
                });
            }
            rows
        }
        _ => Vec::new(),
    }
}

/// Delegation verbs and agent names: the agent hue, bold.
fn agent_style(theme: &Theme) -> ratatui::style::Style {
    theme.tool(ToolKind::Agent).patch(theme.title())
}

/// `name [role]`: the name in the agent hue, the role dim.
fn who(name: &str, role: &str, theme: &Theme) -> Spans {
    let mut spans = vec![span(clean(name), agent_style(theme))];
    // `worker [worker]` repeats itself, and `worker 2` is still a worker.
    let base = name
        .rsplit_once(' ')
        .filter(|(_, n)| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        .map_or(name, |(base, _)| base);
    if !role.is_empty() && role != base {
        spans.push(span(format!(" [{}]", clean(role)), theme.secondary()));
    }
    spans
}

/// Text under the `└` gutter, code spans in the accent.
fn under_text(
    text: &str,
    style: ratatui::style::Style,
    width: usize,
    theme: &Theme,
) -> Vec<Line<'static>> {
    let body = inline_code(&clean(text.trim()), style, theme);
    wrap(
        body,
        width,
        vec![child_prefix(theme, true)],
        vec![child_prefix(theme, false)],
    )
}

/// `✓ tester · 1m 20s · $0.09` under a child result.
fn descendant(d: &DescendantSummary, theme: &Theme) -> Spans {
    let g = theme.glyphs();
    let glyph = match d.outcome {
        ChildOutcome::Finished => span(g.done, theme.success()),
        ChildOutcome::Failed { .. } => span(g.failed, theme.failure()),
        ChildOutcome::Cancelled => span(g.cancelled, theme.secondary()),
    };
    let mut meta = format!(" {} {}", g.dot, format::took(d.elapsed_ms));
    if let Some(cost) = d.cost.map(money) {
        meta.push_str(&format!(" {} {cost}", g.dot));
    }
    vec![
        child_prefix(theme, false),
        glyph,
        span(" ", theme.text()),
        span(clean(&d.name), theme.text()),
        span(meta, theme.secondary()),
    ]
}
