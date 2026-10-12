use super::{
    AgentLine, ApprovalLine, ArtifactLine, Ended, FailureLine, Milestone, Provenance,
    REPORT_SCHEMA, RunBody, RunOutcome, RunReport, TaskLine, ToolLine, ToolTally,
};
use crate::{
    agent::Role,
    event::{Event, EventKind},
    id::{AgentId, ApprovalId, RunId, TaskId, ToolCallId},
    model::{ModelRef, Usage},
    task::Priority,
    time::Timestamp,
};
use std::collections::BTreeMap;

mod fold;
mod timeline;
use fold::{
    answered_with_error, depths_of, end_agent, end_task, kind_label, open_loops_of, outcome_of,
    subtree_usage_of, tool_of, workspace_of,
};
use timeline::milestone_of;

/// Everything one agent contributes, accumulated as the log is walked.
///
/// `order` is the position the agent was first met at. Ids are minted per
/// store, so sorting the document by id would put the same run in a
/// different order elsewhere; first-appearance is the run's own order.
#[derive(Debug, Default)]
struct AgentAcc {
    order: u32,
    parent: Option<AgentId>,
    name: String,
    role: Option<Role>,
    model: Option<ModelRef>,
    workspace: Option<String>,
    turns: u32,
    tool_calls: u32,
    usage: Usage,
    outcome: Ended,
    created_at: Option<Timestamp>,
    ended_at: Option<Timestamp>,
}

#[derive(Debug, Default)]
struct TaskAcc {
    order: u32,
    objective: String,
    owner: Option<AgentId>,
    parent_task: Option<TaskId>,
    dependencies: Vec<TaskId>,
    priority: Priority,
    outcome: Ended,
    created_at: Option<Timestamp>,
    ended_at: Option<Timestamp>,
}

#[derive(Debug, Default)]
struct ToolAcc {
    tally: ToolTally,
    duration_ms: u64,
}

/// Fold a run's events into its report. `events` must be the run's events in
/// ascending sequence order; anything belonging to another run is ignored so
/// a caller cannot silently mix two runs into one document.
pub fn build(run: RunId, events: &[Event], generated_at: Timestamp) -> RunReport {
    let mut builder = Builder::new(run);
    for event in events {
        builder.push(event);
    }
    builder.finish(generated_at)
}

/// The same fold, one event at a time, so a daemon can page a long log
/// through it without ever holding a whole run in memory.
#[derive(Debug)]
pub struct Builder {
    run: RunId,
    agents: BTreeMap<AgentId, AgentAcc>,
    tasks: BTreeMap<TaskId, TaskAcc>,
    tools: BTreeMap<String, ToolAcc>,
    /// Append-ordered, with an index so a resolution finds its request.
    approvals: Vec<ApprovalLine>,
    approval_at: BTreeMap<ApprovalId, usize>,
    artifacts: Vec<ArtifactLine>,
    failures: Vec<FailureLine>,
    timeline: Vec<Milestone>,
    /// Tool name per call id: only `tool.requested` carries the name, and the
    /// outcome events that follow name only the call.
    call_tool: BTreeMap<ToolCallId, String>,
    provenance: Option<Provenance>,
    first_root: Option<AgentId>,
    counted: u64,
    from_seq: u64,
    to_seq: u64,
    started_at: Option<Timestamp>,
    last_at: Option<Timestamp>,
}

impl Builder {
    pub fn new(run: RunId) -> Self {
        Self {
            run,
            agents: BTreeMap::new(),
            tasks: BTreeMap::new(),
            tools: BTreeMap::new(),
            approvals: Vec::new(),
            approval_at: BTreeMap::new(),
            artifacts: Vec::new(),
            failures: Vec::new(),
            timeline: Vec::new(),
            call_tool: BTreeMap::new(),
            provenance: None,
            first_root: None,
            counted: 0,
            from_seq: 0,
            to_seq: 0,
            started_at: None,
            last_at: None,
        }
    }

