//! fallback guards scenarios and fixtures.
use super::*;

/// Providers for the fallback-chain guards: an unpriced API model, an engine,
/// and the builtin priced ones the rest of the suite uses.
async fn chain_guard_harness() -> Harness {
    harness_full(
        vec![],
        |_| {},
        |_| {},
        Some(
            // `frugal` carries its key in a `headers` override and declares
            // no price at all - the reported case of an unpriced
            // gateway, which is metered whatever the guard used to assume.
            // `proxy` is the same shape but with an explicit price, which was
            // already refused before the unpriced one was.
            "[providers.frugal]\nprotocol = \"openai-chat-completions\"\n\
             base_url = \"http://127.0.0.1:1\"\n\
             [providers.frugal.headers]\n\
             Authorization = \"Bearer sk-unpriced\"\n\
             [[providers.frugal.models]]\nid = \"nocost\"\n\
             [providers.proxy]\nprotocol = \"openai-chat-completions\"\n\
             base_url = \"http://127.0.0.1:1\"\n\
             [providers.proxy.headers]\n\
             Authorization = \"Bearer sk-billed\"\n\
             [[providers.proxy.models]]\nid = \"big\"\n\
             [providers.proxy.models.capabilities]\n\
             context_window = 200000\nmax_output_tokens = 8192\n\
             [providers.proxy.models.capabilities.cost]\n\
             input_per_mtok = 3.0\noutput_per_mtok = 15.0\n\
             [providers.cc]\nprotocol = \"claude-code\"\n\
             base_url = \"http://127.0.0.1:1\"\n\
             [[providers.cc.models]]\nid = \"sonnet\"\n\
             [[providers.cc.models]]\nid = \"haiku\"\n",
        ),
        None,
        None,
        |_| {},
    )
    .await
}

fn live_agents(h: &Harness) -> usize {
    h.store
        .agents(&AgentFilter::default())
        .expect("agents")
        .len()
}

/// A fallback is a model the turn really runs on, so every link
/// of a chain faces the guards the primary faces. An MCP client composing
/// `provider/model,provider/model` must not reach past a cost budget or a
/// container backend by hiding the offending model behind the first link.
#[tokio::test]
async fn every_fallback_link_faces_the_guards_the_primary_faces() {
    use aether_core::agent::ExecutionBackend;

    let h = chain_guard_harness().await;
    let before = live_agents(&h);

    // A cost budget the second link could never enforce.
    let budgeted = CreateAgentSpec {
        fallbacks: vec![ModelRef::new("frugal", "nocost")],
        config: Some(AgentConfig {
            cost_budget: Some(1.0),
            approval: ApprovalPolicy::none(),
            ..Default::default()
        }),
        ..spec_on("spend a little", "anthropic", "claude-sonnet-5-5")
    };
    let err = h
        .runtime
        .create_agent(budgeted, Actor::user())
        .expect_err("the budget cannot be enforced on the fallback");
    assert!(
        err.to_string().contains("fallback model `frugal/nocost`")
            && err.to_string().contains("no dollar cost"),
        "the refusal names the link and the guard: {err}"
    );

    // A container backend the second link would not be confined by.
    let contained = Scope {
        workspace_root: Some(h.workspace_root.clone()),
        backend: ExecutionBackend::Container {
            image: "rust:1".into(),
        },
        ..Default::default()
    };
    let mut escapes = CreateAgentSpec {
        fallbacks: vec![ModelRef::new("cc", "sonnet")],
        ..spec_on("build it", "anthropic", "claude-sonnet-5-5")
    };
    escapes.scope = Some(contained.clone());
    let err = h
        .runtime
        .create_agent(escapes, Actor::user())
        .expect_err("an engine fallback would run its tools on the host");
    assert!(
        err.to_string().contains("fallback model `cc/sonnet`")
            && err.to_string().contains("runs its tools on the host"),
        "the refusal names the link and the guard: {err}"
    );
    assert_eq!(before, live_agents(&h), "no agent was created");

    // A chain whose every link passes both guards is created as before.
    let mut ok = CreateAgentSpec {
        fallbacks: vec![ModelRef::new("anthropic", "claude-haiku-5-5")],
        config: Some(AgentConfig {
            cost_budget: Some(1.0),
            approval: ApprovalPolicy::none(),
            ..Default::default()
        }),
        ..spec_on("build it", "anthropic", "claude-sonnet-5-5")
    };
    ok.scope = Some(contained);
    let (agent, _) = h
        .runtime
        .create_agent(ok, Actor::user())
        .expect("valid chain");
    assert_eq!(
        agent.fallbacks,
        vec![ModelRef::new("anthropic", "claude-haiku-5-5")]
    );

    // And a live agent cannot be patched onto a chain that fails a guard.
    let err = h
        .runtime
        .update_agent(
            agent.id,
            aether_runtime::AgentUpdate {
                fallbacks: Some(vec![ModelRef::new("frugal", "nocost")]),
                ..Default::default()
            },
            Actor::user(),
        )
        .expect_err("the same guards on update");
    assert!(
        err.to_string().contains("fallback model `frugal/nocost`"),
        "{err}"
    );
    assert_eq!(
        h.store.agent(agent.id).expect("a").expect("e").fallbacks,
        vec![ModelRef::new("anthropic", "claude-haiku-5-5")],
        "nothing was written"
    );
}

