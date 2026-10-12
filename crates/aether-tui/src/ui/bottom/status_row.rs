//! The status row while the viewed agent is busy (SPEC §3.3):
//! `• Working (1m 12s • esc to interrupt) · 2 agents running`. Owner: W5.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_width::UnicodeWidthStr;

use aether_core::agent::{AgentStatus, BlockReason, WaitCondition};
use aether_core::id::{AgentId, TaskId};
use aether_core::time::Timestamp;
use aether_protocol::TreeNode;

use super::agents_strip;
use super::approval::ellipsis;
use crate::app::App;
use crate::app::approval_flow::{approval_pause_ms, turn_started_at};
use crate::app::board::BoardStatus;
use crate::app::transcript::{CallStatus, CallTarget, CellModel, CompactionState, inner_command};
use crate::format;
use crate::ui::theme::tokens::Theme;

/// Shimmer: starts after 600 ms, a 1 s sweep every 4 s.
pub const SHIMMER_DELAY_MS: u64 = 600;
pub const SHIMMER_SWEEP_MS: u64 = 1_000;
pub const SHIMMER_PERIOD_MS: u64 = 4_000;
/// One full dim → bright → dim cycle of the bullet and running-agent pulses.
pub const PULSE_PERIOD_MS: u64 = 2_000;
/// Frame spacing while the band sweeps (≈30 fps), and while only pulses move.
const SWEEP_FRAME_MS: u64 = 32;
const PULSE_FRAME_MS: u64 = 100;
/// Width of the bright band, in columns.
const SHIMMER_BAND_COLS: f32 = 6.0;

/// Header text in priority order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatusHeader {
    Reasoning(String),
    Running(String),
    Waiting(Vec<String>),
    Compacting,
    Working,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusRow {
    pub header: StatusHeader,
    /// Turn elapsed, paused while an approval for this agent is open.
    pub elapsed_ms: u64,
    pub descendants_running: usize,
    /// Descendants waiting on the operator; the frame zeroes it when the
    /// needs-you strip fits, so one fact shows once.
    pub needs_you: usize,
    pub retrying_in_s: Option<u64>,
    /// How long the approval card on screen has waited; the row then reads
    /// `Waiting for your approval`, since esc answers the card.
    pub approval_wait_ms: Option<u64>,
}

/// `None` when the viewed agent is idle (the row takes no space).
pub fn model(app: &App) -> Option<StatusRow> {
    let agent = app.selected?;
    let node = app.tree.nodes.get(&agent)?;
    if !is_busy(&node.agent.status) {
        return None;
    }
    let descendants = agents_strip::descendants(&app.tree, agent);
    let descendants_running = descendants
        .iter()
        .filter(|n| n.agent.status == AgentStatus::Running)
        .count();
    let shown = app.shown_approval();
    // The card already says it; once is enough.
    let needs_you = if shown.is_some() {
        0
    } else {
        app.needs_you()
            .iter()
            .filter(|row| descendants.iter().any(|n| n.agent.id == row.agent))
            .count()
    };
    Some(StatusRow {
        header: header(app, agent, node),
        elapsed_ms: turn_elapsed_ms(app, agent),
        descendants_running,
        needs_you,
        retrying_in_s: retrying_in_s(app, node),
        approval_wait_ms: shown
            .map(|a| (app.now - a.requested_at).num_milliseconds().max(0) as u64),
    })
}

/// Next animation deadline in ms, for the frame scheduler; `None` when static.
/// Also paces the agents strip, which pulses on the same clock.
pub fn next_frame_in(app: &App) -> Option<u64> {
    let row = model(app);
    if let Some(wait) = row.as_ref().and_then(|r| r.approval_wait_ms) {
        // Only the wait clock moves; the strip hides behind the card.
        return Some(1_000 - wait % 1_000);
    }
    let strip = agents_strip::items(app);
    // Only running and waiting strip items show a clock.
    let strip_moves = strip
        .iter()
        .any(|item| matches!(item.status, BoardStatus::Running | BoardStatus::Waiting));
    if row.is_none() && !strip_moves {
        return None;
    }
    let theme = &app.theme;
    let pulsing = row.is_some() || strip.iter().any(|item| item.status == BoardStatus::Running);
    if theme.animates() && pulsing {
        let into_period = epoch_ms(app.now) % SHIMMER_PERIOD_MS;
        let shimmering = row
            .as_ref()
            .is_some_and(|r| r.elapsed_ms >= SHIMMER_DELAY_MS);
        if shimmering && into_period < SHIMMER_SWEEP_MS {
            return Some(SWEEP_FRAME_MS);
        }
        return Some(PULSE_FRAME_MS);
    }
    // Static: only the clocks move, once a second. An approval pauses the
    // turn clock, and a paused clock alone never needs another frame.
    let row_ticks = row
        .filter(|r| r.retrying_in_s.is_some() || !turn_clock_paused(app))
        .map(|r| 1_000 - r.elapsed_ms % 1_000);
    row_ticks.or(strip_moves.then_some(1_000))
}

