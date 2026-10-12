//! Retrieved memory placement and session retention.
use super::*;
/// Retrieved memory reaches an engine agent the way the assignment does: in
/// the user turn, never the machine-wide `--system-prompt`, and only when the
/// retained session does not already hold the same set.
#[tokio::test]
async fn retrieved_memory_reaches_an_engine_turn_once_per_session() {
    use aether_core::snapshot::LayerPlacement;

    const FACT: &str = "The indexer runs in batches of five hundred rows here";
    let h = harness(Reply::Resumable).await;
    let id = chat_agent(&h);
    let agent = wait_for(h.store.as_ref(), id, |a| {
        a.status.kind() == AgentStatusKind::Waiting
    })
    .await;
    let memory_id = seed_repository_memory(&h, &agent, FACT);

    engine_turn(&h, id, "what should I know about the indexer?").await;
    let first = latest_snapshot(&h, id);
    let first_prompt = h.prompts.lock().unwrap().last().expect("a turn").clone();
    let first_system = h.systems.lock().unwrap().last().expect("a turn").clone();
    assert_eq!(first.memory_ids, vec![memory_id]);
    assert_eq!(memory_layer(&first).placement, LayerPlacement::UserTurn);
    assert!(
        first_prompt.contains(FACT),
        "a stored repository memory reaches the engine: {first_prompt}"
    );
    assert!(
        !first_system.contains(FACT),
        "memory varies per agent, so it stays out of the shared system prompt"
    );
    assert_eq!(
        first_system.len(),
        PREFIX_BYTES,
        "with memory present the cached prefix is still the shared static one"
    );
    let touched = h.store.memory(memory_id).expect("memory").expect("exists");
    assert_eq!(touched.retrieval_count, 1, "the retrieval is recorded");

    engine_turn(&h, id, "carry on").await;
    let second = latest_snapshot(&h, id);
    let second_prompt = h.prompts.lock().unwrap().last().expect("a turn").clone();
    let second_system = h.systems.lock().unwrap().last().expect("a turn").clone();
    assert_eq!(second.memory_ids, vec![memory_id]);
    assert_eq!(memory_layer(&second).placement, LayerPlacement::Retained);
    assert!(
        !second_prompt.contains(FACT),
        "the resumed session already holds an unchanged memory set: {second_prompt}"
    );
    assert_eq!(first_system, second_system, "the system prefix never moves");
    let touched = h.store.memory(memory_id).expect("memory").expect("exists");
    assert_eq!(
        touched.retrieval_count, 2,
        "a retained memory is still in front of the model, so it still counts"
    );
    h.runtime.shutdown().await;
}

/// Two memories that different messages would rank in opposite orders. The
/// engine ranks on the task objective alone, so the rendered layer - and the
/// digest that gates re-sending it - stays put from one message to the next.
#[tokio::test]
async fn engine_memory_does_not_churn_with_each_message() {
    use aether_core::snapshot::LayerPlacement;

    const INDEXER: &str = "The indexer batches rows in groups of five hundred";
    const LEDGER: &str = "The ledger migrations run nightly against staging";
    for model in [
        ModelRef::new("claude-code", "haiku"),
        ModelRef::new("codex", "gpt-6.1-sol"),
    ] {
        let h = harness(Reply::Resumable).await;
        let (agent, _) = h
            .runtime
            .create_agent(
                CreateAgentSpec {
                    role: "assistant".into(),
                    model: Some(model.clone()),
                    start: true,
                    config: Some(AgentConfig {
                        long_lived: true,
                        approval: ApprovalPolicy::none(),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                Actor::user(),
            )
            .expect("create");
        let agent = wait_for(h.store.as_ref(), agent.id, |a| {
            a.status.kind() == AgentStatusKind::Waiting
        })
        .await;
        seed_repository_memory(&h, &agent, INDEXER);
        seed_repository_memory(&h, &agent, LEDGER);

        engine_turn(&h, agent.id, "how do the ledger migrations run?").await;
        let first = latest_snapshot(&h, agent.id);
        let first_system = h.systems.lock().unwrap().last().expect("a turn").clone();
        engine_turn(&h, agent.id, "how does the indexer batch rows?").await;
        let second = latest_snapshot(&h, agent.id);
        let second_system = h.systems.lock().unwrap().last().expect("a turn").clone();
        let second_prompt = h.prompts.lock().unwrap().last().expect("a turn").clone();

        assert_eq!(first.memory_ids.len(), 2, "{model:?}: both memories fit");
        assert_eq!(
            first.memory_ids, second.memory_ids,
            "{model:?}: the memory set and its order do not follow the message"
        );
        assert!(
            !second_prompt.contains(INDEXER) && !second_prompt.contains(LEDGER),
            "{model:?}: an unchanged memory set is not re-sent: {second_prompt}"
        );
        assert_eq!(
            first_system, second_system,
            "{model:?}: the system prompt stays cacheable across messages"
        );
        assert_eq!(memory_layer(&second).placement, LayerPlacement::Retained);
        assert!(second_system.is_empty());
        h.runtime.shutdown().await;
    }
}

/// Store a repository-scoped memory the agent's retrieval will find.
fn seed_repository_memory(
    h: &Harness,
    agent: &aether_core::agent::Agent,
    text: &str,
) -> aether_core::id::MemoryId {
    use aether_core::memory::{MemoryCandidate, MemoryKind, MemoryRecord, MemoryScope};
    use aether_store::{Commit, Mutation};

    let record = MemoryRecord::from_candidate(
        MemoryCandidate {
            scope: MemoryScope::Repository {
                id: agent.scope.repository.clone().expect("repository"),
            },
            kind: MemoryKind::Fact,
            text: text.into(),
            structured: None,
            importance: 0.9,
            confidence: 0.9,
            tags: vec![],
            provenance: Default::default(),
            supersedes: None,
        },
        aether_core::time::now(),
    );
    let id = record.id;
    h.store
        .commit(Commit::new().mutate(Mutation::PutMemory(record)))
        .expect("seed memory");
    id
}

/// The retrieved-memory layer of a stored snapshot.
fn memory_layer(
    snapshot: &aether_core::snapshot::InvocationSnapshot,
) -> aether_core::snapshot::ContextLayer {
    snapshot
        .layers
        .iter()
        .find(|l| l.kind == aether_core::snapshot::LayerKind::RetrievedMemory)
        .expect("memory layer")
        .clone()
}