/// The other order of the same pair: a chain with an unpriced link is fine
/// until a cost budget arrives, and then the budget is refused rather than
/// stored as a stop that fires on the primary and never on the fallback.
#[tokio::test]
async fn a_cost_budget_is_refused_when_a_fallback_link_reports_no_cost() {
    let h = chain_guard_harness().await;
    let (agent, _) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                fallbacks: vec![ModelRef::new("frugal", "nocost")],
                ..spec_on("spend nothing", "anthropic", "claude-sonnet-5-5")
            },
            Actor::user(),
        )
        .expect("no budget, no guard to fail");
    let err = h
        .runtime
        .update_agent(
            agent.id,
            aether_runtime::AgentUpdate {
                cost_budget: Some(2.50),
                ..Default::default()
            },
            Actor::user(),
        )
        .expect_err("the budget could not be enforced on the second link");
    assert!(
        err.to_string().contains("fallback model `frugal/nocost`")
            && err.to_string().contains("no dollar cost"),
        "the refusal names the link and the guard: {err}"
    );
    assert_eq!(
        h.store
            .agent(agent.id)
            .expect("a")
            .expect("e")
            .config
            .cost_budget,
        None,
        "nothing was written"
    );

    // Dropping the unpriced link is what makes the budget enforceable.
    let budgeted = h
        .runtime
        .update_agent(
            agent.id,
            aether_runtime::AgentUpdate {
                fallbacks: Some(vec![ModelRef::new("anthropic", "claude-haiku-5-5")]),
                cost_budget: Some(2.50),
                ..Default::default()
            },
            Actor::user(),
        )
        .expect("every link prices its tokens");
    assert_eq!(budgeted.config.cost_budget, Some(2.50));
}

