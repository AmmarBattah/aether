//! Artifacts: large outputs kept outside the model context.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::event::Actor;
use crate::id::{AgentId, ArtifactId, TaskId};
use crate::time::Timestamp;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ArtifactKind {
    File,
    CommandOutput,
    Diff,
    Commit,
    Result,
    Other { label: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Artifact {
    pub id: ArtifactId,
    pub kind: ArtifactKind,
    pub name: String,
    /// External location when the artifact lives outside the store (a repo file, a commit).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    /// Short description suitable for a prompt.
    pub summary: String,
    pub size_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    /// SHA-256 of the stored bytes, when stored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    pub created_by: Actor,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<AgentId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<TaskId>,
    pub created_at: Timestamp,
    /// When the bytes were dropped under the retention policy. The record
    /// stays - history is never deleted - but the payload is gone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evicted_at: Option<Timestamp>,
}

impl Artifact {
    /// Whether the bytes are still readable.
    pub fn has_bytes(&self) -> bool {
        self.evicted_at.is_none() && self.path.is_some()
    }
}

/// One stored payload and every record pointing at it.
struct Blob<'a> {
    newest: Timestamp,
    size_bytes: u64,
    records: Vec<&'a Artifact>,
}

/// How long artifact bytes are kept. Records are never removed; only the
/// payload is, and only once both limits allow it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RetentionPolicy {
    /// Bytes older than this are dropped. `None` keeps them forever.
    pub max_age_days: Option<u32>,
    /// Total bytes kept across all artifacts; the oldest go first.
    /// `None` means no size limit.
    pub max_total_bytes: Option<u64>,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            max_age_days: Some(30),
            max_total_bytes: Some(2 << 30),
        }
    }
}

impl RetentionPolicy {
    pub fn keep_everything() -> Self {
        Self {
            max_age_days: None,
            max_total_bytes: None,
        }
    }

