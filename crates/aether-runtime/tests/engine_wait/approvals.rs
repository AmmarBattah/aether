use super::*;

/// A prompt raised while the turn winds down is denied outright. Putting it
/// to a person would both authorize work in a turn that is over and suspend
/// the drive loop on an unbounded approval wait, so the grace would never
/// fire and the agent would sit `Running` for ever.
#[tokio::test]
async fn a_permission_prompt_after_the_park_is_denied_and_the_stop_still_fires() {
    let mut h = harness(Duration::from_millis(200)).await;
    let gated = start_agent(
        &h,
        CreateAgentSpec {
            config: Some(AgentConfig {
                approval: ApprovalPolicy {
                    require_for: [SideEffectClass::Mutation].into_iter().collect(),
                },
                long_lived: true,
                ..Default::default()
            }),
            ..coordinator_spec()
        },
    );
    let first = turn_of(&mut h, gated).await;

    let (reply, is_error) = call(&h, &first, "wait_for", json!({"timeout_seconds": 3600})).await;
    assert!(!is_error, "{reply}");

    let (reply, is_error) = call(
        &h,
        &first,
        "permission",
        json!({"tool_name": "Bash", "input": {"command": "rm -rf /tmp/whatever"}}),
    )
    .await;
    assert!(
        !is_error,
        "the prompt itself is answered, not errored: {reply}"
    );
    let verdict: Value = serde_json::from_str(&reply).expect("prompt answer");
    assert_eq!(verdict["behavior"], json!("deny"));
    assert!(
        verdict["message"]
            .as_str()
            .expect("a reason")
            .starts_with(TURN_IS_OVER),
        "{reply}"
    );

    let parked = wait_for_park(&h, gated).await;
    assert!(
        first.observed.cancelled(),
        "the prompt suspended the wind-down; the engine was never stopped"
    );
    assert!(matches!(parked.status, AgentStatus::Waiting { .. }));
    assert!(
        seqs(h.store.as_ref(), |e| matches!(
            e.kind,
            EventKind::ApprovalRequested { .. }
        ))
        .is_empty(),
        "an operator was asked to authorize work in a turn that had already ended"
    );
    assert_eq!(
        custom(h.store.as_ref(), gated, "engine.parked").expect("recorded")["refused_calls"],
        json!(1)
    );
    h.runtime.shutdown().await;
}

/// The other door into the same hang: a harness call that needs an approval.
/// Once the turn's end is committed the gate may not ask a person - it would
/// authorize new work in a turn that is over, and the unbounded wait would
/// suspend the drive loop so the grace never fires.
#[tokio::test]
async fn a_harness_call_needing_approval_is_denied_after_the_park() {
    let mut h = harness(Duration::from_millis(200)).await;
    let gated = start_agent(
        &h,
        CreateAgentSpec {
            config: Some(AgentConfig {
                approval: ApprovalPolicy {
                    require_for: [SideEffectClass::ReadOnly].into_iter().collect(),
                },
                long_lived: true,
                ..Default::default()
            }),
            ..coordinator_spec()
        },
    );
    let first = turn_of(&mut h, gated).await;
    // Every harness tool is read-only, so without this the parking call would
    // be the one that asks.
    h.runtime
        .add_approval_rule(
            ApprovalDecision::Approved,
            RuleMatch::tool("wait_for"),
            RuleScope::Agent { id: gated },
            Actor::user(),
            None,
        )
        .expect("rule");

    let (reply, is_error) = call(&h, &first, "wait_for", json!({"timeout_seconds": 3600})).await;
    assert!(!is_error, "{reply}");

    let (reply, is_error) = call(&h, &first, "list_tasks", json!({})).await;
    assert!(
        is_error,
        "a gated call ran in a turn that was over: {reply}"
    );
    assert!(reply.starts_with(TURN_IS_OVER), "{reply}");
    assert!(
        !events(h.store.as_ref()).iter().any(|e| matches!(
            &e.kind,
            EventKind::ApprovalRequested { tool, .. } if tool == "list_tasks"
        )),
        "an operator was asked to authorize work in a turn that had already ended"
    );

    let parked = wait_for_park(&h, gated).await;
    assert!(
        first.observed.cancelled(),
        "the gate suspended the wind-down"
    );
    assert!(matches!(parked.status, AgentStatus::Waiting { .. }));
    h.runtime.shutdown().await;
}

