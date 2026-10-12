//! agent support scenarios and fixtures.
use super::*;

pub(super) fn waits_on_inbox(condition: &aether_core::agent::WaitCondition) -> bool {
    use aether_core::agent::WaitCondition as C;
    match condition {
        C::Inbox => true,
        C::Any { of } | C::All { of } | C::AtLeast { of, .. } => of.iter().any(waits_on_inbox),
        _ => false,
    }
}

/// A tool an `ask` policy holds for a person, so a batch containing it queues
/// an approval before any of its calls run.
pub(super) struct CallOutside;

#[async_trait::async_trait]
impl aether_tools::Tool for CallOutside {
    fn descriptor(&self) -> aether_core::tool::ToolDescriptor {
        aether_core::tool::ToolDescriptor {
            spec: aether_core::model::ToolSpec {
                name: "call_outside".into(),
                description: "reaches an outside service".into(),
                input_schema: json!({"type": "object", "properties": {}}),
                strict: false,
            },
            side_effect: SideEffectClass::External,
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

    async fn execute(
        &self,
        _input: serde_json::Value,
        _ctx: &aether_tools::ToolContext,
        _cancel: tokio_util::sync::CancellationToken,
    ) -> Result<aether_tools::ToolExecution, aether_tools::ToolError> {
        Ok(aether_tools::ToolExecution::Outcome(
            aether_core::tool::ToolOutcome::ok("called the outside service"),
        ))
    }
}

/// The outcome recorded for the (single) call of `name` in `events`.
pub(super) fn settled_outcome(events: &[aether_core::event::Event], name: &str) -> ToolOutcome {
    let wanted = events
        .iter()
        .find_map(|e| match &e.kind {
            EventKind::ToolRequested { call, .. } if call.name == name => Some(call.id),
            _ => None,
        })
        .unwrap_or_else(|| panic!("{name} was never requested"));
    events
        .iter()
        .find_map(|e| match &e.kind {
            EventKind::ToolSucceeded { call, outcome } if *call == wanted => Some(outcome.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("{name} never settled"))
}

/// An idle, long-lived agent: it parks on its inbox instead of running a task,
/// so a test decides exactly when a turn becomes possible.
pub(super) fn idle_spec() -> CreateAgentSpec {
    let mut s = spec("idle");
    s.objective = None;
    s.config.as_mut().expect("config").long_lived = true;
    s
}

pub(super) fn paused(store: &dyn Store, id: AgentId) -> bool {
    store.agent(id).expect("agent").expect("exists").paused
}

/// A started agent that waits between messages, so a test decides when each
/// turn runs and which scripted response it takes.
pub(super) fn long_lived(role: &str, parent: Option<AgentId>) -> CreateAgentSpec {
    CreateAgentSpec {
        role: role.into(),
        parent,
        start: true,
        config: Some(AgentConfig {
            effort: Some(aether_core::model::Effort::Low),
            long_lived: true,
            approval: ApprovalPolicy::none(),
            ..Default::default()
        }),
        ..Default::default()
    }
}

/// The one child `parent` delegated, once it exists.
pub(super) async fn child_of(h: &Harness, parent: AgentId) -> AgentId {
    let children = || {
        h.store
            .agents(&AgentFilter {
                parent: Some(parent),
                ..Default::default()
            })
            .expect("agents")
    };
    h.wait_for("the delegated child", || !children().is_empty())
        .await;
    children().pop().expect("a delegated child").id
}
