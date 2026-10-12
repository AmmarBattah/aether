//! pause recovery scenarios and fixtures.
use super::*;

/// VISION §3.1: the pause is durable state, not a scheduler decision, so a
/// daemon restart finds it and honours it; only a resume lifts it, and the
/// inbox that filled meanwhile is answered then.
#[tokio::test]
async fn a_pause_survives_a_restart_and_only_a_resume_lifts_it() {
    let h = harness(vec![]).await;
    let (agent, _) = h
        .runtime
        .create_agent(idle_spec(), Actor::user())
        .expect("create");
    h.wait_until(agent.id, |s| s.kind() == AgentStatusKind::Waiting)
        .await;

    let outcome = h
        .runtime
        .pause_agent(agent.id, false, Actor::user())
        .expect("pause");
    assert_eq!(outcome.changed.len(), 1);
    assert!(outcome.changed[0].paused);
    assert_eq!(
        outcome.changed[0].status.kind(),
        AgentStatusKind::Waiting,
        "a pause is a flag, not a status"
    );
    assert!(
        h.runtime
            .pause_agent(agent.id, false, Actor::user())
            .expect("pause again")
            .changed
            .is_empty(),
        "pausing twice changes nothing"
    );

    // The inbox keeps filling while paused; nothing is consumed.
    h.runtime
        .send_message(
            agent.id,
            vec![ContentBlock::text("are you there?")],
            Actor::user(),
        )
        .expect("send");
    tokio::time::sleep(Duration::from_millis(300)).await;
    let while_paused = h.store.agent(agent.id).expect("agent").expect("exists");
    assert!(while_paused.paused);
    assert_eq!(
        while_paused.inbox_cursor, 0,
        "nothing consumed while paused"
    );
    assert_eq!(h.store.inbox(agent.id, 0).expect("inbox").len(), 1);

    // A second daemon over the same state: the flag is in the store, not in
    // this process.
    h.runtime.shutdown().await;
    let h2 = harness_full(
        vec![ScriptedClient::text("Here.")],
        |_| {},
        |_| {},
        None,
        None,
        Some(h.store.clone()),
        |_| {},
    )
    .await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(paused(h2.store.as_ref(), agent.id));
    assert_eq!(h2.client.remaining(), 1, "no turn ran across the restart");

    let resumed = h2
        .runtime
        .resume_agent(agent.id, false, Actor::user())
        .expect("resume");
    assert_eq!(resumed.len(), 1);
    assert!(!resumed[0].paused);
    h2.wait_until(agent.id, |s| s.kind() == AgentStatusKind::Waiting)
        .await;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    while h2
        .store
        .agent(agent.id)
        .expect("a")
        .expect("e")
        .inbox_cursor
        == 0
    {
        assert!(
            tokio::time::Instant::now() < deadline,
            "the resumed agent never answered the message it slept through"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(
        h2.store
            .transcript(agent.id, 0)
            .expect("transcript")
            .last()
            .expect("reply")
            .message
            .text(),
        "Here."
    );
    let types = types(h2.store.as_ref(), agent.id);
    assert!(types.contains(&"agent.paused".to_owned()), "{types:?}");
    assert!(types.contains(&"agent.resumed".to_owned()), "{types:?}");
}

/// VISION §3.1 with §3.5: a subtree pause reaches every live descendant and
/// stops at the subtree's edge; the resume wakes only the agent that had work
/// waiting for it.
#[tokio::test]
async fn pausing_a_subtree_reaches_every_agent_beneath_it() {
    let h = harness(vec![ScriptedClient::text("Answering now.")]).await;
    let (root, _) = h
        .runtime
        .create_agent(idle_spec(), Actor::user())
        .expect("root");
    let mut child_spec = idle_spec();
    child_spec.parent = Some(root.id);
    let (child, _) = h
        .runtime
        .create_agent(child_spec, Actor::user())
        .expect("child");
    let mut grandchild_spec = idle_spec();
    grandchild_spec.parent = Some(child.id);
    let (grandchild, _) = h
        .runtime
        .create_agent(grandchild_spec, Actor::user())
        .expect("grandchild");
    // An agent outside the subtree must not be touched.
    let (bystander, _) = h
        .runtime
        .create_agent(idle_spec(), Actor::user())
        .expect("bystander");
    for id in [root.id, child.id, grandchild.id, bystander.id] {
        h.wait_until(id, |s| s.kind() == AgentStatusKind::Waiting)
            .await;
    }

    let outcome = h
        .runtime
        .pause_agent(root.id, true, Actor::user())
        .expect("pause subtree");
    let ids: Vec<_> = outcome.changed.iter().map(|a| a.id).collect();
    assert_eq!(ids, vec![root.id, child.id, grandchild.id]);
    assert!(
        !paused(h.store.as_ref(), bystander.id),
        "a subtree pause stays inside the subtree"
    );
    // The descendants' events name the pause that reached them.
    for id in [child.id, grandchild.id] {
        let via = h
            .store
            .events(0, 1000, &EventFilter::for_agent(id))
            .expect("events")
            .into_iter()
            .find_map(|e| match e.kind {
                EventKind::AgentPaused { via } => Some(via),
                _ => None,
            })
            .expect("agent.paused");
        assert_eq!(via, Some(root.id));
    }

    h.runtime
        .send_message(
            grandchild.id,
            vec![ContentBlock::text("anyone home?")],
            Actor::user(),
        )
        .expect("send");
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        h.store
            .agent(grandchild.id)
            .expect("agent")
            .expect("exists")
            .inbox_cursor,
        0,
        "a person's message does not override a pause"
    );

    let resumed = h
        .runtime
        .resume_agent(root.id, true, Actor::user())
        .expect("resume subtree");
    assert_eq!(resumed.len(), 3);
    for id in [root.id, child.id, grandchild.id] {
        assert!(!paused(h.store.as_ref(), id));
    }
    // Exactly one of them had work; the other two go back to sleep, so the
    // single scripted reply is enough to prove which.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    while h.client.remaining() > 0 {
        assert!(
            tokio::time::Instant::now() < deadline,
            "the agent with a message waiting never took its turn"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    for id in [root.id, child.id] {
        assert_eq!(
            h.store.agent(id).expect("a").expect("e").stats.turns,
            0,
            "an agent with nothing waiting takes no turn on resume"
        );
    }
    assert_eq!(
        h.store
            .agent(grandchild.id)
            .expect("a")
            .expect("e")
            .inbox_cursor,
        1
    );
    assert!(
        h.runtime
            .resume_agent(root.id, true, Actor::user())
            .is_err(),
        "resuming a subtree with nothing paused says so"
    );
}

/// A paused root's turn in flight still finishes, and ending does not take
/// its subtree's pause with it: the subtree resume lifts it from the ended
/// root, whose own stale marker goes too.
#[tokio::test]
async fn a_subtree_resume_works_from_a_root_that_ended_while_paused() {
    let h = harness(vec![ScriptedClient::tool_call(
        "complete_task",
        json!({"summary": "done"}),
    )])
    .await;
    let gate = h.client.hold_response_at(1);
    let (root, _) = h
        .runtime
        .create_agent(spec("finish up"), Actor::user())
        .expect("root");
    wait_for_event(h.store.as_ref(), root.id, "snapshot.created").await;
    let mut child_spec = idle_spec();
    child_spec.parent = Some(root.id);
    let (child, _) = h
        .runtime
        .create_agent(child_spec, Actor::user())
        .expect("child");
    h.runtime
        .pause_agent(root.id, true, Actor::user())
        .expect("pause subtree");
    gate.cancel();
    let status = h.wait_until(root.id, |s| s.is_terminal()).await;
    assert_eq!(status.kind(), AgentStatusKind::Completed);
    assert!(paused(h.store.as_ref(), root.id));
    assert!(paused(h.store.as_ref(), child.id));

    let resumed = h
        .runtime
        .resume_agent(root.id, true, Actor::user())
        .expect("resume from the ended root");
    let ids: Vec<_> = resumed.iter().map(|a| a.id).collect();
    assert_eq!(ids, vec![root.id, child.id]);
    for id in [root.id, child.id] {
        assert!(!paused(h.store.as_ref(), id));
    }
    assert_eq!(
        h.store.agent(root.id).expect("a").expect("e").status.kind(),
        AgentStatusKind::Completed,
        "lifting the marker does not revive the root"
    );
}

/// `resume --subtree` lifts every pause beneath its root, whoever stamped it.
#[tokio::test]
async fn a_mid_tree_subtree_resume_lifts_pauses_an_ancestor_stamped() {
    let h = harness(vec![]).await;
    let (root, _) = h
        .runtime
        .create_agent(idle_spec(), Actor::user())
        .expect("root");
    let mut child_spec = idle_spec();
    child_spec.parent = Some(root.id);
    let (child, _) = h
        .runtime
        .create_agent(child_spec, Actor::user())
        .expect("child");
    let mut grandchild_spec = idle_spec();
    grandchild_spec.parent = Some(child.id);
    let (grandchild, _) = h
        .runtime
        .create_agent(grandchild_spec, Actor::user())
        .expect("grandchild");
    h.runtime
        .pause_agent(root.id, true, Actor::user())
        .expect("subtree pause");
    let resumed = h
        .runtime
        .resume_agent(child.id, true, Actor::user())
        .expect("mid-tree subtree resume");
    assert_eq!(
        resumed.iter().map(|a| a.id).collect::<Vec<_>>(),
        vec![child.id, grandchild.id]
    );
    assert!(
        paused(h.store.as_ref(), root.id),
        "the root was not resumed"
    );
    for id in [child.id, grandchild.id] {
        assert!(!paused(h.store.as_ref(), id), "{id} is still paused");
    }
    h.runtime.shutdown().await;
}

#[tokio::test]
async fn root_subtree_resume_also_lifts_a_childs_independent_subtree_pause() {
    let h = harness(vec![]).await;
    let (root, _) = h.runtime.create_agent(idle_spec(), Actor::user()).unwrap();
    let mut child_spec = idle_spec();
    child_spec.parent = Some(root.id);
    let (child, _) = h.runtime.create_agent(child_spec, Actor::user()).unwrap();
    let mut grandchild_spec = idle_spec();
    grandchild_spec.parent = Some(child.id);
    let (grandchild, _) = h
        .runtime
        .create_agent(grandchild_spec, Actor::user())
        .unwrap();
    for id in [root.id, child.id, grandchild.id] {
        h.wait_until(id, |s| s.kind() == AgentStatusKind::Waiting)
            .await;
    }
    h.runtime.pause_agent(root.id, true, Actor::user()).unwrap();
    h.runtime
        .pause_agent(child.id, true, Actor::user())
        .unwrap();
    for id in [child.id, grandchild.id] {
        let agent = h.store.agent(id).unwrap().unwrap();
        assert!(agent.paused);
        assert_eq!(agent.paused_via, Some(child.id));
    }
    let resumed = h
        .runtime
        .resume_agent(root.id, true, Actor::user())
        .unwrap();
    assert_eq!(
        resumed.iter().map(|a| a.id).collect::<Vec<_>>(),
        vec![root.id, child.id, grandchild.id]
    );
    for id in [root.id, child.id, grandchild.id] {
        let agent = h.store.agent(id).unwrap().unwrap();
        assert!(!agent.paused);
        assert_eq!(agent.paused_via, None);
        assert_eq!(agent.status.kind(), AgentStatusKind::Waiting);
    }
    h.runtime.shutdown().await;
}
