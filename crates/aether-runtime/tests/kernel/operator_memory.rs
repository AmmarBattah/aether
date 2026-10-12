//! operator memory scenarios and fixtures.
use super::*;

fn operator_memory(
    scope: aether_core::memory::OperatorMemoryScope,
) -> aether_core::memory::OperatorMemorySpec {
    aether_core::memory::OperatorMemorySpec {
        text: "Use the repository verification script before publishing changes.".into(),
        kind: aether_core::memory::MemoryKind::Fact,
        scope,
        importance: None,
        confidence: None,
        tags: vec!["verification".into()],
        supersedes: None,
    }
}

#[tokio::test]
async fn operator_memory_scopes_authorship_and_admission_are_truthful() {
    use aether_core::memory::{MemoryScope, OperatorMemoryScope};
    let actor = Actor::connected_user(aether_core::event::OperatorOrigin::LocalSocket {
        connection: "authenticated-connection".into(),
    });
    let h = harness(Vec::new()).await;
    let (agent, _) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                start: false,
                ..spec("scope target")
            },
            Actor::user(),
        )
        .unwrap();
    let scopes = [
        OperatorMemoryScope::Agent { id: agent.id },
        OperatorMemoryScope::Global,
        OperatorMemoryScope::Repository {
            root: Some(h.workspace_root.clone()),
        },
    ];
    for scope in scopes {
        let spec = operator_memory(scope.clone());
        let before = h
            .store
            .events(0, 10000, &EventFilter::default())
            .unwrap()
            .len();
        let record = h
            .runtime
            .create_memory(spec.clone(), actor.clone())
            .unwrap();
        assert_eq!(record.provenance.actor.as_ref(), Some(&actor));
        assert_eq!(record.provenance.agent, None);
        assert_eq!(
            h.store.memory(record.id).unwrap().unwrap().provenance,
            record.provenance
        );
        assert_eq!(record.importance, 0.5);
        assert_eq!(record.confidence, 0.7);
        match scope {
            OperatorMemoryScope::Agent { id } => {
                assert_eq!(record.scope, MemoryScope::Agent { id })
            }
            OperatorMemoryScope::Global => assert_eq!(record.scope, MemoryScope::Global),
            OperatorMemoryScope::Repository { root } => {
                let (id, _) = h
                    .runtime
                    .sync_repository_knowledge(root, Actor::user())
                    .unwrap();
                assert_eq!(record.scope, MemoryScope::Repository { id });
            }
        }
        let events = h.store.events(0, 10000, &EventFilter::default()).unwrap();
        let admitted = &events[before..];
        assert_eq!(admitted.len(), 2);
        assert!(
            admitted
                .iter()
                .all(|e| e.actor == actor && e.agent_id.is_none() && e.task_id.is_none())
        );
        let err = h.runtime.create_memory(spec, Actor::user()).unwrap_err();
        assert!(
            err.to_string().contains("redundant with existing memory"),
            "{err}"
        );
        assert_eq!(
            h.store.events(0, 10000, &EventFilter::default()).unwrap(),
            events
        );
    }
    assert!(
        h.store
            .memories(&aether_store::MemoryQuery {
                provenance_agent: Some(agent.id),
                limit: 100,
                ..Default::default()
            })
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn operator_memory_invalid_input_has_no_records_or_events() {
    use aether_core::memory::OperatorMemoryScope;
    let h = harness(Vec::new()).await;
    let base = operator_memory(OperatorMemoryScope::Global);
    let mut cases = Vec::new();
    for text in [
        "",
        " ",
        "tiny",
        "This is currently a transient temporary observation.",
    ] {
        let mut spec = base.clone();
        spec.text = text.into();
        cases.push(spec);
    }
    let mut long = base.clone();
    long.text = "x".repeat(1201);
    cases.push(long);
    for value in [-0.1, 1.1, f32::NAN, f32::INFINITY, 0.1] {
        let mut importance = base.clone();
        importance.importance = Some(value);
        cases.push(importance);
        let mut confidence = base.clone();
        confidence.confidence = Some(value);
        cases.push(confidence);
    }
    for tags in [
        vec![" ".into()],
        vec!["line\nbreak".into()],
        vec!["x".repeat(65)],
        vec!["tag".into(); 33],
    ] {
        let mut spec = base.clone();
        spec.tags = tags;
        cases.push(spec);
    }
    let mut missing = base.clone();
    missing.scope = OperatorMemoryScope::Agent { id: AgentId::new() };
    cases.push(missing);
    let outside = tempfile::tempdir().unwrap();
    let mut repository = base.clone();
    repository.scope = OperatorMemoryScope::Repository {
        root: Some(outside.path().into()),
    };
    cases.push(repository);
    let mut missing_old = base.clone();
    missing_old.supersedes = Some(aether_core::id::MemoryId::new());
    cases.push(missing_old);
    let before = h.store.events(0, 10000, &EventFilter::default()).unwrap();
    for spec in cases {
        assert!(h.runtime.create_memory(spec, Actor::user()).is_err());
        assert_eq!(
            h.store.events(0, 10000, &EventFilter::default()).unwrap(),
            before
        );
        assert!(
            h.store
                .memories(&aether_store::MemoryQuery {
                    limit: 100,
                    ..Default::default()
                })
                .unwrap()
                .is_empty()
        );
    }
    assert!(
        h.runtime
            .create_memory(base, Actor::agent(AgentId::new()))
            .unwrap_err()
            .to_string()
            .contains("only an operator")
    );
    assert_eq!(
        h.store.events(0, 10000, &EventFilter::default()).unwrap(),
        before
    );
}

#[tokio::test]
async fn operator_memory_supersession_cannot_fork_or_cross_scope() {
    use aether_core::memory::OperatorMemoryScope;
    let h = harness(Vec::new()).await;
    let original = h
        .runtime
        .create_memory(operator_memory(OperatorMemoryScope::Global), Actor::user())
        .unwrap();
    let mut replacement = operator_memory(OperatorMemoryScope::Repository {
        root: Some(h.workspace_root.clone()),
    });
    replacement.supersedes = Some(original.id);
    replacement.text =
        "Run the focused checks first and the complete verification before publishing.".into();
    let before = h.store.events(0, 10000, &EventFilter::default()).unwrap();
    assert!(
        h.runtime
            .create_memory(replacement.clone(), Actor::user())
            .unwrap_err()
            .to_string()
            .contains("same scope")
    );
    assert_eq!(
        h.store.events(0, 10000, &EventFilter::default()).unwrap(),
        before
    );
    replacement.scope = OperatorMemoryScope::Global;
    let record = h
        .runtime
        .create_memory(replacement.clone(), Actor::user())
        .unwrap();
    assert_eq!(record.supersedes, Some(original.id));
    assert_eq!(
        h.store.memory(original.id).unwrap().unwrap().superseded_by,
        Some(record.id)
    );
    let before = h.store.events(0, 10000, &EventFilter::default()).unwrap();
    assert!(
        h.runtime
            .create_memory(replacement, Actor::user())
            .unwrap_err()
            .to_string()
            .contains("must be current")
    );
    assert_eq!(
        h.store.events(0, 10000, &EventFilter::default()).unwrap(),
        before
    );
}

#[tokio::test]
async fn operator_memory_concurrent_retries_admit_exactly_once() {
    use aether_core::memory::OperatorMemoryScope;
    let h = harness(Vec::new()).await;
    let before = h
        .store
        .events(0, 10000, &EventFilter::default())
        .unwrap()
        .len();
    let successes = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let runtime = &h.runtime;
                scope.spawn(move || {
                    runtime
                        .create_memory(operator_memory(OperatorMemoryScope::Global), Actor::user())
                        .is_ok()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| usize::from(handle.join().unwrap()))
            .sum::<usize>()
    });
    assert_eq!(successes, 1);
    assert_eq!(
        h.store
            .memories(&aether_store::MemoryQuery {
                limit: 100,
                ..Default::default()
            })
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        h.store
            .events(0, 10000, &EventFilter::default())
            .unwrap()
            .len(),
        before + 2
    );
}

