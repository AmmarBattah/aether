use super::*;

#[tokio::test]
async fn status_since_turn_merge_preserves_status_pair_and_operator_changes() {
    let (mut h, _dir) = ComposeHarness::new().await;
    let at = now() - chrono::Duration::minutes(4);
    h.ctx.agent.transition(AgentStatus::Running, at).unwrap();
    h.runtime
        .apply(Commit::new().mutate(Mutation::UpsertAgent(h.ctx.agent.clone())))
        .unwrap();
    h.runtime
        .update_agent(
            h.ctx.agent.id,
            crate::AgentUpdate {
                name: Some("operator name".into()),
                token_budget: Some(900_000),
                ..Default::default()
            },
            Actor::user(),
        )
        .unwrap();
    h.runtime
        .pause_agent(h.ctx.agent.id, false, Actor::user())
        .unwrap();
    let later = at + chrono::Duration::minutes(1);
    h.ctx
        .agent
        .transition(
            AgentStatus::Waiting {
                condition: WaitCondition::Inbox,
            },
            later,
        )
        .unwrap();
    h.ctx.agent.transition(AgentStatus::Ready, later).unwrap();
    h.ctx.agent.transition(AgentStatus::Running, later).unwrap();
    h.ctx.agent.engine_session = Some("new-session".into());
    let hold = h.runtime.hold();
    let mut row = agent_row_with_session(&h.runtime, &h.ctx, &hold).unwrap();
    assert_eq!(row.status, AgentStatus::Running);
    assert_eq!(
        row.status_since,
        Some(later),
        "ABA copies time even with the same final kind"
    );
    assert_eq!(row.name, "operator name");
    assert_eq!(row.config.token_budget, Some(900_000));
    assert!(row.paused);
    assert_eq!(row.engine_session.as_deref(), Some("new-session"));
    row.transition(
        AgentStatus::Cancelled {
            reason: Some("operator stopped".into()),
        },
        later + chrono::Duration::minutes(1),
    )
    .unwrap();
    h.runtime
        .apply_held(
            hold,
            Commit::new().mutate(Mutation::UpsertAgent(row.clone())),
        )
        .unwrap();
    let hold = h.runtime.hold();
    let merged = agent_row(&h.runtime, &h.ctx, &hold).unwrap();
    assert_eq!(merged.status, row.status);
    assert_eq!(merged.status_since, row.status_since);
    drop(hold);
    finish(&h.runtime, &h.ctx, Next::Ready).unwrap();
    assert_eq!(
        h.runtime.store.agent(row.id).unwrap().unwrap().status_since,
        row.status_since
    );
}

#[tokio::test]
async fn a_same_engine_model_change_keeps_a_session_opened_mid_turn() {
    let (mut h, _dir) = ComposeHarness::new().await;
    let first = ModelRef::new("codex", "gpt-6.1-sol");
    let next = ModelRef::new("codex", "gpt-6-luna");
    h.ctx.agent.model = first.clone();
    h.ctx.agent.engine_session = Some("opened-in-flight".into());
    let mut row = h.runtime.store.agent(h.ctx.agent.id).unwrap().unwrap();
    row.model = first;
    row.engine_session = None;
    h.runtime
        .apply(Commit::new().mutate(Mutation::UpsertAgent(row)))
        .unwrap();

    h.runtime
        .update_agent(
            h.ctx.agent.id,
            crate::AgentUpdate {
                model: Some(next),
                ..Default::default()
            },
            Actor::user(),
        )
        .unwrap();
    let hold = h.runtime.hold();
    let merged = agent_row_with_session(&h.runtime, &h.ctx, &hold).unwrap();
    assert_eq!(
        merged.engine_session.as_deref(),
        Some("opened-in-flight"),
        "a switch within Codex discarded the session opened by the active turn"
    );
    drop(hold);

    h.runtime
        .update_agent(
            h.ctx.agent.id,
            crate::AgentUpdate {
                model: Some(ModelRef::new("codex", "default")),
                ..Default::default()
            },
            Actor::user(),
        )
        .unwrap();
    let hold = h.runtime.hold();
    let reset = agent_row_with_session(&h.runtime, &h.ctx, &hold).unwrap();
    assert!(
        reset.engine_session.is_none(),
        "selecting the native default must start a fresh session"
    );
}