/// The other permission route: an engine that asks through its own protocol
/// (Codex's `EngineEvent::Permission`) rather than the MCP bridge. It is
/// denied the same way, and the grace still fires - awaiting an approval here
/// would suspend the drive loop instead.
#[tokio::test]
async fn a_prompt_on_the_engine_s_own_protocol_is_denied_after_the_park() {
    let mut h = harness_full(
        Duration::from_millis(200),
        OnKill::Report(vec![cancelled_with(Usage::default())]),
        Script {
            prompt_on_stop: Some("Bash".into()),
            ..Default::default()
        },
    )
    .await;
    let gated = start_agent(
        &h,
        CreateAgentSpec {
            config: Some(AgentConfig {
                approval: ApprovalPolicy {
                    require_for: [SideEffectClass::Mutation].into_iter().collect(),
                },
                long_lived: true,
                ..Default::default()
            }),
            ..coordinator_spec()
        },
    );
    let first = turn_of(&mut h, gated).await;

    let (reply, is_error) = call(&h, &first, "wait_for", json!({"timeout_seconds": 3600})).await;
    assert!(!is_error, "{reply}");

    let parked = wait_for_park(&h, gated).await;
    let decided = first.observed.decided();
    assert_eq!(
        decided.len(),
        1,
        "the engine's own prompt was never answered: {decided:?}"
    );
    assert!(!decided[0].allow, "{decided:?}");
    assert!(
        decided[0]
            .message
            .as_deref()
            .expect("a reason")
            .starts_with(TURN_IS_OVER),
        "{decided:?}"
    );
    assert!(
        seqs(h.store.as_ref(), |e| matches!(
            e.kind,
            EventKind::ApprovalRequested { .. }
        ))
        .is_empty(),
        "an operator was asked to authorize work in a turn that had already ended"
    );
    assert!(first.observed.cancelled(), "the grace never fired");
    assert!(matches!(parked.status, AgentStatus::Waiting { .. }));
    assert_eq!(
        custom(h.store.as_ref(), gated, "engine.parked").expect("recorded")["refused_calls"],
        json!(1)
    );
    h.runtime.shutdown().await;
}

/// A parked turn may not ask a person - but a rule the operator wrote
/// beforehand is an answer they already gave, so a gated sibling of the
/// parking call still runs on it.
#[tokio::test]
async fn a_remembered_rule_still_admits_a_gated_sibling_after_the_park() {
    let mut h = harness(Duration::from_secs(30)).await;
    let gated = start_agent(
        &h,
        CreateAgentSpec {
            config: Some(AgentConfig {
                approval: ApprovalPolicy {
                    require_for: [SideEffectClass::ReadOnly].into_iter().collect(),
                },
                long_lived: true,
                ..Default::default()
            }),
            ..coordinator_spec()
        },
    );
    let first = turn_of(&mut h, gated).await;
    // Every harness tool is read-only, so both calls need a rule of their own.
    for tool in ["wait_for", "list_tasks"] {
        h.runtime
            .add_approval_rule(
                ApprovalDecision::Approved,
                RuleMatch::tool(tool),
                RuleScope::Agent { id: gated },
                Actor::user(),
                None,
            )
            .expect("rule");
    }

    let (reply, is_error) = call(&h, &first, "wait_for", json!({"timeout_seconds": 3600})).await;
    assert!(!is_error, "{reply}");

    let (reply, is_error) = call(&h, &first, "list_tasks", json!({})).await;
    assert!(
        !is_error,
        "a rule the operator wrote in advance was ignored once the turn parked: {reply}"
    );
    assert!(reply.contains(TURN_IS_OVER), "{reply}");

    let _ = first.end.send(());
    let parked = wait_for_park(&h, gated).await;
    assert!(matches!(parked.status, AgentStatus::Waiting { .. }));
    assert_eq!(
        custom(h.store.as_ref(), gated, "engine.parked").expect("recorded")["refused_calls"],
        json!(0)
    );
    h.runtime.shutdown().await;
}
