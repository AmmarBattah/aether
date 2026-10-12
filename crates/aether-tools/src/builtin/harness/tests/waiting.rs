use super::*;

#[tokio::test]
async fn wait_threshold_requires_tasks_and_a_positive_reachable_count() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = ctx_in(dir.path());
    let task = TaskId::new();
    for (input, received) in [
        (json!({"tasks": [task], "at_least": 0}), "0"),
        (json!({"at_least": 1}), "1"),
        (
            json!({"tasks": [], "at_least": 1, "message_from": AgentId::new()}),
            "1",
        ),
        (json!({"tasks": [task, task], "at_least": 2}), "2"),
    ] {
        let error = act(&WaitFor, &ctx, input).await.unwrap_err().to_string();
        assert!(error.contains("at_least"), "{error}");
        assert!(error.contains(received), "{error}");
        assert!(error.contains("distinct task"), "{error}");
    }
}

#[tokio::test]
async fn wait_threshold_preserves_valid_subset_and_inbox_waits() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = ctx_in(dir.path());
    let one = TaskId::new();
    let two = TaskId::new();
    let action = act(&WaitFor, &ctx, json!({"tasks": [one, two], "at_least": 1}))
        .await
        .unwrap();
    assert!(matches!(action, HarnessAction::WaitFor {
        condition: WaitCondition::AtLeast { n: 1, ref of }
    } if of.len() == 2));
    assert_eq!(
        act(&WaitFor, &ctx, json!({})).await.unwrap(),
        HarnessAction::WaitFor {
            condition: WaitCondition::Inbox
        }
    );
}

#[tokio::test]
async fn scheduled_wake_preserves_absolute_time_and_rejects_bad_deadlines() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = ctx_in(dir.path());
    let at = aether_core::time::now() + chrono::Duration::hours(2);
    let action = act(&WaitFor, &ctx, json!({"wake_at":at.to_rfc3339()}))
        .await
        .unwrap();
    assert_eq!(
        action,
        HarnessAction::WaitFor {
            condition: WaitCondition::Any {
                of: vec![WaitCondition::Inbox, WaitCondition::Deadline { at }],
            }
        }
    );
    for input in [
        json!({"wake_at":"yesterday"}),
        json!({"wake_at":"2000-01-01T00:00:00Z"}),
        json!({"wake_at":at.to_rfc3339(),"timeout_seconds":1}),
        json!({"timeout_seconds":u64::MAX}),
        json!({"timeout_seconds":0}),
    ] {
        assert!(act(&WaitFor, &ctx, input).await.is_err());
    }
}

#[tokio::test]
async fn wait_for_builds_conditions() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = ctx_in(dir.path());
    let t1 = TaskId::new();
    let t2 = TaskId::new();
    let a = act(
        &WaitFor,
        &ctx,
        json!({"tasks": [t1.to_string(), t2.to_string()]}),
    )
    .await
    .unwrap();
    assert_eq!(
        a,
        HarnessAction::WaitFor {
            condition: WaitCondition::all_tasks(BTreeSet::from([t1, t2]))
        }
    );
    let a = act(
        &WaitFor,
        &ctx,
        json!({"tasks": [t1.to_string()], "any": true, "timeout_seconds": 5}),
    )
    .await
    .unwrap();
    match a {
        HarnessAction::WaitFor {
            condition: WaitCondition::Any { of },
        } => {
            assert_eq!(of.len(), 2);
            assert!(matches!(of[1], WaitCondition::Deadline { .. }));
        }
        other => panic!("{other:?}"),
    }
    let a = act(&WaitFor, &ctx, json!({})).await.unwrap();
    assert_eq!(
        a,
        HarnessAction::WaitFor {
            condition: WaitCondition::Inbox
        }
    );
    // A named sender is a wait of its own, and joins tasks as an `any`.
    let sender = aether_core::id::AgentId::new();
    let a = act(&WaitFor, &ctx, json!({"message_from": sender.to_string()}))
        .await
        .unwrap();
    assert_eq!(
        a,
        HarnessAction::WaitFor {
            condition: WaitCondition::MessageFrom { agent: sender }
        }
    );
    let a = act(
        &WaitFor,
        &ctx,
        json!({"tasks": [t1.to_string()], "message_from": sender.to_string()}),
    )
    .await
    .unwrap();
    match a {
        HarnessAction::WaitFor {
            condition: WaitCondition::Any { of },
        } => assert_eq!(
            of,
            vec![
                WaitCondition::all_tasks([t1]),
                WaitCondition::MessageFrom { agent: sender }
            ]
        ),
        other => panic!("{other:?}"),
    }
    assert!(matches!(
        act(&WaitFor, &ctx, json!({"message_from": "nope"})).await,
        Err(ToolError::InvalidInput(_))
    ));
    let a = act(
        &WaitFor,
        &ctx,
        json!({"tasks": [t1.to_string(), t2.to_string()], "at_least": 1}),
    )
    .await
    .unwrap();
    assert!(matches!(
        a,
        HarnessAction::WaitFor {
            condition: WaitCondition::AtLeast { n: 1, .. }
        }
    ));
    assert!(matches!(
        act(&WaitFor, &ctx, json!({"tasks": ["bogus"]})).await,
        Err(ToolError::InvalidInput(_))
    ));
}
