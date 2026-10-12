use super::RuntimeStatus;
use crate::{Result, Runtime};
use aether_core::agent::{Agent, AgentStatus, AgentStatusKind, BlockReason};
use aether_core::id::TaskId;
use aether_core::time::now;
use aether_store::{AgentFilter, TaskFilter};
use std::collections::BTreeSet;

impl Runtime {
    pub fn status(&self) -> Result<RuntimeStatus> {
        let mut engines: Vec<String> = self.engines.values().map(|e| e.adapter()).collect();
        engines.sort();
        // Read before the struct literal: `provider_health` takes the same
        // lock, and a guard living inside the literal would still be held.
        let engine_turns_running = self.sched.lock().on_engine() as u32;
        let mut status = RuntimeStatus {
            yolo: self.config.yolo,
            network_default: self.config.network_default,
            max_concurrent_turns: self.config.max_concurrent_turns,
            max_concurrent_engine_turns: self.config.max_concurrent_engine_turns,
            engine_turns_running,
            run_budget_usd: self.config.run_budget_usd,
            engines,
            providers: self.provider_health()?,
            resource_leases: self.resource_holdings(),
            mcp: self.mcp_health(),
            low_disk: self.low_disk(),
            ..Default::default()
        };
        for agent in self.store.agents(&AgentFilter {
            live_only: true,
            ..Default::default()
        })? {
            status.agents_live += 1;
            if agent.paused {
                status.agents_paused += 1;
            }
            match agent.status.kind() {
                AgentStatusKind::Running => status.agents_running += 1,
                AgentStatusKind::Waiting => status.agents_waiting += 1,
                AgentStatusKind::Blocked => {
                    status.agents_blocked += 1;
                    if matches!(
                        &agent.status,
                        AgentStatus::Blocked {
                            reason: BlockReason::Exhausted { .. }
                        }
                    ) {
                        status.agents_suspended += 1;
                    }
                }
                _ => {}
            }
        }
        let all = self.store.agents(&AgentFilter::default())?;
        for agent in &all {
            // An imported run is an archive of what another machine spent.
            // Counting it here would bill this daemon for someone else's
            // work; it is reported under its own run, by `runs report`.
            if agent.status.kind() != AgentStatusKind::Imported {
                status.usage_total += agent.stats.usage;
            }
        }
        status.runs = aether_core::budget::active_runs(&all, now());
        status.tasks_active = self
            .store
            .tasks(&TaskFilter {
                statuses: vec![
                    aether_core::task::TaskStatusKind::Active,
                    aether_core::task::TaskStatusKind::Assigned,
                ],
                ..Default::default()
            })?
            .len() as u32;
        status.pending_approvals = self.store.pending_approvals()?.len() as u32;
        status.approval_default = self.config.approval_default;
        Ok(status)
    }

    /// Tasks that a waiting agent is blocked on, for the live graph.
    pub fn waiting_on(agent: &Agent) -> BTreeSet<TaskId> {
        match &agent.status {
            AgentStatus::Waiting { condition } => condition.task_ids(),
            _ => BTreeSet::new(),
        }
    }
}
