//! subtree policy scenarios and fixtures.
use super::*;

#[tokio::test]
async fn network_defaults_apply_only_to_new_roots_and_children_still_narrow() {
    use aether_core::agent::NetworkPolicy;
    assert_eq!(
        RuntimeConfig::default().network_default,
        NetworkPolicy::Allowed
    );
    for network in [NetworkPolicy::Allowed, NetworkPolicy::Denied] {
        let h = harness_configured(vec![], |_| {}, |c| c.network_default = network).await;
        let (root, _) = h
            .runtime
            .create_agent(
                CreateAgentSpec {
                    role: "coordinator".into(),
                    ..Default::default()
                },
                Actor::user(),
            )
            .expect("root");
        assert_eq!(root.scope.network, network);
        assert_eq!(h.runtime.status().expect("status").network_default, network);
        let (child, _) = h
            .runtime
            .create_agent(
                CreateAgentSpec {
                    role: "worker".into(),
                    parent: Some(root.id),
                    config: Some(aether_core::agent::AgentConfig {
                        effort: Some(aether_core::model::Effort::Low),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                Actor::agent(root.id),
            )
            .expect("child");
        assert_eq!(child.scope.network, network);
        let (denied, _) = h
            .runtime
            .create_agent(
                CreateAgentSpec {
                    role: "coordinator".into(),
                    scope: Some(aether_core::agent::Scope::default()),
                    ..Default::default()
                },
                Actor::user(),
            )
            .expect("explicit denial");
        assert_eq!(denied.scope.network, NetworkPolicy::Denied);
        assert!(
            h.runtime
                .create_agent(
                    CreateAgentSpec {
                        role: "worker".into(),
                        parent: Some(denied.id),
                        config: Some(aether_core::agent::AgentConfig {
                            effort: Some(aether_core::model::Effort::Low),
                            ..Default::default()
                        }),
                        scope: Some(aether_core::agent::Scope {
                            network: NetworkPolicy::Allowed,
                            ..denied.scope.clone()
                        }),
                        ..Default::default()
                    },
                    Actor::agent(denied.id)
                )
                .is_err()
        );
        h.runtime.shutdown().await;
        let restarted = harness_full(
            vec![],
            |_| {},
            |config| config.network_default = NetworkPolicy::Allowed,
            None,
            None,
            Some(h.store.clone()),
            |_| {},
        )
        .await;
        assert_eq!(
            restarted
                .store
                .agent(root.id)
                .expect("stored root")
                .expect("root exists")
                .scope
                .network,
            network
        );
        assert_eq!(
            restarted
                .store
                .agent(denied.id)
                .expect("stored denied root")
                .expect("root exists")
                .scope
                .network,
            NetworkPolicy::Denied
        );
        restarted.runtime.shutdown().await;
    }
}

#[tokio::test]
async fn subtree_cap_survives_a_completed_intermediate() {
    let h = harness_configured(
        vec![
            ScriptedClient::tool_call("complete_task", json!({"summary":"first"})),
            ScriptedClient::tool_call("complete_task", json!({"summary":"second"})),
        ],
        |_| {},
        |config| config.max_concurrent_turns = 4,
    )
    .await;
    h.client.set_delay(Duration::from_millis(50));
    let mut root_spec = spec("supervise");
    root_spec.start = false;
    root_spec.config.as_mut().unwrap().concurrency = Some(1);
    let (root, _) = h.runtime.create_agent(root_spec, Actor::user()).unwrap();
    let mut middle_spec = spec("finished middle");
    middle_spec.start = false;
    middle_spec.parent = Some(root.id);
    let (mut middle, _) = h.runtime.create_agent(middle_spec, Actor::user()).unwrap();
    let mut children = Vec::new();
    for name in ["first", "second"] {
        let mut child_spec = spec(name);
        child_spec.start = false;
        child_spec.parent = Some(middle.id);
        children.push(h.runtime.create_agent(child_spec, Actor::user()).unwrap().0);
    }
    middle.status = AgentStatus::Completed { summary: None };
    let mut commit = Commit::new().mutate(Mutation::UpsertAgent(middle));
    for child in &mut children {
        child.status = AgentStatus::Ready;
        commit.push_mutation(Mutation::UpsertAgent(child.clone()));
    }
    h.store.commit(commit).unwrap();
    aether_runtime::scheduler::tick(&h.runtime).unwrap();
    for child in &children {
        h.wait_until(child.id, |status| status.is_terminal()).await;
    }
    let events = h
        .store
        .events(0, 1000, &EventFilter::for_run(root.run))
        .unwrap();
    let mut active = 0;
    for event in events {
        if !children
            .iter()
            .any(|child| Some(child.id) == event.agent_id)
        {
            continue;
        }
        match event.type_name().as_str() {
            "agent.started" => {
                active += 1;
                assert_eq!(active, 1, "ancestor cap was bypassed");
            }
            "agent.turn_completed" => active -= 1,
            _ => {}
        }
    }
    assert_eq!(active, 0);
}

#[tokio::test]
async fn subtree_pause_reaches_past_a_completed_intermediate() {
    let h = harness(vec![]).await;
    let mut parent = None;
    let mut tree = Vec::new();
    for name in ["root", "middle", "grandchild"] {
        let mut requested = spec(name);
        requested.start = false;
        requested.parent = parent;
        let (agent, _) = h.runtime.create_agent(requested, Actor::user()).unwrap();
        parent = Some(agent.id);
        tree.push(agent);
    }
    tree[1].status = AgentStatus::Completed { summary: None };
    h.store
        .commit(Commit::new().mutate(Mutation::UpsertAgent(tree[1].clone())))
        .unwrap();
    h.runtime
        .pause_agent(tree[0].id, true, Actor::user())
        .unwrap();
    assert!(h.store.agent(tree[2].id).unwrap().unwrap().paused);
    assert!(!h.store.agent(tree[1].id).unwrap().unwrap().paused);
}

#[tokio::test]
async fn subtree_policy_checks_reach_past_a_completed_intermediate() {
    let h = harness(vec![]).await;
    let mut parent = None;
    let mut tree = Vec::new();
    for name in ["root", "middle", "grandchild"] {
        let mut requested = spec(name);
        requested.start = false;
        requested.parent = parent;
        let (agent, _) = h.runtime.create_agent(requested, Actor::user()).unwrap();
        parent = Some(agent.id);
        tree.push(agent);
    }
    let mut commit = Commit::new();
    tree[1].status = AgentStatus::Completed { summary: None };
    for agent in &mut tree {
        agent.scope.network = aether_core::agent::NetworkPolicy::Allowed;
        commit.push_mutation(Mutation::UpsertAgent(agent.clone()));
    }
    h.store.commit(commit).unwrap();
    for update in [
        AgentUpdate {
            network: Some(aether_core::agent::NetworkPolicy::Denied),
            ..Default::default()
        },
        AgentUpdate {
            approval: Some(ApprovalPolicy {
                require_for: [SideEffectClass::IdempotentMutation].into(),
            }),
            ..Default::default()
        },
    ] {
        let error = h
            .runtime
            .update_agent(tree[0].id, update, Actor::user())
            .unwrap_err();
        assert!(
            error.to_string().contains(&tree[2].id.to_string()),
            "{error}"
        );
    }
    let root = h.store.agent(tree[0].id).unwrap().unwrap();
    assert_eq!(
        root.scope.network,
        aether_core::agent::NetworkPolicy::Allowed
    );
    assert_eq!(root.config.approval, tree[0].config.approval);
}
