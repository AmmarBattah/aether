//! shell leases scenarios and fixtures.
use super::*;

/// Two agents in ONE workspace: a read-only shell call never waits for the
/// exclusive key a mutating one holds. Limit 5 is about writers.
#[tokio::test]
async fn a_read_only_shell_call_runs_beside_a_mutating_one_in_one_workspace() {
    let mutating = || ScriptedClient::tool_call("bash", json!({"command": "sleep 0.4"}));
    let reading =
        || ScriptedClient::tool_call("bash", json!({"command": "sleep 0.4", "read_only": true}));
    let done = || ScriptedClient::tool_call("complete_task", json!({"summary": "done"}));
    let h = harness_configured(
        vec![mutating(), reading(), done(), done()],
        |_| {},
        |c| c.max_concurrent_turns = 2,
    )
    .await;
    let (a, b) = two_agents_in_one_workspace(&h);
    h.wait_until(a, |s| s.is_terminal()).await;
    h.wait_until(b, |s| s.is_terminal()).await;
    assert_eq!(
        lease_waits(h.store.as_ref()),
        0,
        "a read-only call must not queue behind a writer"
    );
    let (sa, ea) = shell_window(h.store.as_ref(), a);
    let (sb, eb) = shell_window(h.store.as_ref(), b);
    assert!(
        sa < eb && sb < ea,
        "the calls were serialized: {sa}..{ea} vs {sb}..{eb}"
    );
    assert_eq!(h.client.remaining(), 0);
}

/// The other half of the same rule: two mutating shell calls in one
/// workspace take the repository key in turn, and the waiter is visible.
#[tokio::test]
async fn two_mutating_shell_calls_in_one_workspace_serialize() {
    let mutating = || ScriptedClient::tool_call("bash", json!({"command": "sleep 0.4"}));
    let done = || ScriptedClient::tool_call("complete_task", json!({"summary": "done"}));
    let h = harness_configured(
        vec![mutating(), mutating(), done(), done()],
        |_| {},
        |c| c.max_concurrent_turns = 2,
    )
    .await;
    let (a, b) = two_agents_in_one_workspace(&h);
    h.wait_until(a, |s| s.is_terminal()).await;
    h.wait_until(b, |s| s.is_terminal()).await;
    assert_eq!(
        lease_waits(h.store.as_ref()),
        1,
        "one of the two must wait for the other"
    );
    let (sa, ea) = shell_window(h.store.as_ref(), a);
    let (sb, eb) = shell_window(h.store.as_ref(), b);
    assert!(
        ea < sb || eb < sa,
        "two writers overlapped in one repository: {sa}..{ea} vs {sb}..{eb}"
    );
    assert_eq!(h.client.remaining(), 0);
}

/// A lease wait is bounded: past the scheduler's wait deadline the turn ends
/// with its calls settled and the agent is told who held the key.
#[tokio::test]
async fn a_lease_wait_gives_up_and_names_the_holder() {
    let mutating = || ScriptedClient::tool_call("bash", json!({"command": "sleep 3"}));
    let done = || ScriptedClient::tool_call("complete_task", json!({"summary": "done"}));
    let h = harness_configured(
        vec![mutating(), mutating(), done(), done(), done(), done()],
        |_| {},
        |c| {
            c.max_concurrent_turns = 2;
            c.resource_wait_timeout = Duration::from_millis(300);
        },
    )
    .await;
    let (a, b) = two_agents_in_one_workspace(&h);
    h.wait_until(a, |s| s.is_terminal()).await;
    h.wait_until(b, |s| s.is_terminal()).await;
    let cancelled: Vec<AgentId> = [a, b]
        .into_iter()
        .filter(|id| types(h.store.as_ref(), *id).contains(&"tool.cancelled".to_owned()))
        .collect();
    assert_eq!(cancelled.len(), 1, "exactly one call gave up on the key");
    let told = h
        .store
        .inbox(cancelled[0], 0)
        .expect("inbox")
        .into_iter()
        .any(|i| {
            matches!(&i.kind, aether_core::inbox::InboxKind::Wake { reason }
            if reason.contains("claimed by"))
        });
    assert!(told, "the agent is told who held the key");
}

/// Two agents sharing one workspace root, the case leases exist for.
fn two_agents_in_one_workspace(h: &Harness) -> (AgentId, AgentId) {
    let in_workspace = || CreateAgentSpec {
        scope: Some(Scope {
            workspace_root: Some(h.workspace_root.clone()),
            ..Default::default()
        }),
        ..spec("shell out")
    };
    let (a, _) = h
        .runtime
        .create_agent(in_workspace(), Actor::user())
        .expect("a");
    let (b, _) = h
        .runtime
        .create_agent(in_workspace(), Actor::user())
        .expect("b");
    (a.id, b.id)
}

fn lease_waits(store: &dyn Store) -> usize {
    store
        .events(0, 10_000, &EventFilter::all())
        .expect("events")
        .into_iter()
        .filter(|e| e.type_name() == "lease.resource_waiting")
        .count()
}

/// VISION §3.6 limit 5 is about data, not about the network: two agents
/// shelling out in different repositories run at once even when both may
/// reach the network, and neither waits for a key.
#[tokio::test]
async fn a_networked_shell_call_in_another_repository_is_not_a_fleet_wide_mutex() {
    let sleeping = || ScriptedClient::tool_call("bash", json!({"command": "sleep 0.4"}));
    let done = || ScriptedClient::tool_call("complete_task", json!({"summary": "done"}));
    let h = harness_configured(
        vec![sleeping(), sleeping(), done(), done()],
        |_| {},
        |c| c.max_concurrent_turns = 2,
    )
    .await;
    let networked = |dir: &str| {
        let root = h.workspace_root.join(dir);
        std::fs::create_dir_all(&root).expect("workspace");
        CreateAgentSpec {
            scope: Some(Scope {
                workspace_root: Some(root),
                network: aether_core::agent::NetworkPolicy::Allowed,
                ..Default::default()
            }),
            ..spec("shell out")
        }
    };
    let (a, _) = h
        .runtime
        .create_agent(networked("one"), Actor::user())
        .expect("a");
    let (b, _) = h
        .runtime
        .create_agent(networked("two"), Actor::user())
        .expect("b");
    h.wait_until(a.id, |s| s.is_terminal()).await;
    h.wait_until(b.id, |s| s.is_terminal()).await;
    let waits = h
        .store
        .events(0, 10_000, &EventFilter::all())
        .expect("events")
        .into_iter()
        .filter(|e| e.type_name() == "lease.resource_waiting")
        .count();
    assert_eq!(waits, 0, "separate repositories never contend");
    let (sa, ea) = shell_window(h.store.as_ref(), a.id);
    let (sb, eb) = shell_window(h.store.as_ref(), b.id);
    assert!(
        sa < eb && sb < ea,
        "the shell calls were serialized: {sa}..{ea} vs {sb}..{eb}"
    );
    assert_eq!(h.client.remaining(), 0);
}
