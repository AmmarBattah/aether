use super::*;

/// A turn that leaves early - an error returned past it, an unwind - drops
/// its heartbeat without reaching the end of `execute`. A heartbeat that
/// outlived it would renew the lease of an agent no turn is running, forever.
#[tokio::test]
async fn a_dropped_heartbeat_never_renews_again() {
    let (runtime, _dir) = crate::test_runtime(|c| c.lease_ttl = Duration::from_millis(40)).await;
    let (agent, _) = runtime
        .create_agent(
            CreateAgentSpec {
                role: "worker".into(),
                ..Default::default()
            },
            Actor::user(),
        )
        .expect("agent");
    let renewals = || {
        let filter = EventFilter {
            types: vec!["lease.renewed".into()],
            ..EventFilter::for_agent(agent.id)
        };
        runtime
            .store
            .events(0, 1_000, &filter)
            .expect("events")
            .len()
    };
    let lease = Lease::new(agent.id, "test", 1, now(), runtime.config.lease_ttl);
    let heartbeat = spawn_heartbeat(runtime.clone(), lease, &agent);
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while renewals() == 0 {
        assert!(std::time::Instant::now() < deadline, "never renewed");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    drop(heartbeat);
    let renewed = renewals();
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(renewals(), renewed, "a dropped heartbeat renewed the lease");
}