pub fn render(frame: &mut Frame, area: Rect, row: &StatusRow, app: &App, theme: &Theme) {
    frame.render_widget(Paragraph::new(line(row, app, theme, area.width)), area);
}

/// Triangle wave in `0.0..=1.0` over `period_ms`, for pulses.
pub(crate) fn pulse_phase(now_ms: u64, period_ms: u64) -> f32 {
    let t = (now_ms % period_ms) as f32 / period_ms as f32;
    1.0 - (2.0 * t - 1.0).abs()
}

/// Wall-clock phase source for shimmer and pulses; pre-epoch clocks read 0.
pub(crate) fn epoch_ms(now: Timestamp) -> u64 {
    u64::try_from(now.timestamp_millis()).unwrap_or(0)
}

fn is_busy(status: &AgentStatus) -> bool {
    match status {
        AgentStatus::Running => true,
        // Waiting on the inbox alone is an idle chat agent.
        AgentStatus::Waiting { condition } => !matches!(condition, WaitCondition::Inbox),
        // An inspection hold waits for a person; nothing is running to interrupt.
        AgentStatus::Blocked { reason } => !matches!(reason, BlockReason::Inspection { .. }),
        _ => false,
    }
}

fn header(app: &App, agent: AgentId, node: &TreeNode) -> StatusHeader {
    let transcript = app.transcripts.get(&agent).map(|t| t.live_state());
    let active = transcript
        .and_then(|t| t.active.as_ref())
        .filter(|c| app.policy(Some(agent)).shows(&c.model))
        .map(|c| &c.model);
    if node.agent.status == AgentStatus::Running {
        if let Some(title) = transcript
            .filter(|_| {
                app.policy(Some(agent))
                    .shows_category(crate::view_modes::Category::Reasoning)
            })
            .and_then(|t| t.stream.reasoning_title.as_deref())
            .map(str::trim)
            .filter(|t| !t.is_empty())
        {
            return StatusHeader::Reasoning(title.to_owned());
        }
        if let Some(verb) = active.and_then(running_tool) {
            return StatusHeader::Running(verb);
        }
    }
    if let Some(names) = waiting_names(app, node) {
        return StatusHeader::Waiting(names);
    }
    if matches!(active, Some(CellModel::Compaction(CompactionState::Active))) {
        return StatusHeader::Compacting;
    }
    StatusHeader::Working
}

/// What a running tool cell is doing, as the words after `Running`.
fn running_tool(cell: &CellModel) -> Option<String> {
    match cell {
        CellModel::Exec {
            command,
            status: CallStatus::Running,
            ..
        } => Some(
            inner_command(command)
                .lines()
                .next()
                .unwrap_or_default()
                .trim()
                .to_owned(),
        ),
        CellModel::Called {
            target,
            status: CallStatus::Running,
            ..
        } => Some(match target {
            CallTarget::Mcp { server, tool } => format!("{server} {tool}"),
            CallTarget::WebSearch { .. } => "web search".to_owned(),
            CallTarget::WebFetch { .. } => "web fetch".to_owned(),
            CallTarget::Tool { name } => name.clone(),
        }),
        _ => None,
    }
}

/// Who a waiting or lease-blocked agent waits for; `None` when it is not waiting.
fn waiting_names(app: &App, node: &TreeNode) -> Option<Vec<String>> {
    let mut names: Vec<String> = Vec::new();
    let mut add = |name: String| {
        if !names.contains(&name) {
            names.push(name);
        }
    };
    match &node.agent.status {
        AgentStatus::Waiting { condition } if !matches!(condition, WaitCondition::Inbox) => {
            for task in &node.waiting_on {
                if let Some(name) = task_owner_name(app, *task) {
                    add(name);
                }
            }
            let mut agents = Vec::new();
            message_senders(condition, &mut agents);
            for agent in agents {
                add(app.agent_display_name(agent));
            }
        }
        AgentStatus::Blocked {
            reason: BlockReason::Resource { holder, .. },
        } => add(app.agent_display_name(*holder)),
        AgentStatus::Blocked {
            reason: BlockReason::Dependency { on },
        } => {
            if let Some(name) = task_owner_name(app, *on) {
                add(name);
            }
        }
        _ => return None,
    }
    Some(names)
}

