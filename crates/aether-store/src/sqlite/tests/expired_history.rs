use super::*;
use aether_core::id::TaskId;

fn fixture() -> (SqliteStore, RunId, RunId, AgentId, AgentId, TaskId) {
    fixture_with(EventKind::Custom {
        name: "old".into(),
        payload: serde_json::json!({"private": "payload"}),
    })
}

fn fixture_with(kind: EventKind) -> (SqliteStore, RunId, RunId, AgentId, AgentId, TaskId) {
    let store = SqliteStore::open_in_memory().expect("store");
    let (expired, retained) = (RunId::new(), RunId::new());
    let (parent, child) = (AgentId::new(), AgentId::new());
    let task = TaskId::new();
    for run in [expired, retained, expired, retained] {
        let draft = if run == expired {
            EventDraft::new(Actor::System, kind.clone())
                .agent(child)
                .parent(Some(parent))
                .task(task)
                .run(run)
        } else {
            EventDraft::new(Actor::System, EventKind::AgentStarted { turn: 1 })
                .agent(parent)
                .run(run)
        };
        store.commit(Commit::new().event(draft)).expect("event");
    }
    // Simulate atomic expiry only inside this disposable database. Production
    // eligibility and deletion are not exercised by these read-path tests.
    let mut conn = store.conn.lock();
    let tx = conn.transaction().expect("transaction");
    tx.execute("INSERT INTO expired_event_spans SELECT run, COALESCE(agent_id,''), COALESCE(parent_agent_id,''), COALESCE(task_id,''), type, family, MIN(seq), MAX(seq) FROM events WHERE run = ?1 GROUP BY run, agent_id, parent_agent_id, task_id, type, family", [expired.to_string()]).expect("expiry spans");
    tx.execute("DELETE FROM events WHERE run = ?1", [expired.to_string()])
        .expect("expire fixture events");
    tx.commit().expect("commit");
    drop(conn);
    (store, expired, retained, parent, child, task)
}

#[test]
fn replay_never_silently_skips_expired_history() {
    let (store, expired, _, _, _, _) = fixture();
    for filter in [EventFilter::all(), EventFilter::for_run(expired)] {
        assert!(matches!(
            store.events(0, 100, &filter),
            Err(StoreError::HistoryExpired {
                first_seq: 1,
                last_seq: 3
            })
        ));
    }
    assert_eq!(store.last_seq().unwrap(), 4);
    assert!(matches!(
        store.event_by_seq(1),
        Err(StoreError::HistoryExpired { .. })
    ));
    assert_eq!(store.event_by_seq(2).unwrap().unwrap().seq, 2);
    assert!(store.event_by_seq(5).unwrap().is_none());
}

#[test]
fn recent_pages_and_unaffected_runs_remain_readable() {
    let (store, _, retained, _, _, _) = fixture();
    let seqs = |events: Vec<Event>| events.into_iter().map(|e| e.seq).collect::<Vec<_>>();
    assert_eq!(
        seqs(store.events(3, 100, &EventFilter::all()).unwrap()),
        vec![4]
    );
    assert_eq!(
        seqs(store.latest_events(1, &EventFilter::all()).unwrap()),
        vec![4]
    );
    assert_eq!(
        seqs(
            store
                .events(0, 100, &EventFilter::for_run(retained))
                .unwrap()
        ),
        vec![2, 4]
    );
    assert!(matches!(
        store.latest_events(2, &EventFilter::all()),
        Err(StoreError::HistoryExpired { .. })
    ));
}

#[test]
fn parent_filters_include_expired_children_but_own_filters_do_not() {
    let (store, _, _, parent, child, _) = fixture();
    for agent in [parent, child] {
        assert!(matches!(
            store.events(0, 100, &EventFilter::for_agent(agent)),
            Err(StoreError::HistoryExpired { .. })
        ));
    }
    assert_eq!(
        store
            .events(0, 100, &EventFilter::own_events(parent))
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn task_family_and_type_filters_apply_to_expiry_spans() {
    let (store, _, _, _, _, task) = fixture();
    for filter in [
        EventFilter {
            task: Some(task),
            ..Default::default()
        },
        EventFilter {
            types: vec!["custom".into()],
            ..Default::default()
        },
        EventFilter {
            families: vec!["custom".into()],
            ..Default::default()
        },
    ] {
        assert!(matches!(
            store.events(0, 100, &filter),
            Err(StoreError::HistoryExpired { .. })
        ));
    }
    let unrelated = EventFilter {
        types: vec!["agent.started".into()],
        ..Default::default()
    };
    assert_eq!(store.events(0, 100, &unrelated).unwrap().len(), 2);
}

#[test]
fn zero_limit_and_out_of_range_cursor_are_empty_without_wrapping() {
    let (store, _, _, _, _, _) = fixture();
    assert!(store.events(0, 0, &EventFilter::all()).unwrap().is_empty());
    assert!(
        store
            .events(u64::MAX, 10, &EventFilter::all())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn expired_transcript_is_not_reported_as_an_empty_conversation() {
    let (store, _, _, parent, child, _) = fixture_with(EventKind::AgentMessage {
        role: MessageRole::Assistant,
        content: vec![ContentBlock::text("old message")],
        invocation: None,
    });
    assert!(matches!(
        store.transcript(child, 0),
        Err(StoreError::HistoryExpired { .. })
    ));
    assert!(store.transcript(parent, 0).unwrap().is_empty());
    assert!(store.transcript(child, 3).unwrap().is_empty());
}

#[test]
fn before_pages_check_expiry_only_inside_the_requested_window() {
    let (store, _, retained, _, _, _) = fixture();
    let filter = EventFilter::all();
    assert_eq!(store.events_before(5, 1, &filter).unwrap()[0].seq, 4);
    assert!(matches!(
        store.events_before(4, 1, &filter),
        Err(StoreError::HistoryExpired { .. })
    ));
    assert!(matches!(
        store.events_before(3, 2, &filter),
        Err(StoreError::HistoryExpired { .. })
    ));
    assert!(store.events_before(1, 100, &filter).unwrap().is_empty());
    let events = store
        .events_before(3, 100, &EventFilter::for_run(retained))
        .unwrap();
    assert_eq!(events.iter().map(|e| e.seq).collect::<Vec<_>>(), [2]);
}

#[test]
fn child_types_check_expiry_only_for_the_child_types_requested() {
    let (store, _, _, parent, _, _) = fixture();
    let with_children = |types: &[&str]| EventFilter {
        agent: Some(parent),
        child_types: types.iter().map(|t| t.to_string()).collect(),
        ..Default::default()
    };
    let shown = store
        .events_before(u64::MAX, 100, &with_children(&["agent.completed"]))
        .expect("no expired child event of a requested type");
    assert_eq!(shown.iter().map(|e| e.seq).collect::<Vec<_>>(), [4, 2]);
    for types in [&[][..], &["custom"][..]] {
        assert!(matches!(
            store.events_before(u64::MAX, 100, &with_children(types)),
            Err(StoreError::HistoryExpired { .. })
        ));
    }
}
