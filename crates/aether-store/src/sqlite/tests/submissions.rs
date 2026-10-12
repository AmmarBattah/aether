use super::*;

/// The commit lock keeps two identical submissions from both getting
/// here; the primary key is what makes it impossible if one ever does.
/// Failing the transaction is the point: the creation it belongs to must
/// not become durable beside the first one's.
#[test]
fn a_second_record_under_one_key_fails_its_whole_commit() {
    let store = SqliteStore::open_in_memory().expect("open");
    let a = agent();
    store
        .commit(Commit::new().mutate(Mutation::RecordSubmission(submission("k", now()))))
        .expect("first");
    let clash = store.commit(
        Commit::new()
            .mutate(Mutation::UpsertAgent(a.clone()))
            .mutate(Mutation::RecordSubmission(submission("k", now()))),
    );
    assert!(clash.is_err(), "a duplicate key must not be swallowed");
    assert!(
        store.agent(a.id).expect("agent").is_none(),
        "the duplicate's own mutations rolled back with it"
    );
}

#[test]
fn submission_retention_drops_the_aged_and_then_the_overflow() {
    let store = SqliteStore::open_in_memory().expect("open");
    let at = now();
    let stale = at - chrono::Duration::days(30);
    for (key, when) in [("old-1", stale), ("old-2", stale), ("new-1", at)] {
        store
            .commit(Commit::new().mutate(Mutation::RecordSubmission(submission(key, when))))
            .expect("record");
    }
    // Recording "new-1" swept the window itself: nothing older than the
    // retention days survives a write.
    assert!(
        store
            .submission(&IdempotencyKey("old-1".into()))
            .expect("read")
            .is_none()
    );
    assert!(
        store
            .submission(&IdempotencyKey("new-1".into()))
            .expect("read")
            .is_some()
    );

    // The size cap is the other half. Three fresh records, room for one.
    for key in ["new-2", "new-3"] {
        store
            .commit(Commit::new().mutate(Mutation::RecordSubmission(submission(
                key,
                at + chrono::Duration::seconds(1),
            ))))
            .expect("record");
    }
    let sweep = store
        .prune_submissions(at - chrono::Duration::days(7), 1)
        .expect("prune");
    assert_eq!(sweep.evicted, 2, "the oldest go first");
    assert_eq!(sweep.aged, 0, "nothing here is outside the window");
    assert!(
        store
            .submission(&IdempotencyKey("new-1".into()))
            .expect("read")
            .is_none()
    );
    assert_eq!(
        store
            .prune_submissions(at - chrono::Duration::days(7), 1)
            .expect("prune again"),
        SubmissionSweep::default(),
        "pruning a table already at the cap drops nothing"
    );
}

/// The size pass only ever evicts records the age pass left behind, so
/// every eviction shortens the retry window for whoever recorded them.
/// It must never be silent: a warn and one ledger event, in the same
/// transaction as the record that pushed the table over.
#[test]
fn evicting_a_live_record_is_reported_on_the_ledger() {
    let cap = 4;
    let store = SqliteStore::open_in_memory_capped(cap).expect("open");
    let at = now();
    let every = aether_core::idempotency::SUBMISSION_SIZE_PASS_EVERY;
    let mut evictions = Vec::new();
    for i in 0..every {
        let events = store
            .commit(Commit::new().mutate(Mutation::RecordSubmission(submission(
                &format!("k-{i:04}"),
                at + chrono::Duration::milliseconds(i as i64),
            ))))
            .expect("record");
        evictions.extend(events.into_iter().filter(
            |e| matches!(&e.kind, EventKind::Custom { name, .. } if name == "submissions.evicted"),
        ));
    }
    assert_eq!(
        evictions.len(),
        1,
        "one size pass ran, so exactly one eviction is reported"
    );
    let EventKind::Custom { payload, .. } = &evictions[0].kind else {
        panic!("checked above");
    };
    assert_eq!(
        payload["n"].as_u64(),
        Some(every - cap as u64),
        "the event names how many records were evicted"
    );
    assert_eq!(
        payload["oldest_at"].as_str(),
        Some(submission_time(at).as_str()),
        "and how far back the eviction reached"
    );
    let survivors: i64 = store
        .conn
        .lock()
        .query_row("SELECT COUNT(*) FROM submissions", [], |r| r.get(0))
        .expect("count");
    assert_eq!(survivors, cap as i64, "the table is back at the cap");
}

/// The size pass runs on a table the daemon has been writing to for a
/// week, inside the commit transaction, under the global write lock. It
/// must therefore cost the overflow and not the table: the delete's plan
/// has to walk the `(at, key)` index, never sort the table into an
/// ephemeral one. Measured at 10k rows, the `NOT IN (... LIMIT cap)`
/// spelling this replaced took 24 ms per single-row eviction; this one
/// takes 0.014 ms.
#[test]
fn the_size_pass_reads_only_the_overflow() {
    let store = SqliteStore::open_in_memory().expect("open");
    {
        let conn = store.conn.lock();
        let tx = conn.unchecked_transaction().expect("tx");
        for i in 0..10_000u32 {
            tx.execute(
                "INSERT INTO submissions(key, fingerprint, at, body) VALUES (?1, ?2, ?3, '{}')",
                params![
                    format!("k-{i:05}"),
                    "fp",
                    submission_time(now() + chrono::Duration::milliseconds(i as i64))
                ],
            )
            .expect("insert");
        }
        tx.commit().expect("commit");
    }
    let conn = store.conn.lock();
    let plan: Vec<String> = conn
        .prepare(&format!(
            "EXPLAIN QUERY PLAN {}",
            SqliteStore::OVERFLOW_DELETE_SQL
        ))
        .expect("prepare")
        .query_map([100], |r| r.get::<_, String>(3))
        .expect("plan")
        .collect::<std::result::Result<_, _>>()
        .expect("rows");
    assert!(
        plan.iter()
            .any(|step| step.contains("COVERING INDEX submissions_at")),
        "the overflow is read from the (at, key) index: {plan:?}"
    );
    assert!(
        !plan.iter().any(|step| step.contains("TEMP B-TREE")),
        "no sort of the whole table to find the oldest rows: {plan:?}"
    );
    assert!(
        !plan.iter().any(|step| step == "SCAN submissions"),
        "and no bare table scan: {plan:?}"
    );
}