fn task_owner_name(app: &App, task: TaskId) -> Option<String> {
    app.tree
        .nodes
        .values()
        .find(|n| n.task.as_ref().is_some_and(|t| t.id == task))
        .map(|n| app.agent_display_name(n.agent.id))
}

fn message_senders(condition: &WaitCondition, out: &mut Vec<AgentId>) {
    match condition {
        WaitCondition::MessageFrom { agent } => out.push(*agent),
        WaitCondition::Any { of }
        | WaitCondition::All { of }
        | WaitCondition::AtLeast { of, .. } => {
            for inner in of {
                message_senders(inner, out);
            }
        }
        _ => {}
    }
}

/// Whether the viewed agent's turn clock stands still: an open approval of
/// its own is banking the wait (see `approval_pause_ms`).
fn turn_clock_paused(app: &App) -> bool {
    let Some(agent) = app.selected else {
        return false;
    };
    app.approvals
        .iter()
        .any(|a| a.agent_id == agent && !app.resolving.contains(&a.id) && a.requested_at <= app.now)
}

fn turn_elapsed_ms(app: &App, agent: AgentId) -> u64 {
    let Some(started) = turn_started_at(app, agent) else {
        return 0;
    };
    let wall = (app.now - started).num_milliseconds().max(0) as u64;
    wall.saturating_sub(approval_pause_ms(app, agent, started))
}

/// Seconds until an exhausted agent's next probe.
fn retrying_in_s(app: &App, node: &TreeNode) -> Option<u64> {
    let AgentStatus::Blocked {
        reason: BlockReason::Exhausted { next_probe, .. },
    } = &node.agent.status
    else {
        return None;
    };
    let ms = (*next_probe - app.now).num_milliseconds().max(0) as u64;
    Some(ms.div_ceil(1_000))
}

fn header_text(header: &StatusHeader) -> String {
    match header {
        StatusHeader::Reasoning(title) => title.clone(),
        StatusHeader::Running(what) => format!("Running {what}"),
        StatusHeader::Waiting(names) if names.is_empty() => "Waiting".to_owned(),
        StatusHeader::Waiting(names) => format!("Waiting for {}", names.join(", ")),
        StatusHeader::Compacting => "Compacting".to_owned(),
        StatusHeader::Working => "Working".to_owned(),
    }
}

fn line(row: &StatusRow, app: &App, theme: &Theme, width: u16) -> Line<'static> {
    if let Some(wait) = row.approval_wait_ms {
        return approval_line(wait, row, theme, width);
    }
    let glyphs = theme.glyphs();
    let motion = theme.animates();
    let now = epoch_ms(app.now);
    let bullet = if motion {
        theme.pulse(pulse_phase(now, PULSE_PERIOD_MS))
    } else {
        theme.running()
    };
    // Fixed so the interrupt hint never moves; optional context follows it.
    let suffix = vec![
        Span::styled(" (", theme.secondary()),
        Span::styled(format::elapsed_compact(row.elapsed_ms), theme.secondary()),
        Span::styled(format!(" {} ", glyphs.bullet), theme.secondary()),
        Span::styled("esc", theme.title()),
        Span::styled(" to interrupt)", theme.secondary()),
    ];
    let mut context = context_groups(row, theme);
    let lead = glyphs.bullet.width() + 1;
    let header = header_text(&row.header);
    let width = usize::from(width);
    while !context.is_empty()
        && lead
            + header.width()
            + spans_width(&suffix)
            + context.iter().map(|g| spans_width(g)).sum::<usize>()
            > width
    {
        context.pop();
    }
    let used = lead + spans_width(&suffix) + context.iter().map(|g| spans_width(g)).sum::<usize>();
    let header =
        format::truncate_with_marker(&header, width.saturating_sub(used).max(1), ellipsis(theme));

    let mut spans = vec![
        Span::styled(glyphs.bullet.to_owned(), bullet),
        Span::raw(" "),
    ];
    spans.extend(header_spans(&header, row.elapsed_ms, now, motion, theme));
    spans.extend(suffix);
    spans.extend(context.into_iter().flatten());
    Line::from(spans)
}