    /// Take the next event of the run, in ascending sequence order.
    pub fn push(&mut self, event: &Event) {
        if event.run != Some(self.run) {
            return;
        }
        // Being imported is not something the run did: it is header
        // material, so the body of the report is the same either side of an
        // export.
        if let EventKind::RunImported {
            source_run,
            source_instance,
            exported_at,
            ..
        } = &event.kind
        {
            self.provenance = Some(Provenance {
                source_run: *source_run,
                source_instance: source_instance.clone(),
                exported_at: *exported_at,
            });
            return;
        }
        self.counted += 1;
        if self.from_seq == 0 {
            self.from_seq = event.seq;
            self.started_at = Some(event.at);
        }
        self.to_seq = event.seq;
        self.last_at = Some(event.at);
        if let Some(milestone) = milestone_of(event) {
            self.timeline.push(milestone);
        }
        let agent = event.agent_id;
        match &event.kind {
            EventKind::AgentCreated {
                role,
                name,
                parent,
                model,
                scope,
                ..
            } => {
                let Some(id) = agent else { return };
                if parent.is_none() && self.first_root.is_none() {
                    self.first_root = Some(id);
                }
                let next = self.agents.len() as u32;
                let acc = self.agents.entry(id).or_default();
                acc.order = next;
                acc.parent = *parent;
                acc.name = name.clone();
                acc.role = Some(role.clone());
                acc.model = Some(model.clone());
                acc.workspace = workspace_of(scope);
                acc.created_at = Some(event.at);
            }
            EventKind::AgentStarted { turn } => {
                if let Some(id) = agent {
                    let acc = self.agents.entry(id).or_default();
                    acc.turns = acc.turns.max(*turn);
                }
            }
            EventKind::AgentCompleted { summary } => {
                end_agent(
                    &mut self.agents,
                    agent,
                    Ended::Completed {
                        summary: summary.clone(),
                    },
                    event.at,
                );
            }
            EventKind::AgentFailed { error } => {
                end_agent(
                    &mut self.agents,
                    agent,
                    Ended::Failed {
                        error: error.clone(),
                    },
                    event.at,
                );
            }
            EventKind::AgentCancelled { reason } => {
                end_agent(
                    &mut self.agents,
                    agent,
                    Ended::Cancelled {
                        reason: reason.clone(),
                    },
                    event.at,
                );
            }
            EventKind::AgentUsageUpdated { total, .. } => {
                if let Some(id) = agent {
                    self.agents.entry(id).or_default().usage = *total;
                }
            }
            EventKind::InvocationCompleted { usage, .. } => {
                if let Some(id) = agent {
                    self.agents.entry(id).or_default().usage += *usage;
                }
            }
            EventKind::TaskCreated {
                objective,
                parent_task,
                dependencies,
                priority,
            } => {
                let Some(id) = event.task_id else { return };
                let next = self.tasks.len() as u32;
                let acc = self.tasks.entry(id).or_default();
                acc.order = next;
                acc.objective = objective.clone();
                acc.parent_task = *parent_task;
                acc.dependencies = dependencies.clone();
                acc.priority = *priority;
                acc.created_at = Some(event.at);
            }
            EventKind::TaskAssigned { owner } => {
                if let Some(id) = event.task_id {
                    self.tasks.entry(id).or_default().owner = Some(*owner);
                }
            }
            EventKind::TaskCompleted { result } => {
                end_task(
                    &mut self.tasks,
                    event.task_id,
                    Ended::Completed {
                        summary: Some(result.summary.clone()),
                    },
                    event.at,
                );
            }
            EventKind::TaskFailed { error, .. } => {
                end_task(
                    &mut self.tasks,
                    event.task_id,
                    Ended::Failed {
                        error: error.clone(),
                    },
                    event.at,
                );
            }
            EventKind::TaskCancelled { reason } => {
                end_task(
                    &mut self.tasks,
                    event.task_id,
                    Ended::Cancelled {
                        reason: reason.clone(),
                    },
                    event.at,
                );
            }
            EventKind::ToolRequested { call, .. } => {
                self.call_tool.insert(call.id, call.name.clone());
                self.tools
                    .entry(call.name.clone())
                    .or_default()
                    .tally
                    .requested += 1;
                if let Some(id) = agent {
                    self.agents.entry(id).or_default().tool_calls += 1;
                }
            }
            // The call ran and answered, but an answer flagged as an error
            // (a failing command, an MCP server's error) is a failed call.
            EventKind::ToolSucceeded { call, outcome } => {
                if let Some(acc) = tool_of(&mut self.tools, &self.call_tool, *call) {
                    if outcome.is_error {
                        acc.tally.failed += 1;
                    } else {
                        acc.tally.succeeded += 1;
                    }
                    acc.duration_ms = acc.duration_ms.saturating_add(outcome.duration_ms);
                }
                if outcome.is_error {
                    let tool = self.call_tool.get(call).map_or("a tool", String::as_str);
                    self.failures.push(FailureLine {
                        at: event.at,
                        kind: event.kind.type_name(),
                        agent,
                        error: answered_with_error(tool, &outcome.content),
                    });
                }
            }
            EventKind::ToolFailed {
                call,
                error,
                duration_ms,
                ..
            } => {
                if let Some(acc) = tool_of(&mut self.tools, &self.call_tool, *call) {
                    acc.tally.failed += 1;
                    if let Some(ms) = duration_ms {
                        acc.duration_ms = acc.duration_ms.saturating_add(*ms);
                    }
                }
                self.failures.push(FailureLine {
                    at: event.at,
                    kind: event.kind.type_name(),
                    agent,
                    error: error.clone(),
                });
            }
            EventKind::ToolDenied { call, reason } => {
                if let Some(acc) = tool_of(&mut self.tools, &self.call_tool, *call) {
                    acc.tally.denied += 1;
                }
                self.failures.push(FailureLine {
                    at: event.at,
                    kind: event.kind.type_name(),
                    agent,
                    error: reason.clone(),
                });
            }
            EventKind::ToolCancelled { call } => {
                if let Some(acc) = tool_of(&mut self.tools, &self.call_tool, *call) {
                    acc.tally.cancelled += 1;
                }
            }
            EventKind::ToolReplayed { call, .. } => {
                if let Some(acc) = tool_of(&mut self.tools, &self.call_tool, *call) {
                    acc.tally.replayed += 1;
                }
            }
            EventKind::ToolRetried { call, .. } => {
                if let Some(acc) = tool_of(&mut self.tools, &self.call_tool, *call) {
                    acc.tally.retries += 1;
                }
            }
            EventKind::ApprovalRequested {
                approval,
                tool,
                description,
                ..
            } => {
                self.approval_at.insert(*approval, self.approvals.len());
                self.approvals.push(ApprovalLine {
                    id: *approval,
                    tool: tool.clone(),
                    description: description.clone(),
                    requested_at: event.at,
                    decision: None,
                    decided_by: None,
                    decided_at: None,
                });
            }
            EventKind::ApprovalResolved {
                approval,
                decision,
                by,
                ..
            } => {
                if let Some(line) = self
                    .approval_at
                    .get(approval)
                    .and_then(|i| self.approvals.get_mut(*i))
                {
                    line.decision = Some(*decision);
                    line.decided_by = Some(by.original().clone());
                    line.decided_at = Some(event.at);
                }
            }
            EventKind::ArtifactCreated { artifact } => {
                self.artifacts.push(ArtifactLine {
                    id: artifact.id,
                    name: artifact.name.clone(),
                    kind: kind_label(&artifact.kind),
                    summary: artifact.summary.clone(),
                    size_bytes: artifact.size_bytes,
                    agent: artifact.agent_id,
                    task: artifact.task_id,
                    created_at: artifact.created_at,
                    evicted: artifact.evicted_at.is_some(),
                });
            }
            EventKind::ArtifactEvicted { artifact, .. } => {
                if let Some(line) = self.artifacts.iter_mut().find(|a| a.id == *artifact) {
                    line.evicted = true;
                }
            }
            EventKind::InvocationFailed {
                error, cancelled, ..
            } if !cancelled => {
                self.failures.push(FailureLine {
                    at: event.at,
                    kind: event.kind.type_name(),
                    agent,
                    error: error.clone(),
                });
            }
            EventKind::CompactionFailed { error, .. }
            | EventKind::McpServerFailed { error, .. } => {
                self.failures.push(FailureLine {
                    at: event.at,
                    kind: event.kind.type_name(),
                    agent,
                    error: error.clone(),
                });
            }
            EventKind::BudgetExceeded { reason, .. } => {
                self.failures.push(FailureLine {
                    at: event.at,
                    kind: event.kind.type_name(),
                    agent,
                    error: reason.clone(),
                });
            }
            _ => {}
        }
    }

