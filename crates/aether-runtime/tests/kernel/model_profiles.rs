//! model profiles scenarios and fixtures.
use super::*;

/// The snapshot of the agent's first invocation.
fn first_snapshot(h: &Harness, agent: AgentId) -> aether_core::snapshot::InvocationSnapshot {
    let invocations = h.store.invocations_for_agent(agent).expect("invocations");
    let id = invocations.first().expect("one invocation").snapshot_id;
    h.store
        .snapshot(id)
        .expect("snapshot")
        .expect("the snapshot the invocation names")
}

/// §4.4/§6.1: an authored model profile reaches the model, and is recorded in
/// the snapshot's own layer, so the inspector can say which profile a past
/// turn was given. Claude ships off, so this also covers the operator's
/// setting beating the recorded default.
#[tokio::test]
async fn a_model_profile_is_sent_and_recorded_for_its_family() {
    let h = harness_full(
        vec![ScriptedClient::tool_call(
            "complete_task",
            json!({"summary": "done"}),
        )],
        |_| {},
        |_| {},
        None,
        Some("[model]\nprofiles = { claude = true }\n"),
        None,
        |_| {},
    )
    .await;
    let (agent, _) = h
        .runtime
        .create_agent(spec("profile the family"), Actor::user())
        .expect("create");
    h.wait_until(agent.id, |s| s.is_terminal()).await;

    let snapshot = first_snapshot(&h, agent.id);
    let layer = snapshot
        .layers
        .iter()
        .find(|l| l.kind == LayerKind::ModelProfile)
        .expect("the model profile layer");
    let text = layer.text.as_deref().expect("profile text");
    assert!(
        text.starts_with("Model profile: Claude."),
        "the default model is anthropic/claude-sonnet-5-5: {text}"
    );
    assert!(layer.items == 1 && layer.tokens > 0, "{layer:?}");

    // Recorded is not the same as sent: check the request itself carried it.
    let sent = h.client.requests();
    let system: String = sent
        .first()
        .expect("one request")
        .system
        .iter()
        .map(|b| b.text.as_str())
        .collect();
    assert!(system.contains("Model profile: Claude."), "{system}");
}

/// §4.4's "helps or is neutral" gate, one test per family default. A family
/// whose committed eval did not show a gain must not be sent unasked: today
/// only Codex earned its default (ARCHITECTURE §8.1).
///
/// Each stands a provider of the family under test and reads the system the
/// model was actually given, not just the recorded layer.
async fn system_for_family(family: &str, settings: Option<&str>) -> String {
    let providers = format!(
        "[providers.fam]\nprotocol = \"openai-chat-completions\"\n\
         base_url = \"http://127.0.0.1:1\"\nfamily = \"{family}\"\n\
         [[providers.fam.models]]\nid = \"m\"\n"
    );
    let h = harness_full(
        vec![ScriptedClient::tool_call(
            "complete_task",
            json!({"summary": "done"}),
        )],
        |_| {},
        |c| c.default_model = Some(ModelRef::new("fam", "m")),
        Some(&providers),
        settings,
        None,
        |_| {},
    )
    .await;
    let (agent, _) = h
        .runtime
        .create_agent(spec("profile default"), Actor::user())
        .expect("create");
    h.wait_until(agent.id, |s| s.is_terminal()).await;
    h.client.requests()[0]
        .system
        .iter()
        .map(|b| b.text.as_str())
        .collect()
}

#[tokio::test]
async fn the_codex_profile_is_not_sent_by_default() {
    let system = system_for_family("codex", None).await;
    assert!(
        !system.contains("Model profile"),
        "model guidance must be opt-in: {system}"
    );
}

#[tokio::test]
async fn the_claude_profile_is_not_sent_by_default() {
    let system = system_for_family("claude", None).await;
    assert!(
        !system.contains("Model profile"),
        "the recorded Claude eval hurt on rename; it must not ship unasked: {system}"
    );
}

#[tokio::test]
async fn the_gpt_profile_is_not_sent_by_default() {
    let system = system_for_family("gpt", None).await;
    assert!(
        !system.contains("Model profile"),
        "GPT was never measured on a real model: {system}"
    );
}

/// Naming one family says nothing about the others.
#[tokio::test]
async fn switching_one_family_off_leaves_the_rest_at_their_defaults() {
    let system = system_for_family("codex", Some("[model]\nprofiles = { claude = true }\n")).await;
    assert!(
        !system.contains("Model profile: Codex."),
        "codex was not named, so it keeps its default: {system}"
    );
    let system = system_for_family("codex", Some("[model]\nprofiles = { codex = false }\n")).await;
    assert!(
        !system.contains("Model profile"),
        "the operator's word beats the eval: {system}"
    );
}

/// The profile is measurable only if it can be switched off; the eval in
/// `evals/model_profiles.py` runs both conditions on the same tasks.
#[tokio::test]
async fn profiles_can_be_switched_off() {
    let h = harness_full(
        vec![ScriptedClient::tool_call(
            "complete_task",
            json!({"summary": "done"}),
        )],
        |_| {},
        |_| {},
        None,
        Some(
            "[model]
profiles = false
",
        ),
        None,
        |_| {},
    )
    .await;
    let (agent, _) = h
        .runtime
        .create_agent(spec("no profile"), Actor::user())
        .expect("create");
    h.wait_until(agent.id, |s| s.is_terminal()).await;

    let layer = first_snapshot(&h, agent.id)
        .layers
        .iter()
        .find(|l| l.kind == LayerKind::ModelProfile)
        .cloned()
        .expect("the layer still exists, empty");
    assert!(layer.text.is_none(), "{layer:?}");
    let system: String = h.client.requests()[0]
        .system
        .iter()
        .map(|b| b.text.as_str())
        .collect();
    assert!(!system.contains("Model profile"), "{system}");
}
