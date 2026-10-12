//! `/approvals`: remembered approval rules, revoke, auto-approve toggle. Owner: W7.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Span;

use aether_core::approval::{ApprovalRule, RuleScope};
use aether_core::id::ApprovalRuleId;
use aether_core::tool::ApprovalDecision;
use aether_protocol::Command;

use super::picker::{Picker, PickerEvent, PickerRow};
use super::{Hint, clean, render_empty, sep, short_id, unhandled};
use crate::app::{App, Effect, Then};
use crate::commands::SlashCommand;
use crate::ui::bottom::{BottomView, ViewAction, ViewOutcome};
use crate::ui::theme::tokens::Theme;

const HINTS: &[Hint] = &[
    ("↑↓", "choose"),
    ("d", "revoke"),
    ("y", "auto-approve on/off"),
    ("esc", "back"),
];

#[derive(Debug)]
pub struct ApprovalsView {
    picker: Picker<ApprovalRuleId>,
}

impl Default for ApprovalsView {
    fn default() -> Self {
        ApprovalsView {
            picker: Picker::new("Approvals", Vec::new()),
        }
    }
}

impl ApprovalsView {
    fn picker(&self, app: &App) -> Picker<ApprovalRuleId> {
        self.picker.with_items(rows(app))
    }

    /// The daemon confirms a revoke before anything changes on screen, so a
    /// refusal surfaces as an error instead of a rule that silently stays.
    fn revoke(&self) -> ViewOutcome {
        let Some(&id) = self.picker.selected() else {
            return ViewOutcome::Act(vec![ViewAction::Flash("no rule highlighted".into())]);
        };
        ViewOutcome::Act(vec![ViewAction::Effect(Box::new(Effect::Request {
            command: Command::RevokeApprovalRule { id, reason: None },
            then: Then::RuleRevoked,
        }))])
    }
}

impl BottomView for ApprovalsView {
    fn wheel(&mut self, up: bool, app: &App) -> ViewOutcome {
        self.handle_key(super::wheel_key(up), app)
    }

    fn kind(&self) -> Option<super::ViewKind> {
        Some(super::ViewKind::Approvals)
    }

    fn handle_key(&mut self, key: KeyEvent, app: &App) -> ViewOutcome {
        self.picker = self.picker(app);
        match key.code {
            KeyCode::Char('y') => {
                return ViewOutcome::Act(vec![ViewAction::Command(SlashCommand::Yolo)]);
            }
            KeyCode::Char('d' | 'x') | KeyCode::Delete => return self.revoke(),
            // Letters are hotkeys here, so they never reach the filter.
            KeyCode::Char(c) if !c.is_ascii_digit() => return unhandled(key),
            _ => {}
        }
        match self.picker.handle_key(key) {
            PickerEvent::Cancelled => ViewOutcome::Close(Vec::new()),
            PickerEvent::Ignored => unhandled(key),
            _ => ViewOutcome::Consumed,
        }
    }

    fn desired_height(&self, width: u16, app: &App) -> u16 {
        self.picker(app).desired_height(width)
    }

    fn render(&self, frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
        let picker = self.picker(app);
        let mut title = title(app, theme);
        if picker.items.is_empty() {
            let text = format!(
                "No remembered decisions{}answer an approval with a or p to add one",
                sep(theme)
            );
            return render_empty(frame, area, title, &text, theme);
        }
        title.extend(picker.filter_spans(theme));
        picker.render_titled(frame, area, title, theme, HINTS);
    }
}

/// `Approvals  auto-approve off for worker and its workers`: whether the
/// viewed agent's subtree asks first; with no agent, what new agents will do.
fn title(app: &App, theme: &Theme) -> Vec<Span<'static>> {
    let (on, scope) = match app.selected_agent() {
        Some(agent) => (
            agent.config.approval.is_yolo(),
            format!(
                "for {} and its workers",
                clean(&app.agent_display_name(agent.id))
            ),
        ),
        None => (app.yolo_default(), "for new agents".to_owned()),
    };
    let (state, style) = if on {
        ("on", theme.accent())
    } else {
        ("off", theme.secondary())
    };
    vec![
        Span::styled("Approvals", theme.title()),
        Span::styled("  auto-approve ", theme.secondary()),
        Span::styled(state, style),
        Span::styled(format!(" {scope}"), theme.secondary()),
    ]
}

fn rows(app: &App) -> Vec<(PickerRow, ApprovalRuleId)> {
    let theme = &app.theme;
    app.approval_rules
        .iter()
        .map(|rule| (rule_row(app, rule, theme), rule.id))
        .collect()
}

fn rule_row(app: &App, rule: &ApprovalRule, theme: &Theme) -> PickerRow {
    let answer = if rule.decision == ApprovalDecision::Approved {
        "allow"
    } else {
        "refuse"
    };
    let tool = rule.matcher.tool.as_deref().unwrap_or("any tool");
    let matches = match (&rule.matcher.argument, &rule.matcher.resource_prefix) {
        (Some(argument), _) => clean(argument),
        (None, Some(prefix)) => clean(prefix),
        (None, None) => "any call".into(),
    };
    let dot = sep(theme);
    let mut detail = format!("{matches}{dot}{}", scope_label(app, &rule.scope));
    if let Some(note) = &rule.note {
        detail.push_str(&dot);
        detail.push_str(&clean(note));
    }
    PickerRow::new(format!("{answer} {}", clean(tool)), detail)
}

/// Agents by name where the tree knows them; ids only as a fallback.
fn scope_label(app: &App, scope: &RuleScope) -> String {
    let name = |id: &aether_core::id::AgentId| {
        app.tree
            .nodes
            .get(id)
            .map(|node| clean(&app.agent_display_name(node.agent.id)))
            .unwrap_or_else(|| short_id(&app.theme, id))
    };
    match scope {
        RuleScope::Everywhere => "everywhere".into(),
        RuleScope::Agent { id } => format!("only {}", name(id)),
        RuleScope::Subtree { root } => format!("{} and its workers", name(root)),
        RuleScope::Run { id } if app.selected_agent().is_some_and(|a| a.run == *id) => {
            "this run".into()
        }
        RuleScope::Run { .. } => "an earlier run".into(),
    }
}
