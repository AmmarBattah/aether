//! Read-only projections of the daemon's retention decisions.
use crate::{
    id::{ArtifactId, RunId},
    time::Timestamp,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RunRetentionDecision {
    Eligible,
    /// The first blocker, in the same order as the deletion path.
    Protected {
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RunRetentionPreview {
    pub run: RunId,
    pub decision: RunRetentionDecision,
    pub events: u64,
    pub rows: u64,
    /// Serialized payload bytes, excluding indexes and SQLite page overhead.
    /// Zero for protected runs. This is reusable database space, not disk shrinkage.
    pub history_payload_bytes: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PendingArtifactCleanup {
    /// Durable unlink retry records; some may still share a live payload.
    pub pending_unlinks: Vec<ArtifactId>,
    /// Staged files awaiting publication/recovery, possibly still in flight.
    /// No publication leases are acquired or released by preview.
    pub staged_publications: Vec<ArtifactId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RetentionPreview {
    pub at: Timestamp,
    pub history_before: Timestamp,
    pub runs: Vec<RunRetentionPreview>,
    pub eligible_artifacts: Vec<ArtifactId>,
    /// Projected logical payload bytes, counting shared digests once.
    pub artifact_payload_bytes: u64,
    pub history_payload_bytes: u64,
    pub pending_cleanup: PendingArtifactCleanup,
}