/// VISION §1.6: a subscription seat pays for the primary, so
/// a fallback onto a metered API is a bill nobody agreed to. Refused by name;
/// the reverse direction and a same-class chain are untouched.
#[tokio::test]
async fn a_subscription_chain_may_not_fall_onto_a_metered_api() {
    let h = chain_guard_harness().await;
    let before = live_agents(&h);

    let crossing = CreateAgentSpec {
        fallbacks: vec![ModelRef::new("anthropic", "claude-sonnet-5-5")],
        ..spec_on("write it", "cc", "sonnet")
    };
    let err = h
        .runtime
        .create_agent(crossing, Actor::user())
        .expect_err("a subscription must not roll onto a metered API");
    assert!(
        err.to_string()
            .contains("fallback model `anthropic/claude-sonnet-5-5`")
            && err.to_string().contains("subscription"),
        "the refusal names the link and why: {err}"
    );
    assert_eq!(before, live_agents(&h), "no agent was created");

    // Within one billing class the chain is fine.
    let (agent, _) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                fallbacks: vec![ModelRef::new("cc", "haiku")],
                ..spec_on("write it", "cc", "sonnet")
            },
            Actor::user(),
        )
        .expect("subscription to subscription");
    assert_eq!(agent.fallbacks, vec![ModelRef::new("cc", "haiku")]);

    // Patching the crossing onto a live agent is refused the same way.
    let err = h
        .runtime
        .update_agent(
            agent.id,
            aether_runtime::AgentUpdate {
                fallbacks: Some(vec![ModelRef::new("anthropic", "claude-sonnet-5-5")]),
                ..Default::default()
            },
            Actor::user(),
        )
        .expect_err("the same refusal on update");
    assert!(err.to_string().contains("subscription"), "{err}");

    // Metered primary onto a subscription fallback adds no bill: allowed.
    h.runtime
        .create_agent(
            CreateAgentSpec {
                fallbacks: vec![ModelRef::new("cc", "sonnet")],
                ..spec_on("write it", "anthropic", "claude-sonnet-5-5")
            },
            Actor::user(),
        )
        .expect("metered to subscription");

    // A gateway that wants no credentials still meters its tokens: the key can
    // ride in a header override, so "no auth" alone must not buy an exemption.
    let err = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                fallbacks: vec![ModelRef::new("proxy", "big")],
                ..spec_on("write it", "cc", "sonnet")
            },
            Actor::user(),
        )
        .expect_err("a priced gateway is a bill even without credentials");
    assert!(
        err.to_string().contains("fallback model `proxy/big`")
            && err.to_string().contains("separately billed"),
        "the refusal names the link and why: {err}"
    );

    // The reported case: a gateway that also carries its key in a
    // header override, but declares no price at all. An absent price is
    // unknown, not free, so it is refused exactly like the priced gateway.
    let err = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                fallbacks: vec![ModelRef::new("frugal", "nocost")],
                ..spec_on("write it", "cc", "sonnet")
            },
            Actor::user(),
        )
        .expect_err("an unpriced gateway is metered, not free");
    assert!(
        err.to_string().contains("fallback model `frugal/nocost`")
            && err.to_string().contains("separately billed"),
        "the refusal names the link and why: {err}"
    );

    // The local servers it is distinguished from stay allowed - the shipped
    // ones quote $0.00 rather than omitting the price, so a guard that asked
    // only whether a price exists would refuse every one.
    for link in [
        ModelRef::new("ollama", "llama3.3"),
        ModelRef::new("vllm", "qwen3"),
        ModelRef::new("lmstudio", "mistral"),
        ModelRef::new("llamacpp", "gemma3"),
    ] {
        h.runtime
            .create_agent(
                CreateAgentSpec {
                    fallbacks: vec![link.clone()],
                    ..spec_on("write it", "cc", "sonnet")
                },
                Actor::user(),
            )
            .unwrap_or_else(|e| panic!("a free local server is no bill: `{link}` refused: {e}"));
    }
}

