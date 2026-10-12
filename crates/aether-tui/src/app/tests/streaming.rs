//! streaming reducer regressions.
use super::*;
use aether_core::model::{ContentBlock, MessageRole};
use aether_protocol::Delta;

use crate::app::transcript::CallStatus;

fn feed(app: &mut App, seq: u64, agent: AgentId, kind: EventKind) {
    app.reduce(AppEvent::Frame(SubscriptionFrame::Event(Box::new(event(
        seq, agent, kind,
    )))));
}

fn delta(app: &mut App, agent: AgentId, invocation: InvocationId, delta: Delta) {
    app.reduce(AppEvent::Frame(SubscriptionFrame::Delta {
        agent,
        invocation,
        delta,
    }));
}

#[test]
fn deltas_stream_then_durable_message_replaces_live_text() {
    let (mut app, id) = app_with_agent();
    let inv = InvocationId::new();
    delta(&mut app, id, inv, Delta::TurnStarted { turn: 1 });
    delta(&mut app, id, inv, Delta::Text { text: "Hel".into() });
    delta(&mut app, id, inv, Delta::Text { text: "lo".into() });
    assert_eq!(app.transcripts[&id].stream.text, "Hello");
    assert!(models(&app).is_empty(), "streamed text is not a cell yet");

    let msg = || EventKind::AgentMessage {
        role: MessageRole::Assistant,
        content: vec![ContentBlock::text("Hello")],
        invocation: Some(inv),
    };
    feed(&mut app, 5, id, msg());
    assert!(
        app.transcripts[&id].stream.is_empty(),
        "the live text is replaced by the durable message"
    );
    assert_eq!(
        models(&app),
        vec![CellModel::Assistant {
            text: "Hello".into()
        }]
    );
    assert_eq!(app.last_seq, Some(5));
    // Replaying the same seq is a no-op.
    feed(&mut app, 5, id, msg());
    assert_eq!(models(&app).len(), 1);
}

#[test]
fn tool_lifecycle_updates_one_cell() {
    let (mut app, id) = app_with_agent();
    let call = ToolCall {
        id: ToolCallId::new(),
        provider_call_id: "toolu_1".into(),
        name: "bash".into(),
        input: serde_json::json!({"cmd": "ls"}),
        agent_id: id,
        task_id: None,
        invocation_id: None,
        resources: vec![],
        idempotency_key: None,
        requested_at: now(),
    };
    let call_id = call.id;
    feed(
        &mut app,
        2,
        id,
        EventKind::ToolRequested {
            call,
            side_effect: SideEffectClass::Mutation,
        },
    );
    feed(&mut app, 3, id, EventKind::ToolStarted { call: call_id });
    let [
        CellModel::Exec {
            command, status, ..
        },
    ] = &models(&app)[..]
    else {
        panic!("one exec cell: {:?}", models(&app));
    };
    assert_eq!(command, "ls");
    assert_eq!(*status, CallStatus::Running);

    let outcome = ToolOutcome::ok("file.txt").with_duration(120);
    feed(
        &mut app,
        4,
        id,
        EventKind::ToolSucceeded {
            call: call_id,
            outcome,
        },
    );
    // The durable transcript later repeats the tool use and result; no duplicates.
    feed(
        &mut app,
        5,
        id,
        EventKind::AgentMessage {
            role: MessageRole::Assistant,
            content: vec![ContentBlock::ToolUse {
                id: "toolu_1".into(),
                name: "bash".into(),
                input: serde_json::json!({"cmd": "ls"}),
            }],
            invocation: None,
        },
    );
    feed(
        &mut app,
        6,
        id,
        EventKind::AgentMessage {
            role: MessageRole::User,
            content: vec![ContentBlock::tool_result_text("toolu_1", "file.txt", false)],
            invocation: None,
        },
    );
    let [
        CellModel::Exec {
            status,
            output,
            duration_ms,
            ..
        },
    ] = &models(&app)[..]
    else {
        panic!("still one exec cell: {:?}", models(&app));
    };
    assert_eq!(*status, CallStatus::Succeeded);
    assert_eq!(output, "file.txt");
    assert_eq!(*duration_ms, Some(120));
}

