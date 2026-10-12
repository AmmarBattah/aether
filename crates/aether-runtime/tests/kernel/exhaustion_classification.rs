//! exhaustion classification scenarios and fixtures.
use super::*;

/// A spent link is never retried in place: with a fallback the turn moves on,
/// and without one only a context limit is terminal - spent credits suspend
/// instead (`final_link_exhaustion_suspends_instead_of_failing`).
#[tokio::test]
async fn exhaustion_falls_back_once_or_fails_without_retries() {
    for class in [
        ModelErrorClass::QuotaExhausted,
        ModelErrorClass::UsageLimit,
        ModelErrorClass::ContextLimit,
    ] {
        for recover in [false, true] {
            // Without a fallback only a context limit is terminal.
            if !recover && class != ModelErrorClass::ContextLimit {
                continue;
            }
            let mut responses = vec![ScriptedClient::error(
                class,
                "Original provider message; resets tomorrow 14:00",
            )];
            if recover {
                responses.push(ScriptedClient::tool_call(
                    "complete_task",
                    json!({"summary": "fallback recovered"}),
                ));
            }
            let h = harness(responses).await;
            let (agent, task) = h
                .runtime
                .create_agent(
                    CreateAgentSpec {
                        model: Some(ModelRef::new("anthropic", "claude-sonnet-5-5")),
                        fallbacks: if recover {
                            vec![ModelRef::new("groq", "openai/gpt-oss-120b")]
                        } else {
                            vec![]
                        },
                        ..spec("recover only with a healthy fallback")
                    },
                    Actor::user(),
                )
                .unwrap();
            let status = h.wait_until(agent.id, |s| s.is_terminal()).await;
            // A spent account is benched, bounded; a too-long request is not.
            let benched: Vec<String> = h
                .store
                .events(0, 10_000, &EventFilter::default())
                .unwrap()
                .into_iter()
                .filter_map(|event| match event.kind {
                    EventKind::ProviderBackoff {
                        provider, until, ..
                    } => {
                        assert!(until <= now() + chrono::Duration::seconds(301));
                        Some(provider)
                    }
                    _ => None,
                })
                .collect();
            if class == ModelErrorClass::ContextLimit {
                assert!(benched.is_empty(), "{benched:?}");
            } else {
                assert_eq!(benched, vec!["anthropic".to_owned()]);
            }
            if recover {
                assert!(
                    matches!(status, AgentStatus::Completed { .. }),
                    "{status:?}"
                );
                assert_eq!(h.client.requests().len(), 2);
            } else {
                let AgentStatus::Failed { error } = status else {
                    panic!("{status:?}")
                };
                assert!(error.contains(class.as_str()), "{error}");
                assert!(error.contains("resets tomorrow 14:00"));
                assert!(error.contains("claude-sonnet-5-5"));
                assert!(error.contains(&task.unwrap().id.to_string()));
                assert_eq!(h.client.requests().len(), 1);
            }
            h.runtime.shutdown().await;
        }
    }
}

#[tokio::test]
async fn exhausted_primary_cause_survives_a_different_terminal_fallback_error() {
    let h = harness(vec![
        Err(aether_core::model::ModelError::new(
            ModelErrorClass::QuotaExhausted,
            "credit balance too low",
        )
        .with_retry_after(Duration::from_secs(3600))),
        ScriptedClient::error(
            ModelErrorClass::InvalidRequest,
            "fallback rejected the request",
        ),
    ])
    .await;
    let (agent, _) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                model: Some(ModelRef::new("anthropic", "claude-sonnet-5-5")),
                fallbacks: vec![ModelRef::new("groq", "openai/gpt-oss-120b")],
                ..spec("preserve the exhausted link")
            },
            Actor::user(),
        )
        .unwrap();
    let AgentStatus::Failed { error } = h.wait_until(agent.id, |s| s.is_terminal()).await else {
        panic!("expected failure")
    };
    assert!(error.contains("quota_exhausted"), "{error}");
    assert!(error.contains("credit balance too low"));
    assert!(error.contains("3600 seconds"));
    assert!(error.contains("fallback rejected the request"));
    assert!(error.contains("claude-sonnet-5-5"));
    assert_eq!(h.client.requests().len(), 2);
    h.runtime.shutdown().await;
}

