//! Claude reports per-invocation usage but a session-cumulative dollar total.

use aether_core::model::Usage;
use serde_json::Value;

use crate::EngineEvent;

#[derive(Debug)]
pub(super) struct SessionCost {
    session: Option<String>,
    before: Option<f64>,
}

impl Default for SessionCost {
    fn default() -> Self {
        Self {
            session: None,
            before: Some(0.0),
        }
    }
}

impl SessionCost {
    pub(super) fn resumed(session: Option<String>, usage: Usage) -> Self {
        let before = if session.is_none() {
            Some(0.0)
        } else {
            usage.cost_complete.then_some(usage.cost_usd)
        };
        Self { session, before }
    }

    pub(super) fn receipt(&self, value: &Value) -> (Option<f64>, Vec<EngineEvent>) {
        let Some(total) = value.get("total_cost_usd").and_then(Value::as_f64) else {
            return (None, Vec::new());
        };
        if !total.is_finite() || total < 0.0 {
            return (
                None,
                vec![EngineEvent::ProtocolNote(
                    "Claude reported an invalid session cost".into(),
                )],
            );
        }
        let session = value.get("session_id").and_then(Value::as_str);
        let baseline = match (self.session.as_deref(), session) {
            (Some(expected), Some(actual)) if expected != actual => Some(0.0),
            _ => self.before,
        };
        let cost = baseline
            .filter(|before| total >= *before)
            .map(|before| total - before);
        let mut events = Vec::new();
        if cost.is_none() {
            events.push(EngineEvent::ProtocolNote(
                "Claude session cost baseline is missing or exceeds its receipt; this invocation's billed cost is unknown".into(),
            ));
        }
        if let Some(session) = session {
            let mut counters = Usage::default();
            // Only modelUsage contains cumulative token counters. They are
            // retained as reported, but next turn's Claude parser uses cost
            // only; per-invocation tokens still come from result.usage.
            if let Some(models) = value.get("modelUsage").and_then(Value::as_object) {
                for model in models.values() {
                    counters += Usage {
                        input_tokens: number(model, "inputTokens"),
                        output_tokens: number(model, "outputTokens"),
                        cache_read_tokens: number(model, "cacheReadInputTokens"),
                        cache_write_tokens: number(model, "cacheCreationInputTokens"),
                        reasoning_tokens: model.get("thinkingTokens").and_then(Value::as_u64),
                        ..Usage::default()
                    };
                }
            }
            counters.set_cost(Some(total));
            events.push(EngineEvent::SessionUsage {
                session: session.to_owned(),
                total: counters,
            });
        }
        (cost, events)
    }
}

fn number(value: &Value, key: &str) -> u64 {
    value.get(key).and_then(Value::as_u64).unwrap_or(0)
}

#[cfg(test)]
#[path = "tests/session_cost.rs"]
mod tests;
