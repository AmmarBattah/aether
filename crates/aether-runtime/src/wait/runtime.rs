//! Scheduling waits also require the result-producing turn to be quiescent.

use super::*;
use crate::Runtime;

pub(crate) fn task_turn_running(runtime: &Runtime, task: TaskId) -> bool {
    runtime
        .sched
        .lock()
        .running
        .values()
        .any(|turn| turn.task == Some(task))
}

pub(crate) fn settlement_in_flight(runtime: &Runtime, kind: &InboxKind) -> bool {
    match kind {
        InboxKind::TaskCompleted { task, .. }
        | InboxKind::TaskFailed { task, .. }
        | InboxKind::TaskCancelled { task, .. } => task_turn_running(runtime, *task),
        InboxKind::ChildFailed { agent, .. } => runtime.sched.lock().running.contains_key(agent),
        _ => false,
    }
}

/// Keep StoreOracle's public store-only contract while adding the runtime's
/// in-flight barrier. A terminal task row can precede a native terminal receipt.
pub(crate) struct RuntimeOracle<'a> {
    pub runtime: &'a Runtime,
    pub agent: AgentId,
    pub inbox_cursor: u64,
}

impl RuntimeOracle<'_> {
    fn stored(&self) -> StoreOracle<'_> {
        StoreOracle {
            store: self.runtime.store.as_ref(),
            agent: self.agent,
            inbox_cursor: self.inbox_cursor,
        }
    }
}

impl WaitOracle for RuntimeOracle<'_> {
    fn task_is_terminal(&self, task: TaskId) -> bool {
        self.stored().task_is_terminal(task) && !task_turn_running(self.runtime, task)
    }

    fn approval_is_resolved(&self, approval: ApprovalId) -> bool {
        self.stored().approval_is_resolved(approval)
    }

    fn has_unconsumed_inbox(&self) -> bool {
        match self.runtime.store.inbox(self.agent, self.inbox_cursor) {
            Ok(items) => items
                .iter()
                .any(|item| !settlement_in_flight(self.runtime, &item.kind)),
            Err(error) => {
                tracing::error!(%error, agent = %self.agent, "inbox lookup failed while evaluating runtime wait");
                false
            }
        }
    }

    fn has_unconsumed_message_from(&self, agent: AgentId) -> bool {
        self.stored().has_unconsumed_message_from(agent)
    }

    fn now(&self) -> Timestamp {
        now()
    }
}