    /// Close the fold into the document.
    pub fn finish(self, generated_at: Timestamp) -> RunReport {
        let started_at = self.started_at.unwrap_or(generated_at);
        let depths = depths_of(&self.agents);
        let subtrees = subtree_usage_of(&self.agents);
        let mut ordered_agents: Vec<(&AgentId, &AgentAcc)> = self.agents.iter().collect();
        ordered_agents.sort_by_key(|(_, acc)| acc.order);
        let agent_lines: Vec<AgentLine> = ordered_agents
            .into_iter()
            .map(|(id, acc)| AgentLine {
                id: *id,
                parent: acc.parent,
                name: acc.name.clone(),
                role: acc.role.clone().unwrap_or_else(|| Role::new("unknown")),
                model: acc
                    .model
                    .clone()
                    .unwrap_or_else(|| ModelRef::new("unknown", "unknown")),
                depth: depths.get(id).copied().unwrap_or(0),
                turns: acc.turns,
                tool_calls: acc.tool_calls,
                usage: acc.usage,
                subtree_usage: subtrees.get(id).copied().unwrap_or_default(),
                outcome: acc.outcome.clone(),
                created_at: acc.created_at.unwrap_or(started_at),
                ended_at: acc.ended_at,
                workspace: acc.workspace.clone(),
            })
            .collect();

        let mut usage = Usage::default();
        for line in &agent_lines {
            usage += line.usage;
        }

        let mut ordered_tasks: Vec<(&TaskId, &TaskAcc)> = self.tasks.iter().collect();
        ordered_tasks.sort_by_key(|(_, acc)| acc.order);
        let task_lines: Vec<TaskLine> = ordered_tasks
            .into_iter()
            .map(|(id, acc)| TaskLine {
                id: *id,
                objective: acc.objective.clone(),
                owner: acc.owner,
                parent_task: acc.parent_task,
                dependencies: acc.dependencies.clone(),
                priority: acc.priority,
                outcome: acc.outcome.clone(),
                created_at: acc.created_at.unwrap_or(started_at),
                ended_at: acc.ended_at,
            })
            .collect();

        let tool_lines: Vec<ToolLine> = self
            .tools
            .into_iter()
            .map(|(tool, acc)| ToolLine {
                tool,
                tally: acc.tally,
                duration_ms: acc.duration_ms,
            })
            .collect();

        let open_loops = open_loops_of(&agent_lines, &task_lines, &self.approvals);
        let outcome = outcome_of(&agent_lines, self.first_root);
        let ended_at = (outcome != RunOutcome::Running)
            .then(|| agent_lines.iter().filter_map(|a| a.ended_at).max())
            .flatten();
        // Clamped here rather than in the renderer, so `--json` and the
        // human form agree: an import copies each event's `at` verbatim and
        // the first and last are by sequence, not by clock.
        let duration_ms = (self.last_at.unwrap_or(started_at) - started_at)
            .num_milliseconds()
            .max(0);

        RunReport {
            schema: REPORT_SCHEMA,
            run: self.run,
            generated_at,
            from_seq: self.from_seq,
            to_seq: self.to_seq,
            provenance: self.provenance,
            body: RunBody {
                objective: self
                    .first_root
                    .and_then(|root| task_lines.iter().find(|t| t.owner == Some(root)))
                    .map(|t| t.objective.clone()),
                outcome,
                started_at,
                ended_at,
                duration_ms,
                usage,
                events: self.counted,
                agents: agent_lines,
                tasks: task_lines,
                tools: tool_lines,
                approvals: self.approvals,
                failures: self.failures,
                artifacts: self.artifacts,
                open_loops,
                timeline: self.timeline,
            },
        }
    }
}
