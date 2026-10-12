//! In-process fan-out of committed events and transient streaming deltas.
//!
//! The bus is a fast path, not the source of truth. A subscriber that needs
//! durability replays from the store using sequence numbers.

use std::sync::Arc;

use aether_core::event::Event;
use aether_core::id::{AgentId, InvocationId};
use tokio::sync::broadcast;

/// Transient streaming signal. Never persisted.
#[derive(Debug, Clone, PartialEq)]
pub enum DeltaKind {
    TurnStarted { turn: u32 },
    Text { text: String },
    Thinking { text: String },
    ToolInput { name: String, partial_json: String },
    TurnFinished,
}

#[derive(Debug, Clone)]
pub enum BusMessage {
    Event(Arc<Event>),
    Delta {
        agent: AgentId,
        invocation: InvocationId,
        delta: DeltaKind,
    },
}

#[derive(Debug, Clone)]
pub struct EventBus {
    tx: broadcast::Sender<BusMessage>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(8192);
        Self { tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<BusMessage> {
        self.tx.subscribe()
    }

    pub fn publish_event(&self, event: Event) {
        // No receivers is not an error: the store already has the event.
        let _ = self.tx.send(BusMessage::Event(Arc::new(event)));
    }

    pub fn publish_delta(&self, agent: AgentId, invocation: InvocationId, delta: DeltaKind) {
        let _ = self.tx.send(BusMessage::Delta {
            agent,
            invocation,
            delta,
        });
    }
}
