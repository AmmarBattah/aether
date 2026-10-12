use super::*;
use serde_json::json;

fn receipt(session: &str, cost: f64) -> Value {
    json!({"session_id": session, "total_cost_usd": cost,
        "modelUsage": {"sonnet": {"inputTokens": 10, "outputTokens": 384,
            "cacheReadInputTokens": 78969, "cacheCreationInputTokens": 12906}}})
}

#[test]
fn fresh_receipt_preserves_cumulative_baseline() {
    let (cost, events) = SessionCost::default().receipt(&receipt("s", 0.0712778));
    assert_eq!(cost, Some(0.0712778));
    assert!(
        matches!(&events[..], [EngineEvent::SessionUsage { session, total }]
        if session == "s" && total.cost_usd == 0.0712778 && total.cost_complete
        && total.input_tokens == 10 && total.output_tokens == 384
        && total.cache_read_tokens == 78969 && total.cache_write_tokens == 12906)
    );
}

#[test]
fn resumed_receipts_subtract_initial_baseline_exactly_once() {
    let state = SessionCost::resumed(
        Some("s".into()),
        Usage {
            cost_usd: 0.0595702,
            ..Usage::default()
        },
    );
    let (cost, _) = state.receipt(&receipt("s", 0.0712778));
    assert!((cost.unwrap() - 0.0117076).abs() < 1e-12);
    // A later result in the same invocation still covers the entire invocation.
    let (cost, _) = state.receipt(&receipt("s", 0.08));
    assert!((cost.unwrap() - 0.0204298).abs() < 1e-12);
}

#[test]
fn unknown_or_reset_baseline_does_not_charge_old_session_history() {
    for prior in [
        Usage {
            cost_complete: false,
            ..Usage::default()
        },
        Usage {
            cost_usd: 1.0,
            ..Usage::default()
        },
    ] {
        let state = SessionCost::resumed(Some("s".into()), prior);
        let (cost, events) = state.receipt(&receipt("s", 0.5));
        assert_eq!(cost, None);
        assert!(matches!(&events[..], [EngineEvent::ProtocolNote(_),
            EngineEvent::SessionUsage { total, .. }] if total.cost_usd == 0.5));
    }
}

#[test]
fn replacement_session_starts_a_new_cost_history() {
    let state = SessionCost::resumed(
        Some("old".into()),
        Usage {
            cost_usd: 5.0,
            ..Usage::default()
        },
    );
    assert_eq!(state.receipt(&receipt("new", 0.2)).0, Some(0.2));
}

#[test]
fn missing_or_invalid_receipt_never_invents_a_price() {
    let state = SessionCost::default();
    assert_eq!(state.receipt(&json!({})), (None, vec![]));
    let (cost, events) = state.receipt(&receipt("s", -1.0));
    assert_eq!(cost, None);
    assert!(matches!(&events[..], [EngineEvent::ProtocolNote(_)]));
}
