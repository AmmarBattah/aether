//! approval rules scenarios and fixtures.
use super::*;

#[tokio::test]
async fn a_remembered_allow_answers_the_next_call_without_a_person() {
    let h = harness(vec![
        ScriptedClient::tool_call(
            "write_file",
            json!({"path": "note.txt", "content": "hello"}),
        ),
        ScriptedClient::tool_call("complete_task", json!({"summary": "written"})),
    ])
    .await;
    let mut s = spec("write a note");
    s.config = Some(AgentConfig {
        approval: ApprovalPolicy {
            require_for: [SideEffectClass::IdempotentMutation].into(),
        },
        ..Default::default()
    });
    let (agent, _) = h.runtime.create_agent(s, Actor::user()).expect("create");
    h.runtime
        .add_approval_rule(
            ApprovalDecision::Approved,
            RuleMatch::tool("write_file"),
            RuleScope::Agent { id: agent.id },
            Actor::user(),
            Some("notes are fine".into()),
        )
        .expect("rule");

    let status = h.wait_until(agent.id, |s| s.is_terminal()).await;
    assert_eq!(
        status.kind(),
        AgentStatusKind::Completed,
        "{:?}",
        types(h.store.as_ref(), agent.id)
    );
    assert_eq!(
        std::fs::read_to_string(h.workspace_root.join("note.txt")).expect("file"),
        "hello"
    );
    // Nothing waited on a person, and the log still records who decided.
    assert!(
        h.store.pending_approvals().expect("pending").is_empty(),
        "a remembered decision must not leave an approval waiting"
    );
    let resolved = h
        .store
        .events(0, 10_000, &EventFilter::for_agent(agent.id))
        .expect("events")
        .into_iter()
        .find_map(|e| match e.kind {
            EventKind::ApprovalResolved { decision, by, .. } => Some((decision, by)),
            _ => None,
        })
        .expect("an approval was resolved");
    assert_eq!(resolved.0, ApprovalDecision::Approved);
    assert!(
        matches!(resolved.1, Actor::Rule { .. }),
        "the rule must be recorded as the decider, got {:?}",
        resolved.1
    );
}

#[tokio::test]
async fn a_remembered_deny_refuses_the_call_and_a_revoked_rule_stops_deciding() {
    let h = harness(vec![
        ScriptedClient::tool_call(
            "write_file",
            json!({"path": "note.txt", "content": "hello"}),
        ),
        ScriptedClient::tool_call("complete_task", json!({"summary": "gave up"})),
    ])
    .await;
    let mut s = spec("write a note");
    s.config = Some(AgentConfig {
        approval: ApprovalPolicy {
            require_for: [SideEffectClass::IdempotentMutation].into(),
        },
        ..Default::default()
    });
    let (agent, _) = h.runtime.create_agent(s, Actor::user()).expect("create");
    let rule = h
        .runtime
        .add_approval_rule(
            ApprovalDecision::Denied,
            RuleMatch::tool("write_file"),
            RuleScope::Everywhere,
            Actor::user(),
            None,
        )
        .expect("rule");

    h.wait_until(agent.id, |s| s.is_terminal()).await;
    assert!(
        !h.workspace_root.join("note.txt").exists(),
        "a remembered refusal must not write the file"
    );
    assert!(types(h.store.as_ref(), agent.id).contains(&"tool.denied".to_string()));

    let revoked = h
        .runtime
        .revoke_approval_rule(rule.id, Actor::user(), Some("changed my mind".into()))
        .expect("revoke");
    assert!(revoked.revoked.is_some());
    assert!(
        h.runtime
            .approval_rules(false)
            .expect("rules")
            .iter()
            .all(|r| r.id != rule.id),
        "a revoked rule must not be listed as active"
    );
    assert!(
        h.runtime
            .approval_rules(true)
            .expect("rules")
            .iter()
            .any(|r| r.id == rule.id),
        "a revoked rule stays on the record"
    );
    // Nothing decides the same call now, so a person would be asked again.
    let subject = aether_core::approval::CallSubject::new("write_file", None, Vec::new());
    let ctx = h.runtime.rule_context(agent.id).expect("ctx");
    assert_eq!(
        h.runtime.gate_call(true, &subject, &ctx).expect("gate"),
        aether_runtime::Gate::AskPerson
    );
}

