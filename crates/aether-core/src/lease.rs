//! Leases: ownership of in-flight work with an expiry, so a dead process
//! never leaves an agent permanently "running".

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::id::AgentId;
use crate::time::Timestamp;
use crate::tool::ResourceKey;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Lease {
    pub agent_id: AgentId,
    /// Opaque identity of the process or worker holding the lease.
    pub holder: String,
    pub turn: u32,
    pub granted_at: Timestamp,
    pub expires_at: Timestamp,
}

impl Lease {
    pub fn new(
        agent_id: AgentId,
        holder: impl Into<String>,
        turn: u32,
        at: Timestamp,
        ttl: Duration,
    ) -> Self {
        let ttl = chrono::Duration::from_std(ttl).unwrap_or(chrono::Duration::seconds(60));
        Self {
            agent_id,
            holder: holder.into(),
            turn,
            granted_at: at,
            expires_at: at + ttl,
        }
    }

    pub fn is_expired(&self, now: Timestamp) -> bool {
        now >= self.expires_at
    }

    pub fn renew(&mut self, now: Timestamp, ttl: Duration) {
        let ttl = chrono::Duration::from_std(ttl).unwrap_or(chrono::Duration::seconds(60));
        self.expires_at = now + ttl;
    }
}

/// Resource keys one agent's mutating tool call holds for the length of that
/// call. Two agents never hold conflicting keys at once (VISION §3.6 limit 5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ResourceHolding {
    pub agent: AgentId,
    pub keys: Vec<ResourceKey>,
    pub since: Timestamp,
    /// Held for an agent parked for inspection rather than for a running
    /// call: nothing is using the keys, and only a person clears them (§15).
    #[serde(default)]
    pub quarantined: bool,
}

impl ResourceHolding {
    /// The first key of this holding that `key` may not touch.
    pub fn conflict_with(&self, key: &ResourceKey) -> Option<&ResourceKey> {
        self.keys.iter().find(|held| held.conflicts_with(key))
    }
}
