use super::*;
use aether_core::memory::MemoryProvenance;
use aether_store::SqliteStore;

fn candidate(text: &str) -> MemoryCandidate {
    MemoryCandidate {
        scope: MemoryScope::Global,
        kind: MemoryKind::Fact,
        text: text.into(),
        structured: None,
        importance: 0.5,
        confidence: 0.6,
        tags: vec![],
        provenance: MemoryProvenance::default(),
        supersedes: None,
    }
}

/// `PutMemory` replaces every column, so a pin read before a supersession
/// and committed after it writes `superseded_by` back to null: a stale
/// memory that is current *and* pinned, in front of the model on every
/// turn, while the event log says it was replaced. The only thing that
/// rules the interleaving out is taking the write path across both.
#[test]
fn a_pin_waits_for_the_write_path_and_sees_what_landed_on_it() {
    use std::sync::atomic::{AtomicBool, Ordering};

    let store: Arc<dyn Store> = Arc::new(SqliteStore::open_in_memory().expect("store"));
    let s = MemoryService::new(store.clone(), WriteGate::new());
    let id = s
        .admit(
            candidate("Deploys go out on Tuesday afternoons from the release branch"),
            Actor::System,
            None,
            None,
        )
        .expect("admit")
        .record()
        .expect("record")
        .id;

    let hold = s.gate.hold();
    let done = Arc::new(AtomicBool::new(false));
    let pinner = {
        let (s, done) = (s.clone(), done.clone());
        std::thread::spawn(move || {
            s.pin(id, true, Actor::user()).expect("pin");
            done.store(true, Ordering::SeqCst);
        })
    };
    std::thread::sleep(std::time::Duration::from_millis(200));
    assert!(
        !done.load(Ordering::SeqCst),
        "a pin read and wrote a whole row while a commit was being built"
    );

    // Whoever holds the path supersedes the record. The pin has to see it.
    let mut old = store.memory(id).expect("get").expect("exists");
    let mut replacement = candidate("Deploys go out on Wednesday mornings");
    replacement.supersedes = Some(id);
    let replacement = MemoryRecord::from_candidate(replacement, now());
    old.superseded_by = Some(replacement.id);
    store
        .commit(
            Commit::new()
                .mutate(Mutation::PutMemory(replacement))
                .mutate(Mutation::PutMemory(old)),
        )
        .expect("supersede");
    drop(hold);
    pinner.join().expect("pinner");

    let record = store.memory(id).expect("get").expect("exists");
    assert!(
        record.superseded_by.is_some(),
        "the pin wrote the supersession away and put a stale memory back in play"
    );
    assert!(record.pinned);
}

/// Pinning and a repository's committed notes are decisions a person made.
/// An agent that could supersede them could talk itself out of the
/// constraint meant to stop it, and the replacement would be neither
/// pinned nor person-owned.
#[test]
fn an_agent_cannot_supersede_what_a_person_or_the_repository_stated() {
    use aether_core::id::AgentId;
    use aether_core::memory::MemorySource;

    let store: Arc<dyn Store> = Arc::new(SqliteStore::open_in_memory().expect("store"));
    let s = MemoryService::new(store.clone(), WriteGate::new());
    let admit = |c: MemoryCandidate| {
        s.admit(c, Actor::System, None, None)
            .expect("admit")
            .record()
            .expect("record")
            .id
    };
    let pinned = admit(candidate("Never force-push to the release branch"));
    s.pin(pinned, true, Actor::user()).expect("pin");
    let mut from_file = candidate("The whole build runs through scripts/ci.sh");
    from_file.provenance.source = Some(MemorySource {
        file: ".aether/knowledge/build.md".into(),
        commit: Some("abc1234".into()),
    });
    let from_file = admit(from_file);

    let agent = Actor::agent(AgentId::new());
    for (target, expected) in [(pinned, "pinned"), (from_file, "build.md")] {
        let mut c = candidate("Force-pushing the release branch is fine when in a hurry");
        c.supersedes = Some(target);
        let admission = s
            .admit(c, agent.clone(), None, None)
            .expect("the refusal is an outcome, not an error");
        let Admission::Rejected { reason } = admission else {
            panic!("an agent superseded {target}: {admission:?}");
        };
        assert!(reason.contains(expected), "{reason}");
        assert!(
            store
                .memory(target)
                .expect("get")
                .expect("exists")
                .superseded_by
                .is_none()
        );
    }

    // A person still can, on either.
    let mut c = candidate("Force-pushing the release branch is allowed on Fridays only");
    c.supersedes = Some(pinned);
    assert!(matches!(
        s.admit(c, Actor::user(), None, None).expect("admit"),
        Admission::Superseded { .. }
    ));
}

