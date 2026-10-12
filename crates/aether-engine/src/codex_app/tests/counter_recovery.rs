use super::*;

/// A cumulative counter that dips by one token has not restarted, and the
/// fields that did not dip have not either. Re-baselining the whole record
/// billed the thread's entire history again here: 1,011,350 for 800.
#[test]
fn one_field_dipping_does_not_re_bill_the_thread() {
    let mut s = TurnState::new(resumed(Usage {
        input_tokens: 1_000_000,
        output_tokens: 10_000,
        ..Default::default()
    }));
    s.thread_id = Some("th".into());
    let mut billed = Usage::default();
    let mut raised = Vec::new();
    for total in [
        json!({"inputTokens": 1_000_500, "cachedInputTokens": 0, "outputTokens": 10_300}),
        // One token lower, everything else standing: a correction, not a
        // counter that went back to zero.
        json!({"inputTokens": 1_000_499, "cachedInputTokens": 0, "outputTokens": 10_300}),
        json!({"inputTokens": 1_000_500, "cachedInputTokens": 0, "outputTokens": 10_300}),
    ] {
        let (usage, notes) = account(&mut s, total);
        billed += usage;
        raised.extend(notes);
    }
    assert_eq!(
        billed.total_tokens(),
        800,
        "only what this run consumed, never the resumed baseline"
    );
    // Billing nothing for the dip is the rule; saying nothing about it is
    // not - the note is the only trace an invoice can be checked against.
    assert!(
        raised.iter().any(|n| n.contains("inputTokens by 1")),
        "the dipped counter and its depth are named: {raised:?}"
    );
    assert!(
        !raised.iter().any(|n| n.contains("restarted")),
        "a one-token dip is a correction, not a restart: {raised:?}"
    );
}

/// A field that is not a number is not zero. Read as zero it looks like a
/// restarted counter, and the whole thread gets billed again (500,150 for
/// a 50-token request); the previous reading stands for it instead.
#[test]
fn an_unreadable_field_carries_the_previous_reading_and_says_so() {
    let mut s = TurnState::new(resumed(Usage {
        input_tokens: 500_000,
        output_tokens: 5_000,
        ..Default::default()
    }));
    s.thread_id = Some("th".into());
    let (usage, notes) = account(
        &mut s,
        json!({"inputTokens": 500_000, "cachedInputTokens": 0, "outputTokens": "5015"}),
    );
    assert_eq!(usage, Usage::default(), "nothing readable, nothing billed");
    assert!(
        notes.iter().any(|n| n.contains("outputTokens")),
        "the field that could not be read is named: {notes:?}"
    );
    let (usage, _) = account(
        &mut s,
        json!({"inputTokens": 500_000, "cachedInputTokens": 0, "outputTokens": 5_050}),
    );
    assert_eq!(usage.total_tokens(), 50, "the real request, and only it");
}

/// A build that reports reasoning only on the updates that have any: the
/// absent updates are not a counter going backwards. Reading them as zero
/// re-billed the record on every other update (2.5x measured).
#[test]
fn reasoning_reported_only_when_there_is_some_is_not_a_restart() {
    let mut s = TurnState::new(turn(None, false));
    s.thread_id = Some("th".into());
    let mut billed = Usage::default();
    let mut raised = Vec::new();
    for total in [
        json!({"inputTokens": 1_000, "cachedInputTokens": 0, "outputTokens": 100, "reasoningOutputTokens": 50}),
        json!({"inputTokens": 2_000, "cachedInputTokens": 0, "outputTokens": 200}),
        json!({"inputTokens": 3_000, "cachedInputTokens": 0, "outputTokens": 300, "reasoningOutputTokens": 150}),
        json!({"inputTokens": 4_000, "cachedInputTokens": 0, "outputTokens": 400, "reasoningOutputTokens": 0}),
        json!({"inputTokens": 5_000, "cachedInputTokens": 0, "outputTokens": 500, "reasoningOutputTokens": 250}),
    ] {
        let (usage, notes) = account(&mut s, total);
        billed += usage;
        raised.extend(notes);
    }
    assert_eq!(billed.input_tokens, 5_000);
    assert_eq!(billed.output_tokens, 500);
    assert_eq!(billed.reasoning_tokens, Some(250));
    assert!(
        !raised.iter().any(|n| n.contains("restarted")),
        "nothing restarted: {raised:?}"
    );
}

/// Compaction: the cached counter goes to zero while the input counter
/// keeps climbing. Only the cached counter restarted, and the tokens it
/// already counted stay billed once - not twice (1,002,110 measured).
#[test]
fn a_compaction_restarts_only_the_counter_that_restarted() {
    let mut s = TurnState::new(turn(None, false));
    s.thread_id = Some("th".into());
    let (first, _) = account(
        &mut s,
        json!({"inputTokens": 500_000, "cachedInputTokens": 499_000, "outputTokens": 1_000}),
    );
    assert_eq!(first.cache_read_tokens, 499_000);
    let (second, notes) = account(
        &mut s,
        json!({"inputTokens": 501_000, "cachedInputTokens": 0, "outputTokens": 1_110}),
    );
    assert!(
        notes.iter().any(|n| n.contains("cachedInputTokens")),
        "the restarted counter is named: {notes:?}"
    );
    let mut billed = first;
    billed += second;
    assert_eq!(
        billed.total_tokens(),
        502_110,
        "the thread's own reading, billed once"
    );
    assert_eq!(billed.cache_read_tokens, 499_000, "cached tokens once each");
}