/// `• Waiting for your approval (12s) · 2 agents running`: still, with no esc
/// hint, because esc answers the card below.
fn approval_line(wait_ms: u64, row: &StatusRow, theme: &Theme, width: u16) -> Line<'static> {
    let glyphs = theme.glyphs();
    let mut spans = vec![
        Span::styled(glyphs.bullet.to_owned(), theme.attention()),
        Span::raw(" "),
        Span::styled("Waiting for your approval", theme.text()),
        Span::styled(
            format!(" ({})", format::elapsed_compact(wait_ms)),
            theme.secondary(),
        ),
    ];
    let mut context = context_groups(row, theme);
    let width = usize::from(width);
    while !context.is_empty()
        && spans_width(&spans) + context.iter().map(|g| spans_width(g)).sum::<usize>() > width
    {
        context.pop();
    }
    spans.extend(context.into_iter().flatten());
    Line::from(spans)
}

/// ` · 2 agents running`, ` · ◆ 1 needs you`, ` · retrying in 8s`; dropped right to left.
fn context_groups(row: &StatusRow, theme: &Theme) -> Vec<Vec<Span<'static>>> {
    let glyphs = theme.glyphs();
    let sep = || Span::styled(format!(" {} ", glyphs.dot), theme.secondary());
    let mut groups = Vec::new();
    if row.descendants_running > 0 {
        let n = row.descendants_running;
        let noun = if n == 1 { "agent" } else { "agents" };
        groups.push(vec![
            sep(),
            Span::styled(format!("{n} {noun} running"), theme.secondary()),
        ]);
    }
    if row.needs_you > 0 {
        groups.push(vec![
            sep(),
            Span::styled(glyphs.needs_you.to_owned(), theme.attention()),
            Span::styled(format!(" {} needs you", row.needs_you), theme.secondary()),
        ]);
    }
    if let Some(s) = row.retrying_in_s {
        groups.push(vec![
            sep(),
            Span::styled(format!("retrying in {s}s"), theme.secondary()),
        ]);
    }
    groups
}

fn spans_width(spans: &[Span<'_>]) -> usize {
    spans.iter().map(|s| s.content.width()).sum()
}

/// The header with a band of the running hue sweeping it; static dim without motion.
fn header_spans(
    text: &str,
    elapsed_ms: u64,
    now_ms: u64,
    motion: bool,
    theme: &Theme,
) -> Vec<Span<'static>> {
    if !motion {
        return vec![Span::styled(text.to_owned(), theme.secondary())];
    }
    let into_period = now_ms % SHIMMER_PERIOD_MS;
    if elapsed_ms < SHIMMER_DELAY_MS || into_period >= SHIMMER_SWEEP_MS {
        return vec![Span::styled(text.to_owned(), theme.shimmer(0.0))];
    }
    let progress = into_period as f32 / SHIMMER_SWEEP_MS as f32;
    let chars: Vec<char> = text.chars().collect();
    let phases = band_phases(chars.len(), progress);
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut run = String::new();
    let mut run_style: Option<Style> = None;
    for (c, phase) in chars.into_iter().zip(phases) {
        let style = theme.shimmer(phase);
        if run_style.is_some_and(|s| s != style) {
            spans.push(Span::styled(
                std::mem::take(&mut run),
                run_style.unwrap_or_default(),
            ));
        }
        run_style = Some(style);
        run.push(c);
    }
    if let Some(style) = run_style {
        spans.push(Span::styled(run, style));
    }
    spans
}

/// Per-column brightness for a band at `progress` (0 → entering on the left,
/// 1 → gone on the right). Peak 1.0 at the band centre, 0.0 outside it.
fn band_phases(cols: usize, progress: f32) -> Vec<f32> {
    let half = SHIMMER_BAND_COLS / 2.0;
    let centre = -half + progress * (cols as f32 + SHIMMER_BAND_COLS);
    (0..cols)
        .map(|i| {
            let distance = (i as f32 + 0.5 - centre).abs();
            (1.0 - distance / half).clamp(0.0, 1.0)
        })
        .collect()
}

#[cfg(test)]
mod tests;
