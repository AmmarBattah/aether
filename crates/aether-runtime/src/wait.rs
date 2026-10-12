//! Store-backed evaluation of wait conditions.

use aether_core::agent::WaitOracle;
use aether_core::event::Actor;
use aether_core::id::{AgentId, ApprovalId, TaskId};
use aether_core::inbox::InboxKind;
use aether_core::time::{Timestamp, now};
use aether_core::tool::ApprovalStatus;
use aether_store::Store;

mod runtime;
pub(crate) use runtime::{RuntimeOracle, settlement_in_flight, task_turn_running};

#[cfg(test)]
mod tests;

/// Answers the scheduler's questions for one agent from authoritative state.
pub struct StoreOracle<'a> {
    pub store: &'a dyn Store,
    pub agent: AgentId,
    pub inbox_cursor: u64,
}

impl std::fmt::Debug for StoreOracle<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoreOracle")
            .field("agent", &self.agent)
            .finish()
    }
}

impl WaitOracle for StoreOracle<'_> {
    fn task_is_terminal(&self, task: TaskId) -> bool {
        match self.store.task(task) {
            Ok(Some(t)) => t.status.is_terminal(),
            // A missing task can never complete; treat it as terminal so the
            // agent wakes and sees the failure rather than sleeping forever.
            Ok(None) => true,
            Err(e) => {
                tracing::error!(error = %e, %task, "task lookup failed while evaluating wait");
                false
            }
        }
    }

    fn approval_is_resolved(&self, approval: ApprovalId) -> bool {
        match self.store.approval(approval) {
            Ok(Some(a)) => matches!(a.status, ApprovalStatus::Resolved { .. }),
            Ok(None) => true,
            Err(e) => {
                tracing::error!(error = %e, %approval, "approval lookup failed while evaluating wait");
                false
            }
        }
    }

    fn has_unconsumed_inbox(&self) -> bool {
        match self.store.inbox_last_seq(self.agent) {
            Ok(last) => last > self.inbox_cursor,
            Err(e) => {
                tracing::error!(error = %e, agent = %self.agent, "inbox lookup failed");
                false
            }
        }
    }

    fn has_unconsumed_message_from(&self, agent: AgentId) -> bool {
        match self.store.inbox(self.agent, self.inbox_cursor) {
            Ok(items) => items.iter().any(|i| {
                matches!(&i.kind, InboxKind::Message { from: Actor::Agent { id }, .. } if *id == agent)
            }),
            Err(e) => {
                tracing::error!(error = %e, agent = %self.agent, "inbox lookup failed");
                false
            }
        }
    }

    fn now(&self) -> Timestamp {
        now()
    }
}