/// A task attaching to an existing agent changes no cell, so a layout
/// cached by cell revision alone would keep showing the raw id after the
/// client has learned the objective.
#[test]
fn a_task_attaching_to_a_known_agent_resolves_its_id_on_screen() {
    use crate::testing::render_with;
    let mut cache = crate::ui::transcript::ViewCache::default();
    let (mut app, id) = app_with_agent();
    let task_id = aether_core::id::TaskId::new();
    feed(
        &mut app,
        2,
        id,
        EventKind::AgentCancelled {
            reason: Some(format!("waiting on {task_id}")),
        },
    );
    let text = render_with(&app, &mut cache, 160, 40);
    assert!(
        text.contains(&task_id.to_string()),
        "before the tree knows the task, its id stays on screen:\n{text}"
    );

    // The same agent, same node key, now with the task attached.
    let mut task =
        aether_core::task::Task::new("get the render honest", Actor::System, RunId::new(), now());
    task.id = task_id;
    let mut next = tree(agent(id, "root"));
    next.nodes.get_mut(&id).expect("the node").task = Some(task);
    app.reduce(AppEvent::Reply {
        then: Then::Tree,
        result: Ok(Reply::Tree(next)),
    });
    let text = render_with(&app, &mut cache, 160, 40);
    assert!(
        text.contains("get the render honest"),
        "a task attached to a known agent must resolve to its objective:\n{text}"
    );
    assert!(
        !text.contains(&task_id.to_string()),
        "the raw id must be gone once the objective is known:\n{text}"
    );
}

/// Each committed message empties the stream. Streams `first`, commits it,
/// then streams `second` under the same invocation, drawing with one cache
/// throughout as the runner does; returns the last frame.
fn second_message_in_one_turn(first: &str, second: &str) -> String {
    use crate::testing::render_with;
    let mut cache = crate::ui::transcript::ViewCache::default();
    let (mut app, id) = app_with_agent();
    let inv = InvocationId::new();
    delta(&mut app, id, inv, Delta::TurnStarted { turn: 1 });
    delta(&mut app, id, inv, Delta::Text { text: first.into() });
    render_with(&app, &mut cache, 100, 30);
    feed(
        &mut app,
        5,
        id,
        EventKind::AgentMessage {
            role: MessageRole::Assistant,
            content: vec![ContentBlock::text(first)],
            invocation: Some(inv),
        },
    );
    delta(
        &mut app,
        id,
        inv,
        Delta::Text {
            text: second.into(),
        },
    );
    render_with(&app, &mut cache, 100, 30)
}

#[test]
fn the_next_message_in_a_turn_does_not_reuse_the_last_ones_rendering() {
    let text = second_message_in_one_turn("Hello", "Goodbye world");
    assert!(text.contains("Goodbye world"), "{text}");
    assert!(
        !text.contains("Hello world"),
        "the second reply was drawn as a continuation of the first:\n{text}"
    );
}

#[test]
fn a_shorter_first_message_cannot_split_a_character_of_the_next() {
    // Byte 3 of the second text falls inside the multi-byte apostrophe.
    let text = second_message_in_one_turn("Hi.", "I\u{2019}ll check the logs.");
    assert!(text.contains("I\u{2019}ll check the logs."), "{text}");
}

/// A native engine turn as the daemon streams it: each message's deltas under
/// the invocation it commits as, tool calls between. Committed text leaves the
/// live block, so nothing stays pinned below the tool cells.
#[test]
fn an_engine_turn_leaves_no_live_text_below_its_tool_calls() {
    use crate::testing::render_with;
    let mut cache = crate::ui::transcript::ViewCache::default();
    let (mut app, id) = app_with_agent();
    let (first, second) = (InvocationId::new(), InvocationId::new());
    delta(&mut app, id, first, Delta::TurnStarted { turn: 1 });
    let said = "Let me list the files first.";
    delta(&mut app, id, first, Delta::Text { text: said.into() });
    render_with(&app, &mut cache, 100, 30);
    feed(
        &mut app,
        5,
        id,
        EventKind::AgentMessage {
            role: MessageRole::Assistant,
            content: vec![ContentBlock::text(said)],
            invocation: Some(first),
        },
    );
    let call = ToolCall {
        id: ToolCallId::new(),
        provider_call_id: "toolu_1".into(),
        name: "Bash".into(),
        input: serde_json::json!({"command": "ls"}),
        agent_id: id,
        task_id: None,
        invocation_id: Some(first),
        resources: vec![],
        idempotency_key: None,
        requested_at: now(),
    };
    let call_id = call.id;
    feed(
        &mut app,
        6,
        id,
        EventKind::ToolRequested {
            call,
            side_effect: SideEffectClass::Mutation,
        },
    );
    feed(&mut app, 7, id, EventKind::ToolStarted { call: call_id });
    feed(
        &mut app,
        8,
        id,
        EventKind::ToolSucceeded {
            call: call_id,
            outcome: ToolOutcome::ok("README.md"),
        },
    );
    let text = render_with(&app, &mut cache, 100, 30);
    assert_eq!(text.matches(said).count(), 1, "{text}");

    let summary = "The workspace holds one file.";
    delta(
        &mut app,
        id,
        second,
        Delta::Text {
            text: summary.into(),
        },
    );
    let text = render_with(&app, &mut cache, 100, 30);
    assert_eq!(text.matches(said).count(), 1, "{text}");
    feed(
        &mut app,
        9,
        id,
        EventKind::AgentMessage {
            role: MessageRole::Assistant,
            content: vec![ContentBlock::text(summary)],
            invocation: Some(second),
        },
    );
    delta(&mut app, id, InvocationId::new(), Delta::TurnFinished);
    assert!(app.transcripts[&id].stream.is_empty());
    let text = render_with(&app, &mut cache, 100, 30);
    assert_eq!(text.matches(said).count(), 1, "{text}");
    assert_eq!(text.matches(summary).count(), 1, "{text}");
    let ran = text.find("Ran ls").expect("the tool cell");
    assert!(
        text.find(said) < Some(ran) && Some(ran) < text.find(summary),
        "{text}"
    );
}