#[tokio::test]
async fn status_since_resource_callbacks_and_finish_keep_authoritative_pair() {
    let (mut h, _dir) = ComposeHarness::new().await;
    let at = now() - chrono::Duration::minutes(4);
    h.ctx.agent.transition(AgentStatus::Running, at).unwrap();
    h.runtime
        .apply(Commit::new().mutate(Mutation::UpsertAgent(h.ctx.agent.clone())))
        .unwrap();
    let key = ResourceKey("status-time-fixture".into());
    let conflict = crate::resources::ResourceConflict {
        holder: AgentId::new(),
        key: key.clone(),
    };
    block_on_resource(&h.runtime, &mut h.ctx, &[key], &conflict).unwrap();
    let blocked = h.runtime.store.agent(h.ctx.agent.id).unwrap().unwrap();
    assert_eq!(blocked.status, h.ctx.agent.status);
    assert_eq!(blocked.status_since, h.ctx.agent.status_since);
    assert!(blocked.status_since.unwrap() > at);
    leave_resource_block(&h.runtime, &mut h.ctx, None, "released".into()).unwrap();
    let running = h.runtime.store.agent(h.ctx.agent.id).unwrap().unwrap();
    assert_eq!(running.status, h.ctx.agent.status);
    assert_eq!(running.status_since, h.ctx.agent.status_since);
    assert_eq!(running.status.kind(), AgentStatusKind::Running);
    finish(&h.runtime, &h.ctx, Next::Waiting(WaitCondition::Inbox)).unwrap();
    let waiting = h.runtime.store.agent(h.ctx.agent.id).unwrap().unwrap();
    assert_eq!(waiting.status.kind(), AgentStatusKind::Waiting);
    assert!(waiting.status_since.unwrap() >= running.status_since.unwrap());
}

#[tokio::test]
async fn status_since_resource_commit_failure_leaves_turn_and_store_unchanged() {
    for leaving in [false, true] {
        let (mut h, _dir) = ComposeHarness::new().await;
        h.ctx.agent.transition(AgentStatus::Running, now()).unwrap();
        if leaving {
            h.ctx
                .agent
                .transition(resource_test_status(), now())
                .unwrap();
        }
        h.runtime
            .apply(Commit::new().mutate(Mutation::UpsertAgent(h.ctx.agent.clone())))
            .unwrap();
        let before = h.ctx.agent.clone();
        let seq = h.runtime.store.last_seq().unwrap();
        let result = update_resource_status(&h.runtime, &mut h.ctx, |agent, ctx| {
            agent.transition(
                if leaving {
                    AgentStatus::Running
                } else {
                    resource_test_status()
                },
                now(),
            )?;
            Ok(Some(
                Commit::new()
                    .event(ctx.draft(EventKind::ResourceAcquired { keys: vec![] }))
                    .mutate(Mutation::UpsertAgent(agent.clone()))
                    .mutate(Mutation::DeliverInbox(aether_core::inbox::InboxItem {
                        id: aether_core::id::InboxItemId::new(),
                        agent_id: agent.id,
                        seq: 5,
                        kind: InboxKind::Wake {
                            reason: "invalid sequence".into(),
                        },
                        delivered_at: now(),
                    })),
            ))
        });
        assert!(matches!(
            result,
            Err(RuntimeError::Store(StoreError::InboxSequence { .. }))
        ));
        assert_eq!(h.ctx.agent, before);
        assert_eq!(h.runtime.store.agent(before.id).unwrap(), Some(before));
        assert_eq!(
            h.runtime.store.last_seq().unwrap(),
            seq,
            "no ghost acquisition event"
        );
    }
}

fn resource_test_status() -> AgentStatus {
    AgentStatus::Blocked {
        reason: BlockReason::Resource {
            holder: AgentId::new(),
            key: ResourceKey("status-time-fixture".into()),
        },
    }
}

#[tokio::test]
async fn status_since_resource_transaction_serializes_queued_operator_writes() {
    use std::sync::mpsc;
    for leaving in [false, true] {
        for cancel in [false, true] {
            let (mut h, _dir) = ComposeHarness::new().await;
            h.ctx.agent.transition(AgentStatus::Running, now()).unwrap();
            if leaving {
                h.ctx
                    .agent
                    .transition(resource_test_status(), now())
                    .unwrap();
            }
            h.runtime
                .apply(Commit::new().mutate(Mutation::UpsertAgent(h.ctx.agent.clone())))
                .unwrap();
            let runtime = h.runtime.clone();
            let id = h.ctx.agent.id;
            let mut writer = None;
            let mut finished = None;
            update_resource_status(&h.runtime, &mut h.ctx, |agent, _ctx| {
                let (started_tx, started_rx) = mpsc::channel();
                let (done_tx, done_rx) = mpsc::channel();
                writer = Some(std::thread::spawn(move || {
                    started_tx.send(()).unwrap();
                    runtime
                        .update_agent(
                            id,
                            crate::AgentUpdate {
                                name: Some("operator wins".into()),
                                token_budget: Some(900_000),
                                ..Default::default()
                            },
                            Actor::user(),
                        )
                        .unwrap();
                    runtime.pause_agent(id, false, Actor::user()).unwrap();
                    if cancel {
                        runtime
                            .cancel_agent_now(id, Some("operator stop".into()), Actor::user())
                            .unwrap();
                    }
                    done_tx.send(()).unwrap();
                }));
                started_rx.recv_timeout(Duration::from_secs(10)).unwrap();
                // The callback is paused after its read; the operator cannot commit in that gap.
                assert_eq!(
                    done_rx.recv_timeout(Duration::from_millis(100)),
                    Err(mpsc::RecvTimeoutError::Timeout)
                );
                agent.transition(
                    if leaving {
                        AgentStatus::Running
                    } else {
                        resource_test_status()
                    },
                    now(),
                )?;
                finished = Some(done_rx);
                Ok(Some(Commit::new()))
            })
            .unwrap();
            finished
                .unwrap()
                .recv_timeout(Duration::from_secs(10))
                .unwrap();
            writer.unwrap().join().unwrap();
            let latest = h.runtime.store.agent(id).unwrap().unwrap();
            assert_eq!(latest.name, "operator wins");
            assert_eq!(latest.config.token_budget, Some(900_000));
            assert!(latest.paused);
            if cancel {
                assert!(matches!(latest.status, AgentStatus::Cancelled { .. }));
                assert_eq!(latest.status_since, Some(latest.updated_at));
            } else {
                assert_eq!(latest.status, h.ctx.agent.status);
                assert_eq!(latest.status_since, h.ctx.agent.status_since);
            }
        }
    }
}

