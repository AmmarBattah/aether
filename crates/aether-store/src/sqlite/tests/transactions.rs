use super::*;

/// The importer mints event ids so a brought `causation` can be resolved
/// within one file, which means it is the importer's word that they are
/// unique. `events.id` is UNIQUE and the whole import is one transaction,
/// so a collision fails loudly and writes nothing, rather than silently
/// overwriting an event this store already had.
#[test]
fn a_brought_event_id_that_collides_fails_and_writes_nothing() {
    let store = SqliteStore::open_in_memory().expect("store");
    let run = RunId::new();
    let id = EventId::new();
    let brought = |id| ImportedEvent {
        id,
        at: now(),
        draft: EventDraft::new(Actor::System, EventKind::AgentReady).run(run),
    };
    store
        .import(vec![brought(id)], Vec::new())
        .expect("the first import lands");

    let before = store.last_seq().expect("seq");
    let error = store
        .import(
            vec![brought(EventId::new()), brought(id)],
            vec![Mutation::UpsertAgent(agent())],
        )
        .expect_err("a colliding id must fail");
    assert!(
        error.to_string().to_ascii_lowercase().contains("unique"),
        "{error}"
    );
    assert_eq!(store.last_seq().expect("seq"), before, "the seq moved");
    assert_eq!(
        store
            .events(0, 100, &EventFilter::for_run(run))
            .expect("events")
            .len(),
        1,
        "the fresh event of the failed import landed anyway"
    );
    assert!(
        store
            .agents(&AgentFilter::default())
            .expect("agents")
            .is_empty(),
        "the failed import's mutation landed anyway"
    );
}

#[test]
fn commit_assigns_gapless_sequence_and_persists_state() {
    let store = SqliteStore::open_in_memory().expect("open");
    let a = agent();
    let events = store
        .commit(
            Commit::new()
                .event(EventDraft::new(Actor::System, EventKind::AgentReady).agent(a.id))
                .event(
                    EventDraft::new(Actor::System, EventKind::AgentStarted { turn: 1 }).agent(a.id),
                )
                .mutate(Mutation::UpsertAgent(a.clone())),
        )
        .expect("commit");
    assert_eq!(events.iter().map(|e| e.seq).collect::<Vec<_>>(), vec![1, 2]);
    assert_eq!(store.last_seq().expect("seq"), 2);
    assert_eq!(store.agent(a.id).expect("agent"), Some(a.clone()));
    let more = store
        .commit(Commit::new().event(EventDraft::new(Actor::System, EventKind::AgentReady)))
        .expect("commit");
    assert_eq!(more[0].seq, 3);
    assert_eq!(
        store
            .events(1, 10, &EventFilter::for_agent(a.id))
            .expect("events")
            .len(),
        1
    );
}

#[test]
fn status_since_and_status_roll_back_together_on_commit_failure() {
    let store = SqliteStore::open_in_memory().unwrap();
    let original = agent();
    store
        .commit(Commit::new().mutate(Mutation::UpsertAgent(original.clone())))
        .unwrap();
    let mut changed = original.clone();
    changed.transition(AgentStatus::Ready, now()).unwrap();
    let failed = store.commit(
        Commit::new()
            .mutate(Mutation::UpsertAgent(changed))
            .mutate(Mutation::DeliverInbox(inbox_item(original.id, 5))),
    );
    assert!(matches!(failed, Err(StoreError::InboxSequence { .. })));
    assert_eq!(store.agent(original.id).unwrap(), Some(original));
}

#[test]
fn failed_mutation_rolls_back_events() {
    let store = SqliteStore::open_in_memory().expect("open");
    let a = agent();
    let err = store
        .commit(
            Commit::new()
                .event(EventDraft::new(Actor::System, EventKind::AgentReady).agent(a.id))
                .mutate(Mutation::DeliverInbox(inbox_item(a.id, 5))),
        )
        .unwrap_err();
    assert!(matches!(
        err,
        StoreError::InboxSequence {
            expected: 1,
            got: 5,
            ..
        }
    ));
    assert_eq!(store.last_seq().expect("seq"), 0);
    assert!(
        store
            .events(0, 10, &EventFilter::all())
            .expect("events")
            .is_empty()
    );
}

#[test]
fn status_since_survives_durable_reopen_without_backfilling_legacy_rows() {
    let dir = tempfile::tempdir().expect("tmp");
    let path = dir.path().join("status.db");
    let old = agent();
    let mut known = agent();
    known.status_since = Some(known.created_at - chrono::Duration::minutes(4));
    known.updated_at += chrono::Duration::hours(1);
    {
        let store = SqliteStore::open(&path).expect("open");
        store
            .commit(
                Commit::new()
                    .mutate(Mutation::UpsertAgent(old.clone()))
                    .mutate(Mutation::UpsertAgent(known.clone())),
            )
            .expect("commit");
    }
    let store = SqliteStore::open(&path).expect("reopen");
    assert_eq!(store.agent(old.id).unwrap().unwrap().status_since, None);
    assert_eq!(
        store.agent(known.id).unwrap().unwrap().status_since,
        known.status_since
    );
}