#[tokio::test]
async fn a_remembered_answer_is_the_narrowest_reading_of_it() {
    // One `a` on the card must not become "allow this tool everywhere".
    let h = harness(vec![
        ScriptedClient::tool_call(
            "write_file",
            json!({"path": "note.txt", "content": "hello"}),
        ),
        ScriptedClient::tool_call("complete_task", json!({"summary": "written"})),
    ])
    .await;
    let mut s = spec("write a note");
    s.config = Some(AgentConfig {
        approval: ApprovalPolicy {
            require_for: [SideEffectClass::IdempotentMutation].into(),
        },
        ..Default::default()
    });
    let (agent, _) = h.runtime.create_agent(s, Actor::user()).expect("create");
    h.wait_until(agent.id, |s| s.kind() == AgentStatusKind::Waiting)
        .await;
    let pending = h.store.pending_approvals().expect("pending");
    assert_eq!(pending.len(), 1);

    h.runtime
        .resolve_approval(
            pending[0].id,
            ApprovalDecision::Approved,
            Actor::user(),
            None,
            Some(Remember::narrow(Some("remembered from the card".into()))),
        )
        .expect("approve");
    let rules = h.runtime.approval_rules(false).expect("rules");
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].matcher.tool.as_deref(), Some("write_file"));
    assert_eq!(
        rules[0].scope,
        RuleScope::Agent { id: agent.id },
        "the answered agent, not its subtree and not everywhere"
    );
    // write_file exposes no argument, so the narrowing is what it touched.
    let prefix = rules[0]
        .matcher
        .resource_prefix
        .clone()
        .expect("a path-only tool is narrowed by resource");
    assert!(
        prefix.starts_with("file:") && prefix.ends_with("note.txt"),
        "the rule covers the file that was answered, not the tool: {prefix}"
    );
    h.wait_until(agent.id, |s| s.is_terminal()).await;
}

#[tokio::test]
async fn the_rule_and_the_answer_land_together() {
    // A rule that cannot be created must not leave the approval answered,
    // and the answer must not land without the rule it was given with.
    let h = harness(vec![
        ScriptedClient::tool_call(
            "write_file",
            json!({"path": "note.txt", "content": "hello"}),
        ),
        ScriptedClient::tool_call("complete_task", json!({"summary": "written"})),
    ])
    .await;
    let mut s = spec("write a note");
    s.config = Some(AgentConfig {
        approval: ApprovalPolicy {
            require_for: [SideEffectClass::IdempotentMutation].into(),
        },
        ..Default::default()
    });
    let (agent, _) = h.runtime.create_agent(s, Actor::user()).expect("create");
    h.wait_until(agent.id, |s| s.kind() == AgentStatusKind::Waiting)
        .await;
    let pending = h.store.pending_approvals().expect("pending");
    let refused = h.runtime.resolve_approval(
        pending[0].id,
        ApprovalDecision::Approved,
        Actor::user(),
        None,
        Some(Remember {
            // No such agent, so the rule is refused at its boundary.
            scope: Some(RuleScope::Agent { id: AgentId::new() }),
            tool_wide: true,
            note: None,
        }),
    );
    assert!(refused.is_err(), "a rule in a scope that does not exist");
    assert!(
        h.runtime.approval_rules(true).expect("rules").is_empty(),
        "no rule was written"
    );
    assert_eq!(
        h.store.pending_approvals().expect("pending").len(),
        1,
        "and the approval is still unanswered"
    );

    // The same answer with a real scope lands both.
    h.runtime
        .resolve_approval(
            pending[0].id,
            ApprovalDecision::Approved,
            Actor::user(),
            None,
            Some(Remember {
                scope: Some(RuleScope::Subtree { root: agent.id }),
                tool_wide: true,
                note: None,
            }),
        )
        .expect("approve");
    let rules = h.runtime.approval_rules(false).expect("rules");
    assert_eq!(rules.len(), 1);
    assert!(
        rules[0].matcher.resource_prefix.is_none() && rules[0].matcher.argument.is_none(),
        "tool_wide was asked for explicitly, so the rule covers the tool"
    );
    assert!(h.store.pending_approvals().expect("pending").is_empty());
    h.wait_until(agent.id, |s| s.is_terminal()).await;
}