#[tokio::test]
async fn status_since_resource_callbacks_do_not_revive_terminal_or_announce_acquisition() {
    for status in [
        AgentStatus::Cancelled { reason: None },
        AgentStatus::Failed {
            error: "stopped".into(),
        },
        AgentStatus::Completed { summary: None },
    ] {
        let (mut h, _dir) = ComposeHarness::new().await;
        h.ctx.agent.transition(AgentStatus::Running, now()).unwrap();
        h.ctx
            .agent
            .transition(resource_test_status(), now())
            .unwrap();
        let before = h.ctx.agent.clone();
        let mut terminal = before.clone();
        terminal.status = status;
        terminal.status_since = Some(now());
        h.runtime
            .apply(Commit::new().mutate(Mutation::UpsertAgent(terminal.clone())))
            .unwrap();
        let seq = h.runtime.store.last_seq().unwrap();
        let key = ResourceKey("status-time-fixture".into());
        leave_resource_block(
            &h.runtime,
            &mut h.ctx,
            Some(std::slice::from_ref(&key)),
            "available".into(),
        )
        .unwrap();
        let conflict = crate::resources::ResourceConflict {
            holder: AgentId::new(),
            key: key.clone(),
        };
        assert!(block_on_resource(&h.runtime, &mut h.ctx, &[key], &conflict).is_err());
        assert_eq!(h.runtime.store.agent(terminal.id).unwrap(), Some(terminal));
        assert_eq!(h.ctx.agent, before);
        assert_eq!(h.runtime.store.last_seq().unwrap(), seq);
    }
}

#[tokio::test]
async fn status_since_resource_callbacks_read_cancellation_after_queued_hold() {
    use std::sync::mpsc;
    for leaving in [false, true] {
        let (mut h, _dir) = ComposeHarness::new().await;
        h.ctx.agent.transition(AgentStatus::Running, now()).unwrap();
        if leaving {
            h.ctx
                .agent
                .transition(resource_test_status(), now())
                .unwrap();
        }
        h.runtime
            .apply(Commit::new().mutate(Mutation::UpsertAgent(h.ctx.agent.clone())))
            .unwrap();
        let before = h.ctx.agent.clone();
        let hold = h.runtime.hold();
        let seq = h.runtime.store.last_seq().unwrap();
        let runtime = h.runtime.clone();
        let (started_tx, started_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let callback = std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            let key = ResourceKey("status-time-fixture".into());
            let conflict = crate::resources::ResourceConflict {
                holder: AgentId::new(),
                key: key.clone(),
            };
            let result = if leaving {
                leave_resource_block(
                    &runtime,
                    &mut h.ctx,
                    Some(std::slice::from_ref(&key)),
                    "available".into(),
                )
            } else {
                block_on_resource(&runtime, &mut h.ctx, &[key], &conflict)
            };
            done_tx.send((h.ctx.agent, result)).unwrap();
        });
        started_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        let mut terminal = before.clone();
        terminal.name = "operator cancelled".into();
        terminal.config.token_budget = Some(900_000);
        terminal.paused = true;
        terminal
            .transition(AgentStatus::Cancelled { reason: None }, now())
            .unwrap();
        h.runtime
            .apply_held(
                hold,
                Commit::new().mutate(Mutation::UpsertAgent(terminal.clone())),
            )
            .unwrap();
        let (ctx, result) = done_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        callback.join().unwrap();
        assert_eq!(result.is_ok(), leaving);
        assert_eq!(ctx, before);
        assert_eq!(h.runtime.store.agent(terminal.id).unwrap(), Some(terminal));
        assert_eq!(
            h.runtime.store.last_seq().unwrap(),
            seq,
            "no false resource acquisition"
        );
    }
}