/// The guard reads the half of the chain a patch did not touch, so
/// a stored primary the registry can no longer build - a provider dropped from
/// `providers.toml` under a live agent - fails like any other unbuildable
/// model. Waving it through would let a fallbacks-only patch settle the very
/// crossing creation refuses by name.
#[tokio::test]
async fn a_chain_patch_is_refused_while_the_stored_primary_cannot_be_built() {
    let h = chain_guard_harness().await;
    let (agent, _) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                fallbacks: vec![ModelRef::new("cc", "haiku")],
                ..spec_on("write it", "cc", "sonnet")
            },
            Actor::user(),
        )
        .expect("a same-class chain");

    // The primary's provider leaves the registry under the live row.
    let mut row = h.store.agent(agent.id).expect("read").expect("row");
    row.model = ModelRef::new("gone", "sonnet");
    h.store
        .commit(Commit::new().mutate(Mutation::UpsertAgent(row)))
        .expect("commit");

    let metered = vec![ModelRef::new("anthropic", "claude-sonnet-5-5")];
    let err = h
        .runtime
        .update_agent(
            agent.id,
            aether_runtime::AgentUpdate {
                fallbacks: Some(metered.clone()),
                ..Default::default()
            },
            Actor::user(),
        )
        .expect_err("an unbuildable primary cannot be cleared of a crossing");
    assert!(
        err.to_string().contains("this agent's model `gone/sonnet`")
            && err.to_string().contains("unknown provider"),
        "the refusal names the half the caller never touched: {err}"
    );
    assert!(
        err.to_string().contains("fix its provider or credentials"),
        "the refusal carries a remedy hint: {err}"
    );
    assert!(
        matches!(err, aether_runtime::RuntimeError::Models(_)),
        "an unbuildable link is a registry failure, not a bad argument: {err:?}"
    );
    assert_eq!(
        h.store
            .agent(agent.id)
            .expect("read")
            .expect("row")
            .fallbacks,
        vec![ModelRef::new("cc", "haiku")],
        "nothing was written",
    );

    // Both halves move in one patch, so nobody is stuck with a dead row.
    let fixed = h
        .runtime
        .update_agent(
            agent.id,
            aether_runtime::AgentUpdate {
                model: Some(ModelRef::new("cc", "sonnet")),
                fallbacks: Some(vec![ModelRef::new("cc", "haiku")]),
                ..Default::default()
            },
            Actor::user(),
        )
        .expect("the chain is fixable in one call");
    assert_eq!(fixed.model, ModelRef::new("cc", "sonnet"));
}

/// The fallback half of the same guard carries its own remedy and the
/// same `Models` class, so a caller sees why and what to do about it. Reached
/// through a stored fallback the registry can no longer build, the same way
/// [`a_chain_patch_is_refused_while_the_stored_primary_cannot_be_built`]
/// reaches the primary half - a caller always resolves a link it is changing
/// before this guard runs, so only a stored, untouched link can still fail here.
#[tokio::test]
async fn an_unbuildable_fallback_link_carries_a_remedy_and_the_models_class() {
    let h = chain_guard_harness().await;
    let (agent, _) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                fallbacks: vec![ModelRef::new("cc", "haiku")],
                ..spec_on("write it", "cc", "sonnet")
            },
            Actor::user(),
        )
        .expect("a same-class chain");

    // The fallback's provider leaves the registry under the live row.
    let mut row = h.store.agent(agent.id).expect("read").expect("row");
    row.fallbacks = vec![ModelRef::new("gone", "sonnet")];
    h.store
        .commit(Commit::new().mutate(Mutation::UpsertAgent(row)))
        .expect("commit");

    // Only `model` moves, so the stored (broken) fallback is what
    // `check_billing_class` itself resolves, not the pre-check in
    // `update_agent` that only re-validates a changed `fallbacks`.
    let err = h
        .runtime
        .update_agent(
            agent.id,
            aether_runtime::AgentUpdate {
                model: Some(ModelRef::new("cc", "sonnet")),
                ..Default::default()
            },
            Actor::user(),
        )
        .expect_err("the stored fallback provider no longer exists");
    assert!(
        err.to_string().contains("fallback model `gone/sonnet`")
            && err.to_string().contains("unknown provider")
            && err.to_string().contains("drop the fallback"),
        "the refusal names the link, the cause and a remedy: {err}"
    );
    assert!(
        matches!(err, aether_runtime::RuntimeError::Models(_)),
        "an unbuildable link is a registry failure, not a bad argument: {err:?}"
    );
}
