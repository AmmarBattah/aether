use super::*;

#[test]
fn another_runs_events_never_reach_the_document() {
    let mut log = Log::new();
    let mine = AgentId::new();
    log.push(created(mine, None, "mine"));
    let other = Event::from_draft(
        EventDraft {
            run: Some(RunId::new()),
            ..created(AgentId::new(), None, "theirs")
        },
        EventId::new(),
        99,
        log.at,
    );
    log.events.push(other);
    let report = log.report();
    assert_eq!(report.body.agents.len(), 1);
    assert_eq!(report.body.agents[0].name, "mine");
    assert_eq!(report.body.events, 1);
}

#[test]
fn the_same_run_in_two_stores_has_one_canonical_body() {
    let mut log = Log::new();
    let (root, child) = (AgentId::new(), AgentId::new());
    log.push(created(root, None, "root"));
    log.push(created(child, Some(root), "child"));
    log.push(spent(child, 0.5));
    let first = log.report();

    // The same story under fresh ids and a different local sequence, as
    // an import would write it.
    let map: BTreeMap<AgentId, AgentId> = [(root, AgentId::new()), (child, AgentId::new())].into();
    let run = RunId::new();
    let events: Vec<Event> = log
        .events
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let mut json = serde_json::to_value(e).expect("event to json");
            let text = json.to_string();
            let mut swapped = text;
            for (from, to) in &map {
                swapped = swapped.replace(&from.to_string(), &to.to_string());
            }
            swapped = swapped.replace(&log.run.to_string(), &run.to_string());
            json = serde_json::from_str(&swapped).expect("swapped json");
            let mut event: Event = serde_json::from_value(json).expect("event");
            event.seq = 500 + i as u64;
            event.id = EventId::new();
            event
        })
        .collect();
    let second = build(run, &events, log.at);

    assert_ne!(first.run, second.run);
    assert_ne!(first.body, second.body, "raw bodies carry local ids");
    assert_eq!(
        first.canonical_json().expect("canonical"),
        second.canonical_json().expect("canonical")
    );
}

/// `canonical_json` used to swallow a serialization failure into
/// `Value::Null`, which would make two unrelated reports compare equal -
/// silently passing the one test the function exists for. It returns a
/// `Result` now and the infallible form panics.
///
/// The failure the review expected - a non-finite `cost_usd` - does not
/// actually reach it: `serde_json::to_value` maps a non-finite f64 to
/// `Value::Null` rather than refusing it. Pinned here because that also
/// means a nonsense cost is indistinguishable from an unreported one in
/// a canonical body, and because a future serde that starts refusing it
/// should surface as this test failing, not as two reports matching.
#[test]
fn a_non_finite_cost_becomes_null_rather_than_failing() {
    let mut report = Log::new().report();
    report.body.usage.cost_usd = f64::NAN;
    let body = report.canonical_json().expect("serde coerces it");
    assert_eq!(body["usage"]["cost_usd"], serde_json::Value::Null);
}

#[test]
fn the_document_is_stable_against_a_fixture_log() {
    let events: Vec<Event> =
        serde_json::from_str(include_str!("../../../tests/fixtures/run_log.json"))
            .expect("fixture log");
    let run = events
        .iter()
        .find_map(|e| e.run)
        .expect("the fixture names a run");
    let generated_at: Timestamp = "2026-02-01T12:00:00Z".parse().expect("timestamp");
    let report = build(run, &events, generated_at);
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/run_report.json"))
            .expect("fixture report");
    assert_eq!(
        report.canonical_json().expect("canonical"),
        expected,
        "the report of a fixed log changed; update the fixture deliberately"
    );
}
