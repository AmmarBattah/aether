use super::super::{AgentLine, ApprovalLine, Ended, OpenLoop, RunOutcome, TaskLine};
use super::{AgentAcc, TaskAcc, ToolAcc};
use crate::{
    agent::Scope,
    id::{AgentId, TaskId, ToolCallId},
    model::Usage,
    time::Timestamp,
};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn workspace_of(scope: &Scope) -> Option<String> {
    scope
        .workspace_root
        .as_ref()
        .map(|p| p.display().to_string())
}

pub(super) fn kind_label(kind: &crate::artifact::ArtifactKind) -> String {
    use crate::artifact::ArtifactKind as K;
    match kind {
        K::File => "file".into(),
        K::CommandOutput => "command_output".into(),
        K::Diff => "diff".into(),
        K::Commit => "commit".into(),
        K::Result => "result".into(),
        K::Other { label } => label.clone(),
    }
}

/// The first terminal event wins: an agent that failed and was then cancelled
/// on the way down failed, and the report says so.
pub(super) fn end_agent(
    agents: &mut BTreeMap<AgentId, AgentAcc>,
    agent: Option<AgentId>,
    outcome: Ended,
    at: Timestamp,
) {
    let Some(id) = agent else { return };
    let acc = agents.entry(id).or_default();
    if acc.outcome.is_terminal() {
        return;
    }
    acc.outcome = outcome;
    acc.ended_at = Some(at);
}

pub(super) fn end_task(
    tasks: &mut BTreeMap<TaskId, TaskAcc>,
    task: Option<TaskId>,
    outcome: Ended,
    at: Timestamp,
) {
    let Some(id) = task else { return };
    let acc = tasks.entry(id).or_default();
    if acc.outcome.is_terminal() {
        return;
    }
    acc.outcome = outcome;
    acc.ended_at = Some(at);
}

pub(super) fn tool_of<'a>(
    tools: &'a mut BTreeMap<String, ToolAcc>,
    call_tool: &BTreeMap<ToolCallId, String>,
    call: ToolCallId,
) -> Option<&'a mut ToolAcc> {
    let name = call_tool.get(&call)?;
    tools.get_mut(name)
}

/// A failure line for a call whose answer was flagged as an error. Only the
/// first line: the answer can be a whole command's output.
pub(super) fn answered_with_error(tool: &str, content: &str) -> String {
    const MAX_CHARS: usize = 200;
    let line = content
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("no detail");
    let mut excerpt: String = line.chars().take(MAX_CHARS).collect();
    if excerpt.len() < line.len() {
        excerpt.push('…');
    }
    format!("{tool} answered with an error: {excerpt}")
}

/// Depth follows known parent links, counting a missing parent as one boundary
/// edge. Walk rooted components once; cyclic components retain the historical
/// depth cap rather than traversing every agent's ancestor chain separately.
pub(super) fn depths_of(agents: &BTreeMap<AgentId, AgentAcc>) -> BTreeMap<AgentId, u32> {
    let cap = u32::try_from(agents.len()).unwrap_or(u32::MAX);
    let mut depths: BTreeMap<_, _> = agents.keys().map(|id| (*id, cap)).collect();
    let mut children: BTreeMap<AgentId, Vec<AgentId>> = BTreeMap::new();
    let mut stack = Vec::new();
    for (id, agent) in agents {
        match agent.parent {
            Some(parent) if agents.contains_key(&parent) => {
                children.entry(parent).or_default().push(*id);
            }
            Some(_) => stack.push((*id, 1u32)),
            None => stack.push((*id, 0u32)),
        }
    }
    // With one parent per agent, no cycle is reachable from a root.
    while let Some((id, depth)) = stack.pop() {
        depths.insert(id, depth);
        stack.extend(
            children
                .get(&id)
                .into_iter()
                .flatten()
                .map(|child| (*child, depth.saturating_add(1))),
        );
    }
    depths
}

