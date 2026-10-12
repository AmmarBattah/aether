use super::*;

/// A thread carried over from earlier turns must not be billed again. The
/// baseline is what the previous turn *persisted*, never a figure derived
/// from this turn's first update.
#[test]
fn a_resumed_thread_bills_only_what_this_run_consumed() {
    let mut s = TurnState::new(resumed(Usage {
        input_tokens: 400,
        output_tokens: 880,
        cache_read_tokens: 39_600,
        ..Default::default()
    }));
    s.thread_id = Some("th".into());
    let events = s.notification(
        "thread/tokenUsage/updated",
        &json!({"tokenUsage": {
            "last": {"inputTokens": 500, "cachedInputTokens": 400, "outputTokens": 20},
            "total": {"inputTokens": 40_500, "cachedInputTokens": 40_000, "outputTokens": 900},
        }}),
    );
    assert_eq!(
        billed(&events),
        vec![Usage {
            input_tokens: 100,
            output_tokens: 20,
            cache_read_tokens: 400,
            ..Default::default()
        }],
        "the 40k the thread spent before this run is not this run's"
    );
    // A second request in the same run bills only its own increase.
    let events = s.notification(
        "thread/tokenUsage/updated",
        &json!({"tokenUsage": {
            "last": {"inputTokens": 300, "cachedInputTokens": 250, "outputTokens": 5},
            "total": {"inputTokens": 40_800, "cachedInputTokens": 40_250, "outputTokens": 905},
        }}),
    );
    assert_eq!(
        billed(&events),
        vec![Usage {
            input_tokens: 50,
            output_tokens: 5,
            cache_read_tokens: 250,
            ..Default::default()
        }]
    );
}

/// The shape 2,843 of 4,040 turns in the operator's own `~/.codex`
/// rollouts open with: the turn's first `token_count` is a verbatim echo
/// of the previous turn's final counters, `last` included. Deriving the
/// baseline as `total - last` bills that whole previous request here -
/// up to ~150k tokens against a turn that has made no request yet.
#[test]
fn an_echoed_first_update_bills_the_previous_turns_request_to_nobody() {
    let persisted = Usage {
        input_tokens: 6_000_000,
        output_tokens: 747_751,
        cache_read_tokens: 0,
        ..Default::default()
    };
    let mut s = TurnState::new(resumed(persisted));
    s.thread_id = Some("th".into());
    let echo = json!({"tokenUsage": {
        "last": {"inputTokens": 149_000, "cachedInputTokens": 0, "outputTokens": 623},
        "total": {"inputTokens": 6_000_000, "cachedInputTokens": 0, "outputTokens": 747_751},
    }});
    assert!(
        billed(&s.notification("thread/tokenUsage/updated", &echo)).is_empty(),
        "a turn that has made no request owes nothing"
    );
    // The run's own first request, on top of the same counters.
    let real = json!({"tokenUsage": {
        "last": {"inputTokens": 1_000, "cachedInputTokens": 0, "outputTokens": 100},
        "total": {"inputTokens": 6_001_000, "cachedInputTokens": 0, "outputTokens": 747_851},
    }});
    assert_eq!(
        billed(&s.notification("thread/tokenUsage/updated", &real)),
        vec![Usage {
            input_tokens: 1_000,
            output_tokens: 100,
            ..Default::default()
        }],
        "its own request, and only that"
    );
}

/// A thread counter that restarts mid-run (a compaction, a new thread)
/// opens a new segment: both halves are billed, and nothing is refunded.
#[test]
fn a_restarted_thread_counter_bills_both_halves() {
    let mut s = TurnState::new(turn(None, false));
    s.thread_id = Some("th".into());
    let update = |input: u64, output: u64| {
        json!({"tokenUsage": {
            "last": {"inputTokens": 1, "cachedInputTokens": 0, "outputTokens": 1},
            "total": {"inputTokens": input, "cachedInputTokens": 0, "outputTokens": output},
        }})
    };
    let mut total = Usage::default();
    for (input, output) in [(90_000, 10_000), (1_000, 100), (5_000, 500)] {
        for usage in billed(&s.notification("thread/tokenUsage/updated", &update(input, output))) {
            total += usage;
        }
    }
    assert_eq!(
        total.total_tokens(),
        100_000 + 5_500,
        "the restarted counter is billed too, from zero"
    );
    let completed = json!({"turn": {"id": "t1", "status": "completed", "items": []}});
    let EngineEvent::Done(outcome) = &s.notification("turn/completed", &completed)[0] else {
        panic!("expected the turn to end");
    };
    assert_eq!(
        outcome.usage.total_tokens(),
        105_500,
        "and it is the run total"
    );
}