/// Codex streams reasoning before a tool-only and a text-only message; the
/// runtime commits each with that reasoning folded in. Each message's live
/// reasoning retires on commit and is drawn once, as its Reasoning cell.
#[test]
fn engine_reasoning_streamed_before_a_commit_is_kept_once() {
    let (mut app, id) = app_with_agent();
    let (first, second) = (InvocationId::new(), InvocationId::new());
    let thinking = |text: &str| ContentBlock::Thinking {
        text: text.into(),
        signature: None,
        redacted: false,
    };
    let (plan, wrap_up) = ("Listing the files first.", "One file; summarise.");
    let summary = "The workspace holds one file.";
    delta(&mut app, id, first, Delta::TurnStarted { turn: 1 });
    delta(&mut app, id, first, Delta::Thinking { text: plan.into() });
    feed(
        &mut app,
        5,
        id,
        EventKind::AgentMessage {
            role: MessageRole::Assistant,
            content: vec![
                thinking(plan),
                ContentBlock::ToolUse {
                    id: "call_1".into(),
                    name: "bash".into(),
                    input: serde_json::json!({"command": "ls"}),
                },
            ],
            invocation: Some(first),
        },
    );
    assert!(app.transcripts[&id].stream.is_empty());
    delta(
        &mut app,
        id,
        second,
        Delta::Thinking {
            text: wrap_up.into(),
        },
    );
    feed(
        &mut app,
        6,
        id,
        EventKind::AgentMessage {
            role: MessageRole::Assistant,
            content: vec![thinking(wrap_up), ContentBlock::text(summary)],
            invocation: Some(second),
        },
    );
    delta(&mut app, id, InvocationId::new(), Delta::TurnFinished);

    assert!(app.transcripts[&id].stream.is_empty());
    let drawn: Vec<(&str, String)> = models(&app)
        .into_iter()
        .filter_map(|model| match model {
            CellModel::Reasoning { text, .. } => Some(("reasoning", text)),
            CellModel::Assistant { text } => Some(("assistant", text)),
            _ => None,
        })
        .collect();
    assert_eq!(
        drawn,
        vec![
            ("reasoning", plan.to_owned()),
            ("reasoning", wrap_up.to_owned()),
            ("assistant", summary.to_owned()),
        ]
    );
}

/// Archiving drops an agent's transcript; later events start a new one whose
/// cell ids restart, so a layout cached for the first life must not be reused.
#[test]
fn a_transcript_recreated_after_archiving_lays_out_afresh() {
    use crate::testing::render_with;
    let mut cache = crate::ui::transcript::ViewCache::default();
    let (mut app, id) = app_with_agent();
    let say = |text: &str| EventKind::AgentMessage {
        role: MessageRole::Assistant,
        content: vec![ContentBlock::text(text)],
        invocation: None,
    };
    feed(&mut app, 2, id, say("from the first life"));
    let text = render_with(&app, &mut cache, 100, 20);
    assert!(text.contains("from the first life"), "{text}");

    feed(&mut app, 3, id, EventKind::AgentArchived { archived: true });
    feed(&mut app, 4, id, say("from the second life"));
    let text = render_with(&app, &mut cache, 100, 20);
    assert!(text.contains("from the second life"), "{text}");
    assert!(
        !text.contains("first life"),
        "a stale layout was reused:\n{text}"
    );
}
