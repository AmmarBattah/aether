//! engine slot recovery scenarios and fixtures.
use super::*;

/// Panics where the turn task itself runs. Tool *execution* is already
/// isolated by the batch's `JoinSet`, so the tripwire has to be
/// `leases_resources`, which the turn asks on its own task before the batch.
struct Detonate;

#[async_trait::async_trait]
impl aether_tools::Tool for Detonate {
    fn descriptor(&self) -> aether_core::tool::ToolDescriptor {
        aether_core::tool::ToolDescriptor {
            spec: aether_core::model::ToolSpec {
                name: "detonate".into(),
                description: "panics on the turn task".into(),
                input_schema: json!({"type": "object", "properties": {}}),
                strict: false,
            },
            side_effect: SideEffectClass::Mutation,
            needs_network: false,
        }
    }

    fn resources(
        &self,
        _input: &serde_json::Value,
        _ctx: &aether_tools::ToolContext,
    ) -> Vec<aether_core::tool::ResourceKey> {
        Vec::new()
    }

    fn leases_resources(&self, _input: &serde_json::Value) -> bool {
        panic!("detonated");
    }

    async fn execute(
        &self,
        _input: serde_json::Value,
        _ctx: &aether_tools::ToolContext,
        _cancel: tokio_util::sync::CancellationToken,
    ) -> Result<aether_tools::ToolExecution, aether_tools::ToolError> {
        unreachable!("the turn never gets this far")
    }
}

/// The entry in `sched.running` *is* the concurrency slot, so an unwind that
/// leaves it behind holds an engine slot for the daemon's life: at a cap of
/// one, a single panicked turn wedges every engine agent in the fleet. The
/// panic must also stay visible rather than leaving the agent Running.
#[tokio::test]
async fn a_panicked_turn_leaves_its_engine_slot_behind_to_the_next_agent() {
    let h = harness_with_tools(
        vec![],
        |_| {},
        |c| {
            c.max_concurrent_turns = 4;
            c.max_concurrent_engine_turns = Some(1);
        },
        |tools| tools.register(Arc::new(Detonate)),
    )
    .await;
    h.client.push_for(
        "claude-code",
        ScriptedClient::tool_call("detonate", json!({})),
    );
    h.client.push_for(
        "claude-code",
        ScriptedClient::tool_call("complete_task", json!({"summary": "carried on"})),
    );
    let (boom, _) = h
        .runtime
        .create_agent(spec_on("blow up", "claude-code", "haiku"), Actor::user())
        .expect("boom");
    let status = h.wait_until(boom.id, |s| s.is_terminal()).await;
    assert!(
        matches!(&status, AgentStatus::Failed { error }
            if error.contains("panicked") && error.contains("detonated")),
        "the panic was not recorded as a turn failure: {status:?}"
    );
    assert_eq!(
        h.runtime.status().expect("status").engine_turns_running,
        0,
        "the panicked turn kept its engine slot"
    );
    // And the slot is genuinely usable, not merely reported free.
    let (after, _) = h
        .runtime
        .create_agent(spec_on("carry on", "claude-code", "haiku"), Actor::user())
        .expect("after");
    let status = h.wait_until(after.id, |s| s.is_terminal()).await;
    assert_eq!(
        status.kind(),
        AgentStatusKind::Completed,
        "the engine cap stayed wedged after the panic: {status:?}"
    );
}

/// Parking on a resource key gives the engine slot away deliberately, so the
/// cap must bind on the way back in too. Otherwise it is only an admission
/// bound: the replacement turn admitted while a turn was parked, plus the
/// parked turn resuming, is one more engine session than the operator asked
/// and paid for.
#[tokio::test]
async fn an_engine_turn_unparking_from_a_lease_queues_for_its_slot_again() {
    let h = harness_configured(
        vec![],
        |_| {},
        |c| {
            c.max_concurrent_turns = 4;
            c.max_concurrent_engine_turns = Some(1);
        },
    )
    .await;
    // The key holder is an API agent: it never touches the engine cap.
    h.client.push_for(
        "anthropic",
        ScriptedClient::tool_call(
            "bash",
            json!({"command": "for ((i=0;i<1500;i++)); do if test -f release; then exit 0; fi; sleep 0.01; done; exit 1"}),
        ),
    );
    h.client.push_for(
        "anthropic",
        ScriptedClient::tool_call("complete_task", json!({"summary": "holder done"})),
    );
    // Consumed in call order: the parking turn, then its replacement, then
    // the parking turn again once it is let back in.
    h.client.push_for(
        "claude-code",
        ScriptedClient::tool_call("bash", json!({"command": "echo parked > parked.txt"})),
    );
    h.client.push_for(
        "claude-code",
        ScriptedClient::tool_call("complete_task", json!({"summary": "replacement done"})),
    );
    h.client.push_for(
        "claude-code",
        ScriptedClient::tool_call("complete_task", json!({"summary": "parked done"})),
    );

    let (holder, _) = h
        .runtime
        .create_agent(spec("hold the repository"), Actor::user())
        .expect("holder");
    h.wait_for("the key was never taken", || {
        !h.runtime.resource_holdings().is_empty()
    })
    .await;

    let (parked, _) = h
        .runtime
        .create_agent(
            spec_on("touch it too", "claude-code", "haiku"),
            Actor::user(),
        )
        .expect("parked");
    h.wait_until(parked.id, |s| matches!(s, AgentStatus::Blocked { .. }))
        .await;

    // The slot the park freed goes to a fresh engine turn, held mid-stream.
    let release_replacement = h.client.hold_next_response();
    let (replacement, _) = h
        .runtime
        .create_agent(
            spec_on("take the freed slot", "claude-code", "haiku"),
            Actor::user(),
        )
        .expect("replacement");
    h.wait_for("the replacement never opened its stream", || {
        h.client.peak_in_flight("claude-code") >= 1
            && h.store
                .agent(replacement.id)
                .expect("read")
                .expect("exists")
                .status
                .kind()
                == AgentStatusKind::Running
    })
    .await;

    // The key comes free while the replacement still holds the only slot.
    std::fs::write(h.workspace_root.join("release"), "release").expect("release the key");
    h.wait_until(holder.id, |s| s.is_terminal()).await;
    release_replacement.cancel();
    for id in [replacement.id, parked.id] {
        h.wait_until(id, |s| s.is_terminal()).await;
    }

    assert!(
        first_event_seq(h.store.as_ref(), parked.id, "agent.resumed")
            > first_event_seq(h.store.as_ref(), replacement.id, "agent.turn_completed"),
        "the parked turn resumed beside the turn that took its engine slot"
    );
    assert_eq!(
        h.client.peak_in_flight("claude-code"),
        1,
        "more engine turns were open at once than the cap allows"
    );
}