/// A usage update missing the fields billing reads is a protocol change,
/// not a zero: it is said out loud instead of silently locking in nothing.
#[test]
fn a_usage_update_missing_its_fields_is_said_not_swallowed() {
    let mut s = TurnState::new(turn(None, false));
    s.thread_id = Some("th".into());
    let no_total = json!({"tokenUsage": {"last": {"inputTokens": 5, "outputTokens": 1}}});
    let events = s.notification("thread/tokenUsage/updated", &no_total);
    assert!(billed(&events).is_empty());
    assert!(
        matches!(&events[0], EngineEvent::ProtocolNote(n) if n.contains("no `total`")),
        "{events:?}"
    );
    let no_last = json!({"tokenUsage": {
        "total": {"inputTokens": 800, "cachedInputTokens": 0, "outputTokens": 12},
    }});
    let events = s.notification("thread/tokenUsage/updated", &no_last);
    assert_eq!(
        billed(&events),
        vec![Usage {
            input_tokens: 800,
            output_tokens: 12,
            ..Default::default()
        }],
        "billing does not depend on `last`"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, EngineEvent::ProtocolNote(n) if n.contains("no `last`"))),
        "{events:?}"
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, EngineEvent::ContextUsage { .. })),
        "no context size can be reported without it: {events:?}"
    );
}

/// A turn that never reported usage at all is billed zero - loudly. Silent
/// zero billing is a budget that has stopped binding.
#[test]
fn a_turn_that_reported_no_usage_says_so() {
    let mut s = TurnState::new(turn(None, false));
    s.thread_id = Some("th".into());
    let completed = json!({"turn": {"id": "t1", "status": "completed", "items": []}});
    let events = s.notification("turn/completed", &completed);
    assert!(
        matches!(&events[0], EngineEvent::ProtocolNote(n) if n.contains("without reporting any token usage")),
        "{events:?}"
    );
    assert!(matches!(events[1], EngineEvent::Done(_)));
}

/// An interrupted run keeps what it consumed: partial usage stands, and a
/// repeat of the same totals adds nothing.
#[test]
fn a_failed_turn_reports_what_it_consumed_and_repeats_add_nothing() {
    let mut s = TurnState::new(turn(None, false));
    s.thread_id = Some("th".into());
    let usage = json!({"tokenUsage": {
        "last": {"inputTokens": 100, "cachedInputTokens": 0, "outputTokens": 10},
        "total": {"inputTokens": 100, "cachedInputTokens": 0, "outputTokens": 10},
    }});
    assert_eq!(
        billed(&s.notification("thread/tokenUsage/updated", &usage)).len(),
        1
    );
    let repeat = s.notification("thread/tokenUsage/updated", &usage);
    assert!(
        billed(&repeat).is_empty(),
        "an unchanged total is not more spend: {repeat:?}"
    );
    let failed = json!({"turn": {"id": "t1", "status": "failed", "error": {"message": "cut off"}}});
    let EngineEvent::Done(outcome) = &s.notification("turn/completed", &failed)[0] else {
        panic!("expected the turn to end");
    };
    assert!(outcome.is_error);
    assert_eq!(outcome.usage.total_tokens(), 110, "no silent undercount");
}

