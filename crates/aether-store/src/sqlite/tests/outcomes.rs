use super::*;

#[test]
fn a_recorded_outcome_answers_only_its_own_invocation() {
    let store = SqliteStore::open_in_memory().expect("open");
    let agent = AgentId::new();
    let driven = InvocationId::new();
    let later = InvocationId::new();
    let call = ToolCallId::new();
    let key = "bash:h:deadbeef";
    store
        .commit(Commit::new().mutate(Mutation::RecordToolOutcome(recorded(
            agent, driven, call, key, "first",
        ))))
        .expect("record");

    let replayed = store
        .recorded_outcome(agent, driven, call, &IdempotencyKey(key.into()))
        .expect("read")
        .expect("the driven invocation replays");
    assert_eq!(replayed.outcome.content, "first");
    assert!(
        store
            .recorded_outcome(agent, later, call, &IdempotencyKey(key.into()))
            .expect("read")
            .is_none(),
        "a later invocation is a fresh call and must run"
    );
    assert!(
        store
            .recorded_outcome(AgentId::new(), driven, call, &IdempotencyKey(key.into()))
            .expect("read")
            .is_none(),
        "another agent's effect is its own"
    );
}

#[test]
fn a_record_answers_only_the_call_it_was_recorded_for() {
    let store = SqliteStore::open_in_memory().expect("open");
    let agent = AgentId::new();
    let invocation = InvocationId::new();
    let (first, second) = (ToolCallId::new(), ToolCallId::new());
    let key = "bash:h:deadbeef";
    store
        .commit(Commit::new().mutate(Mutation::RecordToolOutcome(recorded(
            agent, invocation, first, key, "first",
        ))))
        .expect("record");
    assert!(
        store
            .recorded_outcome(agent, invocation, second, &IdempotencyKey(key.into()))
            .expect("read")
            .is_none(),
        "a second identical call in one response is a second call and must run"
    );
    // The re-drive of the recorded call keeps its id, so it still replays.
    assert_eq!(
        store
            .recorded_outcome(agent, invocation, first, &IdempotencyKey(key.into()))
            .expect("read")
            .expect("recorded")
            .outcome
            .content,
        "first"
    );
}

#[test]
fn the_first_write_for_a_key_wins() {
    let store = SqliteStore::open_in_memory().expect("open");
    let agent = AgentId::new();
    let invocation = InvocationId::new();
    let call = ToolCallId::new();
    let key = "bash:h:deadbeef";
    for content in ["first", "second"] {
        store
            .commit(Commit::new().mutate(Mutation::RecordToolOutcome(recorded(
                agent, invocation, call, key, content,
            ))))
            .expect("record");
    }
    let found = store
        .recorded_outcome(agent, invocation, call, &IdempotencyKey(key.into()))
        .expect("read")
        .expect("recorded");
    assert_eq!(
        found.outcome.content, "first",
        "a replay can never overwrite what it replays"
    );
}
