use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    id::{AgentId, ApprovalId, TaskId},
    time::Timestamp,
};

/// What a waiting agent is waiting for. A typed expression the scheduler
/// evaluates; the model never polls. Leaves are single facts, and `Any`,
/// `All` and `AtLeast` combine them, so "wait for two of these five workers
/// or a deadline" is a value, not a prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "on", rename_all = "snake_case")]
pub enum WaitCondition {
    /// The task reached a terminal state.
    Task {
        id: TaskId,
    },
    Approval {
        id: ApprovalId,
    },
    MessageFrom {
        agent: AgentId,
    },
    /// Any inbox item at all.
    Inbox,
    Deadline {
        at: Timestamp,
    },
    Any {
        of: Vec<WaitCondition>,
    },
    All {
        of: Vec<WaitCondition>,
    },
    AtLeast {
        n: usize,
        of: Vec<WaitCondition>,
    },
}

/// Facts the scheduler supplies when evaluating a [`WaitCondition`].
pub trait WaitOracle {
    fn task_is_terminal(&self, task: TaskId) -> bool;
    fn approval_is_resolved(&self, approval: ApprovalId) -> bool;
    fn has_unconsumed_inbox(&self) -> bool;
    fn has_unconsumed_message_from(&self, agent: AgentId) -> bool;
    fn now(&self) -> Timestamp;
}

impl WaitCondition {
    /// Wait for every listed task.
    pub fn all_tasks(ids: impl IntoIterator<Item = TaskId>) -> Self {
        Self::All {
            of: ids.into_iter().map(|id| Self::Task { id }).collect(),
        }
    }

    /// Wait for any one of the listed tasks.
    pub fn any_task(ids: impl IntoIterator<Item = TaskId>) -> Self {
        Self::Any {
            of: ids.into_iter().map(|id| Self::Task { id }).collect(),
        }
    }

    /// Bound an open-ended wait: wake at `at` even if nothing else happens.
    /// Idle inbox waits and waits that already carry a deadline are unchanged.
    pub fn with_deadline(self, at: Timestamp) -> Self {
        if matches!(self, Self::Inbox)
            || self.involves_approval()
            || self.earliest_deadline().is_some()
        {
            return self;
        }
        Self::Any {
            of: vec![self, Self::Deadline { at }],
        }
    }

    /// Whether a person's decision is part of the wait. Those are never
    /// bounded or interrupted: the answer takes as long as it takes.
    pub fn involves_approval(&self) -> bool {
        match self {
            Self::Approval { .. } => true,
            Self::Any { of } | Self::All { of } | Self::AtLeast { of, .. } => {
                of.iter().any(WaitCondition::involves_approval)
            }
            _ => false,
        }
    }

    /// One line a model or a person can read.
    pub fn describe(&self) -> String {
        let list = |of: &[WaitCondition]| {
            of.iter()
                .map(WaitCondition::describe)
                .collect::<Vec<_>>()
                .join(", ")
        };
        match self {
            Self::Task { id } => format!("task {id}"),
            Self::Approval { id } => format!("approval {id}"),
            Self::MessageFrom { agent } => format!("a message from {agent}"),
            Self::Inbox => "any inbox item".into(),
            Self::Deadline { at } => format!("deadline {}", at.to_rfc3339()),
            Self::Any { of } => format!("any of [{}]", list(of)),
            Self::All { of } => format!("all of [{}]", list(of)),
            Self::AtLeast { n, of } => format!("{n} of [{}]", list(of)),
        }
    }

    pub fn is_satisfied(&self, oracle: &dyn WaitOracle) -> bool {
        match self {
            Self::Task { id } => oracle.task_is_terminal(*id),
            Self::Approval { id } => oracle.approval_is_resolved(*id),
            Self::MessageFrom { agent } => oracle.has_unconsumed_message_from(*agent),
            Self::Inbox => oracle.has_unconsumed_inbox(),
            Self::Deadline { at } => oracle.now() >= *at,
            // An empty `Any` can never be satisfied; an empty `All` is vacuously true.
            Self::Any { of } => of.iter().any(|c| c.is_satisfied(oracle)),
            Self::All { of } => of.iter().all(|c| c.is_satisfied(oracle)),
            Self::AtLeast { n, of } => of.iter().filter(|c| c.is_satisfied(oracle)).count() >= *n,
        }
    }

    /// Every task id referenced anywhere in the expression.
    pub fn task_ids(&self) -> BTreeSet<TaskId> {
        let mut out = BTreeSet::new();
        self.collect_tasks(&mut out);
        out
    }

    fn collect_tasks(&self, out: &mut BTreeSet<TaskId>) {
        match self {
            Self::Task { id } => {
                out.insert(*id);
            }
            Self::Any { of } | Self::All { of } | Self::AtLeast { of, .. } => {
                for c in of {
                    c.collect_tasks(out);
                }
            }
            _ => {}
        }
    }

    /// Every agent whose message is awaited anywhere in the expression.
    pub fn message_senders(&self) -> BTreeSet<AgentId> {
        let mut out = BTreeSet::new();
        self.collect_senders(&mut out);
        out
    }

    fn collect_senders(&self, out: &mut BTreeSet<AgentId>) {
        match self {
            Self::MessageFrom { agent } => {
                out.insert(*agent);
            }
            Self::Any { of } | Self::All { of } | Self::AtLeast { of, .. } => {
                for c in of {
                    c.collect_senders(out);
                }
            }
            _ => {}
        }
    }

    /// The earliest deadline in the expression, so the scheduler knows when
    /// to re-evaluate even without an inbox delivery.
    pub fn earliest_deadline(&self) -> Option<Timestamp> {
        match self {
            Self::Deadline { at } => Some(*at),
            Self::Any { of } | Self::All { of } | Self::AtLeast { of, .. } => {
                of.iter().filter_map(|c| c.earliest_deadline()).min()
            }
            _ => None,
        }
    }
}