/// The shape the operator's own rollouts actually show - every counter
/// restarting together on a resumed thread - still opens a new segment.
#[test]
fn a_whole_counter_restart_on_a_resumed_thread_bills_both_segments() {
    let mut s = TurnState::new(resumed(Usage {
        input_tokens: 200_000,
        output_tokens: 2_000,
        ..Default::default()
    }));
    s.thread_id = Some("th".into());
    let mut billed = Usage::default();
    for total in [
        json!({"inputTokens": 200_500, "cachedInputTokens": 0, "outputTokens": 2_050}),
        json!({"inputTokens": 150, "cachedInputTokens": 0, "outputTokens": 20}),
        json!({"inputTokens": 400, "cachedInputTokens": 0, "outputTokens": 60}),
    ] {
        billed += account(&mut s, total).0;
    }
    assert_eq!(
        billed.total_tokens(),
        550 + 170 + 290,
        "the pre-restart segment, the restarted counter, and its growth"
    );
}

/// A sweep over counters that climb with occasional misreadings, permanent
/// downward corrections and the odd whole restart: the run's bill never
/// runs more than one request ahead of what the thread really spent, and
/// falls short of it only by what the counters themselves took back.
/// Modelled the way the corpus shows these counters behave - a misreading
/// is transient and the next reading of the same counter corrects it, a
/// correction is kept and the counter climbs on from the lower value, and
/// a restart happens from counters that dwarf one request's spend, which
/// is the only shape a restart is distinguishable from a correction in.
#[test]
fn a_noisy_climbing_counter_is_billed_within_one_request_of_the_truth() {
    const MAX_SPEND: u64 = 1_000;
    const MAX_OUT: u64 = 200;
    const STEPS: u64 = 300;
    let mut seed: u64 = 0x5eed_1234_9abc_def0;
    let mut rand = move |bound: u64| {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (seed >> 33) % bound
    };
    let mut s = TurnState::new(turn(None, false));
    s.thread_id = Some("th".into());
    let (mut counter_in, mut counter_cached, mut counter_out) = (0u64, 0u64, 0u64);
    let (mut real, mut billed, mut restarts, mut misread) = (0u64, Usage::default(), 0, 0);
    // Spend the thread's own counters took back and never reported again.
    let (mut taken_back, mut corrections) = (0u64, 0);
    let mut uncorrected = false;
    for step in 0..STEPS {
        let spend_in = 1 + rand(MAX_SPEND);
        let spend_out = 1 + rand(MAX_OUT);
        let cached = rand(spend_in);
        if !uncorrected && counter_in > 4 * MAX_SPEND && counter_out > 4 * MAX_OUT && rand(25) == 0
        {
            restarts += 1;
            counter_in = spend_in;
            counter_cached = cached;
            counter_out = spend_out;
        } else {
            counter_in += spend_in;
            counter_cached += cached;
            counter_out += spend_out;
        }
        real += spend_in + spend_out;
        // Two shapes of dip. A misreading is transient: the next reading
        // of the same counter corrects it. A correction is permanent - the
        // engine keeps climbing from the lower value, so nothing ever
        // brings the reading back up and the gap is never billed. Never on
        // the last step, which closes the books.
        let mut dip = 0;
        if counter_in > 200 && step + 1 < STEPS && rand(5) == 0 {
            dip = 1 + rand(50);
            if rand(3) == 0 {
                corrections += 1;
                counter_in -= dip;
                taken_back += dip;
                dip = 0;
            } else {
                misread += 1;
            }
        }
        uncorrected = dip > 0;
        billed += account(
            &mut s,
            json!({
                "inputTokens": counter_in - dip,
                "cachedInputTokens": counter_cached,
                "outputTokens": counter_out,
            }),
        )
        .0;
    }
    assert!(
        restarts > 0 && misread > 0 && corrections > 0,
        "every branch is exercised: {restarts} restarts, {misread} misreadings, \
         {corrections} corrections"
    );
    let billed = billed.total_tokens();
    // The bound the rule holds to. Over-billing is what it is built to
    // prevent; under-billing it cannot prevent at all - a counter that
    // takes spend back is the only figure there is, and what stops a run
    // spending on it is the bridge boundary and the turn's end.
    assert!(
        billed <= real + MAX_SPEND,
        "over-billed by more than one request: {billed} > {real}"
    );
    assert!(
        billed + taken_back >= real,
        "short by more than the counters took back: {billed} + {taken_back} < {real}"
    );
}

/// A restart is one contradiction, not two. Raising the dip note beside
/// the restart note spent two of the eight notes a turn has on one event.
#[test]
fn a_restarted_counter_raises_the_restart_note_alone() {
    let mut s = TurnState::new(turn(None, false));
    s.thread_id = Some("th".into());
    let climbing = json!({"inputTokens": 90_000, "cachedInputTokens": 0, "outputTokens": 10_000});
    let (_, quiet) = account(&mut s, climbing);
    assert!(
        quiet.is_empty(),
        "a climbing counter says nothing: {quiet:?}"
    );

    let restarted = json!({"inputTokens": 1_000, "cachedInputTokens": 0, "outputTokens": 100});
    let (_, notes) = account(&mut s, restarted);
    assert_eq!(notes.len(), 1, "one event, one note: {notes:?}");
    assert!(
        notes[0].contains("restarted"),
        "the note that says which way it was billed: {notes:?}"
    );
    assert!(
        notes[0].contains("inputTokens by 89000") && notes[0].contains("outputTokens by 9900"),
        "the note must say how far each field fell, not just that it did: {notes:?}"
    );
}
