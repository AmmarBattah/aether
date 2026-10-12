//! exhaustion support scenarios and fixtures.
use super::*;

pub(super) fn child_failures(store: &dyn Store, id: AgentId) -> Vec<String> {
    store
        .inbox(id, 0)
        .unwrap()
        .into_iter()
        .filter_map(|item| match item.kind {
            aether_core::inbox::InboxKind::ChildFailed { error, .. } => Some(error),
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Suspension on provider exhaustion
// ---------------------------------------------------------------------------

/// A parent that never takes a turn and a child that does, so the child's
/// suspension can be read out of the parent's inbox.
pub(super) async fn parent_and_suspending_child(
    h: &Harness,
    child_spec: CreateAgentSpec,
) -> (AgentId, AgentId, aether_core::id::TaskId) {
    let (parent, _) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                start: false,
                ..spec("watch the worker")
            },
            Actor::user(),
        )
        .expect("parent");
    let (child, task) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                parent: Some(parent.id),
                ..child_spec
            },
            Actor::Agent { id: parent.id },
        )
        .expect("child");
    (parent.id, child.id, task.expect("the child owns a task").id)
}

/// This agent's own `exhaustion.<name>` events, newest last.
pub(super) fn exhaustion_records(
    store: &dyn Store,
    id: AgentId,
    name: &str,
) -> Vec<(Timestamp, serde_json::Value)> {
    store
        .events(0, 10_000, &EventFilter::own_events(id))
        .expect("events")
        .into_iter()
        .filter_map(|event| match event.kind {
            EventKind::Custom {
                name: ref n,
                ref payload,
            } if n == &format!("exhaustion.{name}") => Some((event.at, payload.clone())),
            _ => None,
        })
        .collect()
}

pub(super) fn child_suspensions(store: &dyn Store, id: AgentId) -> Vec<String> {
    store
        .inbox(id, 0)
        .expect("inbox")
        .into_iter()
        .filter_map(|item| match item.kind {
            aether_core::inbox::InboxKind::ChildSuspended { detail, .. } => Some(detail),
            _ => None,
        })
        .collect()
}

/// When the provider was last benched, and whether it was ever let off.
pub(super) fn bench_history(store: &dyn Store) -> (Option<Timestamp>, usize) {
    let events = store
        .events(0, 10_000, &EventFilter::all())
        .expect("events");
    let benched = events.iter().rev().find_map(|event| match event.kind {
        EventKind::ProviderBackoff { until, .. } => Some(until),
        _ => None,
    });
    let recovered = events
        .iter()
        .filter(|event| matches!(event.kind, EventKind::ProviderRecovered { .. }))
        .count();
    (benched, recovered)
}

pub(super) fn blocked_on_exhaustion(status: &AgentStatus) -> bool {
    matches!(
        status,
        AgentStatus::Blocked {
            reason: BlockReason::Exhausted { .. }
        }
    )
}

/// The parked schedule, or a panic naming what the agent is doing instead.
pub(super) fn exhausted_reason(status: &AgentStatus) -> BlockReason {
    match status {
        AgentStatus::Blocked { reason } if matches!(reason, BlockReason::Exhausted { .. }) => {
            reason.clone()
        }
        other => panic!("expected a suspended agent, got {other:?}"),
    }
}

pub(super) fn schedule_of(reason: &BlockReason) -> (Timestamp, u32) {
    let BlockReason::Exhausted {
        next_probe, probes, ..
    } = reason
    else {
        panic!("{reason:?}")
    };
    (*next_probe, *probes)
}

/// Long enough that a suspended agent stays parked for the whole test.
pub(super) const NO_PROBE_IN_THIS_TEST: Duration = Duration::from_secs(3600);
