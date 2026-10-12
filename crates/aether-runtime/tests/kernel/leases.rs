//! leases scenarios and fixtures.
use super::*;

/// Limit 5: two agents mutating the same repository never run their tool
/// calls at once; the later one waits as Blocked with the holder named.
#[tokio::test]
async fn a_resource_lease_blocks_the_second_agent_and_names_the_holder() {
    let sleeping = || {
        ScriptedClient::tool_call(
            "bash",
            json!({"command": "for ((i=0;i<1000;i++)); do if test -f release; then exit 0; fi; sleep 0.01; done; exit 1"}),
        )
    };
    let done = || ScriptedClient::tool_call("complete_task", json!({"summary": "done"}));
    let h = harness_configured(
        vec![sleeping(), sleeping(), done(), done()],
        |_| {},
        |c| c.max_concurrent_turns = 2,
    )
    .await;
    let (a, _) = h
        .runtime
        .create_agent(spec("touch the repo"), Actor::user())
        .expect("a");
    let (b, _) = h
        .runtime
        .create_agent(spec("touch it too"), Actor::user())
        .expect("b");
    wait_for_with_budget(
        h.wait_ceiling,
        WAIT_UNTIL_STALL_WINDOW,
        || {
            let blocked = h
                .store
                .agents(&AgentFilter {
                    statuses: vec![AgentStatusKind::Blocked],
                    ..Default::default()
                })
                .expect("blocked agents");
            (!blocked.is_empty()).then_some(())
        },
        || {
            (
                activity_signature(&h.store.agent(a.id).expect("a").expect("exists")),
                activity_signature(&h.store.agent(b.id).expect("b").expect("exists")),
            )
        },
        || "no resource waiter appeared".to_owned(),
    )
    .await;
    let status = h.runtime.status().expect("status while parked");
    assert_eq!(
        status.agents_running, 1,
        "the resource waiter must not count as running"
    );
    assert_eq!(status.agents_blocked, 1);
    std::fs::write(h.workspace_root.join("release"), "release").expect("release holder");
    h.wait_until(a.id, |s| s.is_terminal()).await;
    h.wait_until(b.id, |s| s.is_terminal()).await;

    let waits: Vec<(AgentId, AgentId)> = h
        .store
        .events(0, 10_000, &EventFilter::all())
        .expect("events")
        .into_iter()
        .filter_map(|e| match e.kind {
            EventKind::ResourceWaiting { holder, .. } => Some((e.agent_id?, holder)),
            _ => None,
        })
        .collect();
    assert_eq!(waits.len(), 1, "expected exactly one waiter: {waits:?}");
    let (waiter, holder) = waits[0];
    assert_ne!(waiter, holder);
    assert!([a.id, b.id].contains(&waiter) && [a.id, b.id].contains(&holder));
    let waiter_events = types(h.store.as_ref(), waiter);
    assert!(
        waiter_events.contains(&"agent.blocked".to_owned()),
        "{waiter_events:?}"
    );
    assert!(
        waiter_events.contains(&"agent.resumed".to_owned()),
        "{waiter_events:?}"
    );
    assert!(
        waiter_events.contains(&"lease.resource_released".to_owned()),
        "{waiter_events:?}"
    );

    // The shell calls themselves never overlapped.
    let (sa, ea) = shell_window(h.store.as_ref(), a.id);
    let (sb, eb) = shell_window(h.store.as_ref(), b.id);
    assert!(
        ea < sb || eb < sa,
        "mutating tool calls overlapped: {a:?} {sa}..{ea} vs {sb}..{eb}",
        a = a.id
    );
    assert!(
        h.runtime.resource_holdings().is_empty(),
        "keys outlived their turns: {:?}",
        h.runtime.resource_holdings()
    );
    assert_eq!(h.client.remaining(), 0);
}