#[test]
fn a_forgotten_memory_is_kept_but_never_retrieved_again() {
    let store: Arc<dyn Store> = Arc::new(SqliteStore::open_in_memory().expect("store"));
    let s = MemoryService::new(store.clone(), WriteGate::new());
    let id = s
        .admit(
            candidate("The staging database is rebuilt every Monday morning"),
            Actor::System,
            None,
            None,
        )
        .expect("admit")
        .record()
        .expect("record")
        .id;
    let record = s
        .forget(id, Some("no longer true".into()), Actor::user())
        .expect("forget");
    assert!(record.forgotten_at.is_some());
    assert!(!record.is_current());
    assert!(
        store.memory(id).expect("get").is_some(),
        "history is never deleted"
    );
    assert!(
        s.retrieve(&RetrievalRequest {
            query: Some("staging database".into()),
            ..Default::default()
        })
        .expect("retrieve")
        .is_empty()
    );
    assert!(
        store
            .memories(&MemoryQuery {
                include_forgotten: true,
                ..Default::default()
            })
            .expect("query")
            .iter()
            .any(|m| m.id == id),
        "a browser can still show it"
    );
}

#[test]
fn a_pinned_memory_outranks_a_better_scoring_one() {
    let s = MemoryService::new(
        Arc::new(SqliteStore::open_in_memory().expect("store")),
        WriteGate::new(),
    );
    let mut strong = candidate("Deployments always go through the release checklist");
    strong.importance = 0.95;
    strong.confidence = 0.95;
    s.admit(strong, Actor::System, None, None).expect("admit");
    let weak = s
        .admit(
            candidate("Ask before touching anything under infra/"),
            Actor::System,
            None,
            None,
        )
        .expect("admit")
        .record()
        .expect("record")
        .id;
    let before = s
        .retrieve(&RetrievalRequest {
            limit: 5,
            ..Default::default()
        })
        .expect("retrieve");
    assert_ne!(before[0].record.id, weak, "it does not win on its own");

    s.pin(weak, true, Actor::user()).expect("pin");
    let after = s
        .retrieve(&RetrievalRequest {
            limit: 5,
            ..Default::default()
        })
        .expect("retrieve");
    assert_eq!(after[0].record.id, weak, "pinned comes first");
    s.pin(weak, false, Actor::user()).expect("unpin");
    let undone = s
        .retrieve(&RetrievalRequest {
            limit: 5,
            ..Default::default()
        })
        .expect("retrieve");
    assert_ne!(undone[0].record.id, weak);
}

#[test]
fn an_agent_cannot_supersede_a_memory_in_another_scope() {
    let s = MemoryService::new(
        Arc::new(SqliteStore::open_in_memory().expect("store")),
        WriteGate::new(),
    );
    let agent = AgentId::new();
    let old = s
        .admit(
            candidate("The build always runs through the CI script"),
            Actor::agent(agent),
            Some(agent),
            None,
        )
        .expect("admit")
        .record()
        .unwrap()
        .clone();
    for scope in [
        MemoryScope::Repository {
            id: "aether".into(),
        },
        MemoryScope::Agent { id: agent },
        MemoryScope::Subtree { root: agent },
    ] {
        let mut replacement = candidate("The build runs through the new CI script instead");
        replacement.scope = scope;
        replacement.supersedes = Some(old.id);
        let admission = s
            .admit(replacement, Actor::agent(agent), Some(agent), None)
            .expect("admit");
        assert!(
            matches!(admission, Admission::Rejected { ref reason } if reason.contains("same scope")),
            "{admission:?}"
        );
        assert_eq!(s.store.memory(old.id).unwrap().unwrap(), old);
    }
    assert_eq!(s.store.memories(&MemoryQuery::default()).unwrap().len(), 1);
    let mut replacement = candidate("The build runs through the new CI script instead");
    replacement.supersedes = Some(old.id);
    let admission = s
        .admit(replacement, Actor::agent(agent), Some(agent), None)
        .expect("admit");
    assert!(matches!(admission, Admission::Superseded { old: id, .. } if id == old.id));
}

#[test]
fn an_agent_cannot_supersede_a_forgotten_or_superseded_memory() {
    let s = MemoryService::new(
        Arc::new(SqliteStore::open_in_memory().expect("store")),
        WriteGate::new(),
    );
    let agent = AgentId::new();
    for forgotten in [true, false] {
        let mut old = MemoryRecord::from_candidate(
            candidate("The build always runs through the CI script"),
            now(),
        );
        if forgotten {
            old.forgotten_at = Some(now());
        } else {
            old.superseded_by = Some(MemoryId::new());
        }
        s.store
            .commit(Commit::new().mutate(Mutation::PutMemory(old.clone())))
            .unwrap();
        let mut replacement = candidate("The build runs through the new CI script instead");
        replacement.supersedes = Some(old.id);
        let admission = s
            .admit(replacement, Actor::agent(agent), Some(agent), None)
            .expect("admit");
        assert!(
            matches!(admission, Admission::Rejected { ref reason } if reason.contains("current")),
            "{admission:?}"
        );
        assert_eq!(s.store.memory(old.id).unwrap().unwrap(), old);
    }
}