/// A root > parent > child tree whose child runs the harness script and fails;
/// returns the three agents and the child's terminal error.
async fn failed_child_of_three_levels(h: &Harness) -> (AgentId, AgentId, AgentId, String) {
    let (root, _) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                start: false,
                ..spec("root")
            },
            Actor::user(),
        )
        .unwrap();
    let (parent, _) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                parent: Some(root.id),
                start: false,
                ..spec("parent")
            },
            Actor::Agent { id: root.id },
        )
        .unwrap();
    let (child, _) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                parent: Some(parent.id),
                ..spec("child")
            },
            Actor::Agent { id: parent.id },
        )
        .unwrap();
    let AgentStatus::Failed { error } = h.wait_until(child.id, |status| status.is_terminal()).await
    else {
        panic!("expected the child to fail")
    };
    (root.id, parent.id, child.id, error)
}

fn exhaustion_events(store: &dyn Store, id: AgentId) -> usize {
    store
        .events(0, 1000, &EventFilter::own_events(id))
        .unwrap()
        .iter()
        .filter(|event| {
            matches!(&event.kind, EventKind::Custom { name, .. } if name == "exhaustion.failed")
        })
        .count()
}

#[tokio::test]
async fn authoritative_model_classes_are_not_reclassified_from_quota_wording() {
    for class in [
        ModelErrorClass::Auth,
        ModelErrorClass::Permission,
        ModelErrorClass::Unknown,
    ] {
        let h = harness(vec![ScriptedClient::error(
            class,
            "HTTP 401: insufficient_quota",
        )])
        .await;
        let (root, _, child, error) = failed_child_of_three_levels(&h).await;
        assert!(error.contains(&format!("{class:?}")));
        assert!(!error.starts_with("exhaustion:"), "{error}");
        assert!(child_failures(h.store.as_ref(), root).is_empty());
        assert_eq!(exhaustion_events(h.store.as_ref(), child), 0);
        let recorded: Vec<_> = h
            .store
            .events(0, 1000, &EventFilter::own_events(child))
            .unwrap()
            .into_iter()
            .filter_map(|event| match event.kind {
                EventKind::InvocationFailed { class, .. } => class,
                _ => None,
            })
            .collect();
        assert!(recorded.contains(&class), "{recorded:?}");
        h.runtime.shutdown().await;
    }
}

/// An agent's own `fail_task` text is its account, not a provider signal:
/// quota or context wording in it never becomes an exhaustion diagnostic.
#[tokio::test]
async fn an_agents_fail_task_prose_is_never_classified_as_exhaustion() {
    for prose in [
        "Blocked: the vendor billing API replied usage limit exceeded for our account",
        "Cannot continue: the embeddings service says insufficient_quota",
        "The generated prompt is too long for the downstream summarizer",
        "token budget of 100: I cannot finish the migration within it",
        "cost budget of $5 is too small for this job",
    ] {
        let h = harness(vec![ScriptedClient::tool_call(
            "fail_task",
            json!({"error": prose}),
        )])
        .await;
        let (root, parent, child, error) = failed_child_of_three_levels(&h).await;
        assert!(error.contains(prose), "{error}");
        assert!(!error.starts_with("exhaustion:"), "{error}");
        assert_eq!(exhaustion_events(h.store.as_ref(), child), 0);
        assert!(child_failures(h.store.as_ref(), root).is_empty());
        let delivered = child_failures(h.store.as_ref(), parent);
        assert_eq!(delivered.len(), 1, "{delivered:?}");
        assert!(delivered[0].contains(prose), "{delivered:?}");
        assert!(!delivered[0].starts_with("exhaustion:"), "{delivered:?}");
        h.runtime.shutdown().await;
    }
}