/// Seven requests, one usage record. Codex reports the thread's
/// running totals, and a tool-only request produces no assistant message
/// to hang usage on - so the totals themselves have to be the ledger.
#[test]
fn every_request_is_billed_once_including_the_tool_only_ones() {
    let mut s = TurnState::new(turn(None, false));
    s.thread_id = Some("th".into());
    let update = |input: u64, cached: u64, output: u64| {
        json!({"tokenUsage": {
            "last": {"inputTokens": 1, "cachedInputTokens": 0, "outputTokens": 1},
            "total": {"inputTokens": input, "cachedInputTokens": cached, "outputTokens": output},
        }})
    };
    let mut billed = Usage::default();
    // First update seeds the baseline: this run starts from a fresh thread.
    for (input, cached, output) in [(1, 0, 1), (900, 128, 40), (129_306, 113_664, 1_046)] {
        for event in s.notification("thread/tokenUsage/updated", &update(input, cached, output)) {
            if let EngineEvent::UsageDelta { usage } = event {
                billed += usage;
            }
        }
    }
    assert_eq!(billed.input_tokens, 129_306 - 113_664);
    assert_eq!(billed.cache_read_tokens, 113_664);
    assert_eq!(billed.output_tokens, 1_046);
    assert_eq!(
        billed.total_tokens(),
        130_352,
        "the authoritative run total"
    );
    let completed = json!({"turn": {"id": "t1", "status": "completed", "items": []}});
    let EngineEvent::Done(outcome) = &s.notification("turn/completed", &completed)[0] else {
        panic!("expected the turn to end");
    };
    // The outcome reconciles against the same figure, never on top of it.
    assert_eq!(outcome.usage.total_tokens(), 130_352);
}

/// Usage that arrives after the item it paid for is still this run's:
/// decoupling the two is what makes a tool-only request billable.
#[test]
fn usage_arriving_after_an_item_completed_is_still_billed() {
    let mut s = TurnState::new(turn(None, false));
    s.thread_id = Some("th".into());
    let started = json!({"item": {"type": "commandExecution", "id": "c1", "command": "ls", "cwd": "/w", "status": "inProgress"}});
    s.notification("item/started", &started);
    let done = json!({"item": {"type": "commandExecution", "id": "c1", "command": "ls", "aggregatedOutput": "a\n", "exitCode": 0, "status": "completed"}});
    let completed = s.notification("item/completed", &done);
    assert!(
        matches!(&completed[0], EngineEvent::ToolResult { tool_use_id, .. } if tool_use_id == "c1"),
        "{completed:?}"
    );
    assert!(
        billed(&completed).is_empty(),
        "the completed tool carries no usage of its own: {completed:?}"
    );
    let usage = json!({"tokenUsage": {
        "last": {"inputTokens": 800, "cachedInputTokens": 0, "outputTokens": 12},
        "total": {"inputTokens": 800, "cachedInputTokens": 0, "outputTokens": 12},
    }});
    assert_eq!(
        billed(&s.notification("thread/tokenUsage/updated", &usage)),
        vec![Usage {
            input_tokens: 800,
            output_tokens: 12,
            ..Default::default()
        }]
    );
}

/// Codex counts reasoning inside `outputTokens` (its `totalTokens` is input
/// plus output), as the OpenAI API does. Priced at the output rate, reasoning
/// must stay inside `output_tokens` and never be added on top of it.
#[test]
fn reasoning_is_counted_inside_output_never_on_top_of_it() {
    let mut s = TurnState::new(turn(None, false));
    s.thread_id = Some("th".into());
    // A real app-server reading from the operator's own rollouts.
    let reading = json!({
        "inputTokens": 116_725, "cachedInputTokens": 91_008,
        "outputTokens": 740, "reasoningOutputTokens": 44, "totalTokens": 117_465,
    });
    let events = s.notification(
        "thread/tokenUsage/updated",
        &json!({"tokenUsage": {"last": reading, "total": reading}}),
    );
    let usage = Usage {
        input_tokens: 116_725 - 91_008,
        output_tokens: 740,
        cache_read_tokens: 91_008,
        reasoning_tokens: Some(44),
        ..Default::default()
    };
    assert_eq!(billed(&events), vec![usage]);
    assert_eq!(usage.total_tokens(), 117_465, "Codex's own total");
}