    /// The artifacts whose bytes must go, oldest first. `live` must be the
    /// artifacts that still have bytes.
    ///
    /// Records that share a digest share one blob, so they are weighed as
    /// one: aged by the *newest* reference - a payload a task produced today
    /// is not old because an identical one was produced last month - counted
    /// once toward the size limit, and evicted together, because dropping
    /// the blob drops it for all of them.
    pub fn evictable<'a>(&self, live: &'a [Artifact], now: Timestamp) -> Vec<&'a Artifact> {
        let mut blobs: Vec<Blob<'a>> = Vec::new();
        let mut at: BTreeMap<String, usize> = BTreeMap::new();
        for artifact in live {
            let key = artifact
                .digest
                .clone()
                .unwrap_or_else(|| artifact.id.to_string());
            match at.get(&key) {
                Some(i) => {
                    let blob: &mut Blob<'a> = &mut blobs[*i];
                    blob.newest = blob.newest.max(artifact.created_at);
                    blob.records.push(artifact);
                }
                None => {
                    at.insert(key, blobs.len());
                    blobs.push(Blob {
                        newest: artifact.created_at,
                        size_bytes: artifact.size_bytes,
                        records: vec![artifact],
                    });
                }
            }
        }
        blobs.sort_by_key(|b| b.newest);
        let mut kept_bytes: u64 = blobs.iter().map(|b| b.size_bytes).sum();
        let mut doomed: Vec<&Artifact> = Vec::new();
        for blob in &blobs {
            // An overflowing future deadline is beyond every representable timestamp.
            let too_old = self.max_age_days.is_some_and(|days| {
                blob.newest
                    .checked_add_signed(chrono::Duration::days(i64::from(days)))
                    .is_some_and(|expires| expires < now)
            });
            let too_big = self.max_total_bytes.is_some_and(|limit| kept_bytes > limit);
            if too_old || too_big {
                kept_bytes = kept_bytes.saturating_sub(blob.size_bytes);
                doomed.extend(blob.records.iter().copied());
            }
        }
        doomed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::now;

    fn artifact(age_days: i64, size: u64) -> Artifact {
        Artifact {
            id: ArtifactId::new(),
            kind: ArtifactKind::CommandOutput,
            name: "out".into(),
            path: Some(PathBuf::from("/tmp/x")),
            summary: String::new(),
            size_bytes: size,
            content_type: None,
            digest: None,
            created_by: Actor::System,
            agent_id: None,
            task_id: None,
            created_at: now() - chrono::Duration::days(age_days),
            evicted_at: None,
        }
    }

    #[test]
    fn age_and_size_both_evict_oldest_first() {
        let live = vec![artifact(60, 10), artifact(40, 10), artifact(1, 10)];
        let by_age = RetentionPolicy {
            max_age_days: Some(30),
            max_total_bytes: None,
        };
        let doomed = by_age.evictable(&live, now());
        assert_eq!(doomed.len(), 2);
        assert_eq!(doomed[0].id, live[0].id, "oldest goes first");

        // 15 bytes of room for 30 bytes of artifacts: the two oldest go.
        let by_size = RetentionPolicy {
            max_age_days: None,
            max_total_bytes: Some(15),
        };
        let doomed = by_size.evictable(&live, now());
        assert_eq!(doomed.len(), 2);
        assert_eq!(doomed[1].id, live[1].id);
    }

    #[test]
    fn shared_bytes_age_by_their_newest_reference() {
        // Two tasks produced the same payload a month apart. The blob is one
        // blob, and a handle created today must not lose its bytes because
        // an identical one is old.
        let old = artifact(60, 10);
        let mut fresh = artifact(1, 10);
        fresh.digest = Some("shared".into());
        let mut old = old;
        old.digest = Some("shared".into());
        let live = vec![old, fresh];
        let by_age = RetentionPolicy {
            max_age_days: Some(30),
            max_total_bytes: None,
        };
        assert!(
            by_age.evictable(&live, now()).is_empty(),
            "the newest reference keeps the shared bytes alive"
        );

        // Once every reference is old, the blob goes - and both records with
        // it, because there is only one payload to drop.
        let mut a = artifact(60, 10);
        a.digest = Some("shared".into());
        let mut b = artifact(45, 10);
        b.digest = Some("shared".into());
        let both = [a, b];
        let doomed = by_age.evictable(&both, now());
        assert_eq!(doomed.len(), 2, "both records lose the one blob together");
    }

    #[test]
    fn shared_bytes_count_once_against_the_size_limit() {
        let mut a = artifact(3, 10);
        a.digest = Some("shared".into());
        let mut b = artifact(2, 10);
        b.digest = Some("shared".into());
        let other = artifact(1, 10);
        // 20 bytes of room: one 10-byte blob shared twice plus one more is
        // 20 bytes stored, not 30, so nothing has to go.
        let by_size = RetentionPolicy {
            max_age_days: None,
            max_total_bytes: Some(20),
        };
        let live = [a, b, other];
        assert!(by_size.evictable(&live, now()).is_empty());
    }

    #[test]
    fn an_unrepresentable_retention_deadline_keeps_bytes() {
        let live = [artifact(1, 10)];
        let policy = RetentionPolicy {
            max_age_days: Some(u32::MAX),
            max_total_bytes: None,
        };
        assert!(policy.evictable(&live, now()).is_empty());
        let limited = RetentionPolicy {
            max_total_bytes: Some(0),
            ..policy
        };
        assert_eq!(limited.evictable(&live, now()).len(), 1);
    }

    #[test]
    fn keeping_everything_evicts_nothing() {
        let live = vec![artifact(9999, 1 << 40)];
        assert!(
            RetentionPolicy::keep_everything()
                .evictable(&live, now())
                .is_empty()
        );
    }

    #[test]
    fn an_evicted_artifact_has_no_bytes() {
        let mut a = artifact(1, 10);
        assert!(a.has_bytes());
        a.evicted_at = Some(now());
        assert!(!a.has_bytes());
    }
}