/// A turn resuming from a park queues for a slot while still holding the
/// resource keys it just won, so every agent behind those keys waits out its
/// queue too. It therefore gets the next free slot ahead of a turn that has
/// been merely ready - otherwise the keys are held for a queue the holder
/// keeps losing.
#[tokio::test]
async fn a_resuming_turn_holding_keys_outranks_a_newcomer_for_the_next_slot() {
    let h = harness_configured(
        vec![],
        |_| {},
        |c| {
            c.max_concurrent_turns = 4;
            c.max_concurrent_engine_turns = Some(1);
        },
    )
    .await;
    // The key holder is an API agent: it never touches the engine cap.
    h.client.push_for(
        "anthropic",
        ScriptedClient::tool_call(
            "bash",
            json!({"command": "for ((i=0;i<1500;i++)); do if test -f release; then exit 0; fi; sleep 0.01; done; exit 1"}),
        ),
    );
    h.client.push_for(
        "anthropic",
        ScriptedClient::tool_call("complete_task", json!({"summary": "holder done"})),
    );
    // In call order: the parking turn, its replacement, then two finishing
    // turns shared by the resumer and the newcomer in whatever order they run.
    h.client.push_for(
        "claude-code",
        ScriptedClient::tool_call("bash", json!({"command": "echo parked > parked.txt"})),
    );
    h.client.push_for(
        "claude-code",
        ScriptedClient::tool_call("complete_task", json!({"summary": "replacement done"})),
    );
    for _ in 0..2 {
        h.client.push_for(
            "claude-code",
            ScriptedClient::tool_call("complete_task", json!({"summary": "done"})),
        );
    }

    let (holder, _) = h
        .runtime
        .create_agent(spec("hold the repository"), Actor::user())
        .expect("holder");
    h.wait_for("the key was never taken", || {
        !h.runtime.resource_holdings().is_empty()
    })
    .await;

    let (parked, _) = h
        .runtime
        .create_agent(
            spec_on("touch it too", "claude-code", "haiku"),
            Actor::user(),
        )
        .expect("parked");
    h.wait_until(parked.id, |s| matches!(s, AgentStatus::Blocked { .. }))
        .await;

    // The slot the park freed goes to a fresh engine turn, held mid-stream.
    let release_replacement = h.client.hold_next_response();
    let (replacement, _) = h
        .runtime
        .create_agent(
            spec_on("take the freed slot", "claude-code", "haiku"),
            Actor::user(),
        )
        .expect("replacement");
    h.wait_for("the replacement never opened its stream", || {
        h.client.peak_in_flight("claude-code") >= 1
    })
    .await;

    // The key comes free: the parked turn wins the lease and starts queueing
    // for a slot, holding the key throughout.
    std::fs::write(h.workspace_root.join("release"), "release").expect("release the key");
    h.wait_until(holder.id, |s| s.is_terminal()).await;
    h.wait_for("the parked turn never took the key", || {
        h.runtime
            .resource_holdings()
            .iter()
            .any(|held| held.agent == parked.id)
    })
    .await;

    // A newcomer arrives while the queue is held back by the cap.
    let (newcomer, _) = h
        .runtime
        .create_agent(
            spec_on("arrive later", "claude-code", "haiku"),
            Actor::user(),
        )
        .expect("newcomer");
    h.wait_until(newcomer.id, |s| {
        matches!(s, AgentStatus::Ready | AgentStatus::Running)
    })
    .await;

    release_replacement.cancel();
    for id in [replacement.id, parked.id, newcomer.id] {
        h.wait_until(id, |s| s.is_terminal()).await;
    }

    assert!(
        first_event_seq(h.store.as_ref(), parked.id, "agent.resumed")
            < first_event_seq(h.store.as_ref(), newcomer.id, "agent.turn_completed"),
        "the newcomer took the freed slot from a resumer sitting on the keys it wanted"
    );
    assert_eq!(
        h.client.peak_in_flight("claude-code"),
        1,
        "more engine turns were open at once than the cap allows"
    );
}