/// Each agent's usage plus every descendant's, folded child-into-parent in
/// one pass. The tree comes out of a file the daemon did not write and is
/// capped in bytes but not in depth, so an ancestor walk per agent - which
/// is what this used to be - turns a deep forged chain into a hang on the
/// first report of it.
pub(super) fn subtree_usage_of(agents: &BTreeMap<AgentId, AgentAcc>) -> BTreeMap<AgentId, Usage> {
    let parent_of = |id: &AgentId| {
        agents
            .get(id)
            .and_then(|a| a.parent)
            .filter(|p| agents.contains_key(p))
    };
    let mut children: BTreeMap<AgentId, Vec<AgentId>> = BTreeMap::new();
    let mut stack: Vec<AgentId> = Vec::new();
    for id in agents.keys() {
        match parent_of(id) {
            Some(parent) => children.entry(parent).or_default().push(*id),
            None => stack.push(*id),
        }
    }
    // Parents before children, so folding the reverse gives every child a
    // finished total before its parent reads it. An agent in a cycle in the
    // stored links is never reached from a root and keeps its own usage,
    // which is the same refusal to trust a cycle as before.
    let mut order: Vec<AgentId> = Vec::with_capacity(agents.len());
    let mut seen: BTreeSet<AgentId> = BTreeSet::new();
    while let Some(id) = stack.pop() {
        if !seen.insert(id) {
            continue;
        }
        order.push(id);
        stack.extend(children.get(&id).into_iter().flatten().copied());
    }
    let mut totals: BTreeMap<AgentId, Usage> =
        agents.iter().map(|(id, acc)| (*id, acc.usage)).collect();
    for id in order.into_iter().rev() {
        let Some(parent) = parent_of(&id) else {
            continue;
        };
        let total = totals[&id];
        if let Some(up) = totals.get_mut(&parent) {
            *up += total;
        }
    }
    totals
}

pub(super) fn outcome_of(agents: &[AgentLine], root: Option<AgentId>) -> RunOutcome {
    if agents.is_empty() {
        return RunOutcome::Running;
    }
    if agents.iter().any(|a| !a.outcome.is_terminal()) {
        return RunOutcome::Running;
    }
    // The root's verdict is the run's verdict; without a root in the log, any
    // failure below is the run's failure.
    let verdict = root
        .and_then(|id| agents.iter().find(|a| a.id == id))
        .map(|a| &a.outcome);
    match verdict {
        Some(Ended::Failed { .. }) => RunOutcome::Failed,
        Some(Ended::Cancelled { .. }) => RunOutcome::Cancelled,
        Some(Ended::Completed { .. }) => RunOutcome::Completed,
        _ if agents
            .iter()
            .any(|a| matches!(a.outcome, Ended::Failed { .. })) =>
        {
            RunOutcome::Failed
        }
        _ if agents
            .iter()
            .any(|a| matches!(a.outcome, Ended::Cancelled { .. })) =>
        {
            RunOutcome::Cancelled
        }
        _ => RunOutcome::Completed,
    }
}

pub(super) fn open_loops_of(
    agents: &[AgentLine],
    tasks: &[TaskLine],
    approvals: &[ApprovalLine],
) -> Vec<OpenLoop> {
    let mut loops = Vec::new();
    for agent in agents.iter().filter(|a| !a.outcome.is_terminal()) {
        loops.push(OpenLoop::Agent {
            id: agent.id,
            name: agent.name.clone(),
        });
    }
    for task in tasks.iter().filter(|t| !t.outcome.is_terminal()) {
        loops.push(OpenLoop::Task {
            id: task.id,
            objective: task.objective.clone(),
        });
    }
    for approval in approvals.iter().filter(|a| a.decision.is_none()) {
        loops.push(OpenLoop::Approval {
            id: approval.id,
            tool: approval.tool.clone(),
            description: approval.description.clone(),
        });
    }
    loops
}