#[tokio::test]
async fn operator_memory_publishes_committed_events_to_live_subscribers() {
    use aether_core::memory::OperatorMemoryScope;
    let h = harness(Vec::new()).await;
    let mut bus = h.runtime.subscribe();
    let original_spec = operator_memory(OperatorMemoryScope::Global);
    let original = h
        .runtime
        .create_memory(original_spec.clone(), Actor::user())
        .unwrap();
    let mut replacement = original_spec.clone();
    replacement.supersedes = Some(original.id);
    replacement.text =
        "Run deterministic focused tests before the full verification script.".into();
    h.runtime.create_memory(replacement, Actor::user()).unwrap();
    let committed = h.store.events(0, 10000, &EventFilter::default()).unwrap();
    let memory_events: Vec<_> = committed
        .into_iter()
        .filter(|event| event.kind.type_name().starts_with("memory."))
        .collect();
    assert_eq!(memory_events.len(), 5);
    for expected in memory_events {
        let aether_runtime::BusMessage::Event(actual) = bus
            .try_recv()
            .expect("a committed memory event reaches live subscribers")
        else {
            panic!("expected a committed event, not a transient delta");
        };
        assert_eq!(*actual, expected);
    }
    let mut rejected = original_spec;
    rejected.text = "tiny".into();
    assert!(h.runtime.create_memory(rejected, Actor::user()).is_err());
    assert!(matches!(
        bus.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Empty)
    ));
}
