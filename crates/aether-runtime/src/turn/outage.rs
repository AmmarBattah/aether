//! A disk that refuses writes during a turn. The turn cannot record its own
//! end then, so it is ended for it once writes land again, and the refusal
//! goes on the agent's log rather than only into a log file on that disk.

use std::sync::Arc;
use std::time::Duration;

use aether_core::event::{Actor, EventDraft, EventKind};
use aether_core::id::{AgentId, RunId};
use aether_core::time::{Timestamp, now};
use aether_store::Commit;

use crate::{Result, Runtime, RuntimeError};

/// The event a refused write leaves on the agent once a write lands again.
const STORE_WRITE_FAILED: &str = "store.write_failed";

/// First wait before ending the turn again; each refusal doubles it.
const RETRY_FLOOR: Duration = Duration::from_secs(1);
const RETRY_CAP: Duration = Duration::from_secs(30);

/// Writes refused in a row, from the first, until one lands.
pub(super) struct StoreOutage {
    error: String,
    since: Timestamp,
    refused: u32,
    /// What was being written when the first was refused.
    during: &'static str,
}

impl StoreOutage {
    pub(super) fn began(error: &RuntimeError, during: &'static str) -> Self {
        Self {
            error: error.to_string(),
            since: now(),
            refused: 1,
            during,
        }
    }

    pub(super) fn refused_again(&mut self) {
        self.refused = self.refused.saturating_add(1);
    }

    pub(super) fn draft(&self, agent: AgentId, parent: Option<AgentId>, run: RunId) -> EventDraft {
        EventDraft::new(
            Actor::System,
            EventKind::Custom {
                name: STORE_WRITE_FAILED.into(),
                payload: serde_json::json!({
                    "error": self.error,
                    "during": self.during,
                    "since": self.since,
                    "refused_writes": self.refused,
                }),
            },
        )
        .agent(agent)
        .parent(parent)
        .run(run)
    }
}

/// How a turn that could not end itself is ended.
pub(super) enum Ending {
    /// As a restart would: its lease is stale, and what the turn left open
    /// decides where the agent goes.
    Recover,
    Fail(String),
}

/// End the turn's agent, waiting out a disk that refuses the writes. On
/// shutdown the agent is left as a crash leaves it, for the next start.
pub(super) async fn settle(
    runtime: &Arc<Runtime>,
    agent_id: AgentId,
    ending: Ending,
    mut outage: Option<StoreOutage>,
) {
    let mut wait = RETRY_FLOOR;
    loop {
        let error = match settle_once(runtime, agent_id, &ending, &mut outage) {
            Ok(()) => return,
            Err(e) if e.is_store_unwritable() => e,
            Err(e) => {
                tracing::error!(agent = %agent_id, error = %e, "could not record how the turn ended");
                return;
            }
        };
        match &mut outage {
            Some(outage) => outage.refused_again(),
            None => {
                tracing::error!(
                    agent = %agent_id,
                    error = %error,
                    "the disk refused the turn's end; retrying until it takes writes"
                );
                outage = Some(StoreOutage::began(&error, "the turn's end"));
            }
        }
        tokio::select! {
            _ = tokio::time::sleep(wait) => {}
            _ = runtime.shutdown.cancelled() => return,
        }
        wait = (wait * 2).min(RETRY_CAP);
    }
}

fn settle_once(
    runtime: &Arc<Runtime>,
    agent_id: AgentId,
    ending: &Ending,
    outage: &mut Option<StoreOutage>,
) -> Result<()> {
    // Written first, so it is also what finds out whether the disk is back.
    if let Some(refused) = outage.as_ref() {
        if let Some(agent) = runtime.store.agent(agent_id)? {
            runtime.apply(Commit::new().event(refused.draft(agent.id, agent.parent, agent.run)))?;
        }
        tracing::warn!(
            agent = %agent_id,
            since = %refused.since,
            refused = refused.refused,
            "the store takes writes again"
        );
        *outage = None;
    }
    match ending {
        Ending::Fail(error) => super::fail_agent(runtime, agent_id, error.clone()),
        Ending::Recover => recover_turn(runtime, agent_id),
    }
}

/// Recover the turn's agent as a restart would. Running is only ever written
/// with the lease, so no lease means the turn never started or already ended.
fn recover_turn(runtime: &Arc<Runtime>, agent_id: AgentId) -> Result<()> {
    let Some(lease) = runtime.store.lease(agent_id)? else {
        return Ok(());
    };
    // Same boot by definition: the turn's own tool processes may still be
    // running, so unsafe calls keep the inspection hold.
    crate::recovery::recover_leases(runtime, vec![lease], false)?;
    Ok(())
}
