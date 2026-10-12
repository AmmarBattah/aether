use super::{SqliteStore, submission_time};
use crate::Result;
use aether_core::{
    event::{Actor, EventDraft, EventKind},
    time::{Timestamp, now},
};
use rusqlite::{Connection, OptionalExtension};
use std::sync::atomic::Ordering;

/// What one retention sweep over `submissions` dropped.
///
/// `evicted` is the number the *size* pass had to drop, and it is never
/// routine: the age pass runs first, so everything left is inside the
/// retention window and every eviction shortens the guarantee for the client
/// that recorded it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct SubmissionSweep {
    pub(super) aged: usize,
    pub(super) evicted: usize,
    /// `at` of the oldest evicted record, as stored.
    pub(super) oldest_evicted: Option<String>,
}

impl SubmissionSweep {
    /// Say what the sweep dropped, and return the ledger entry an eviction
    /// needs. The age pass is routine and goes to debug; the size pass means
    /// the retry guarantee just got shorter than advertised, so it is a warn
    /// and an event. `None` when the size pass evicted nothing.
    pub(super) fn report(&self, max_rows: usize) -> Option<EventDraft> {
        if self.aged > 0 {
            tracing::debug!(
                aged = self.aged,
                "idempotency records swept past the retention window"
            );
        }
        if self.evicted == 0 {
            return None;
        }
        tracing::warn!(
            evicted = self.evicted,
            oldest_at = self.oldest_evicted.as_deref().unwrap_or("unknown"),
            max_submissions = max_rows,
            "idempotency records evicted while still inside the retention window: a retry of one \
             of them will do its work a second time"
        );
        Some(EventDraft::new(
            Actor::System,
            EventKind::Custom {
                name: "submissions.evicted".into(),
                payload: serde_json::json!({
                    "n": self.evicted,
                    "oldest_at": self.oldest_evicted,
                    "max_submissions": max_rows,
                }),
            },
        ))
    }
}

impl SqliteStore {
    /// The overflow eviction statement, shared with
    /// `the_size_pass_reads_only_the_overflow` so the query-plan test guards
    /// what production actually runs, not a copy of it.
    ///
    /// Bounded by the overflow, not by the table: `key IN (... LIMIT n)`
    /// walks n entries of the `(at, key)` index. The `NOT IN (... LIMIT
    /// cap)` spelling this replaced materialized every surviving key into
    /// an ephemeral index and scanned the table against it.
    pub(super) const OVERFLOW_DELETE_SQL: &'static str = "DELETE FROM submissions WHERE key IN
               (SELECT key FROM submissions ORDER BY at ASC, key ASC LIMIT ?1)";

    /// Age-then-size retention for `submissions`, in whichever transaction the
    /// caller is already in.
    ///
    /// The age pass runs on every recorded submission: it is one seek into the
    /// `at` index plus whatever it deletes. Finding the overflow means counting
    /// the table, so `size_pass` says whether this write is the one that pays
    /// for that. Everything the size pass evicts was inside the age window by
    /// construction, since the age pass has already taken everything outside
    /// it, which is why the sweep reports it rather than dropping it quietly.
    pub(super) fn prune_submissions_in(
        tx: &Connection,
        before: Timestamp,
        max_rows: usize,
        size_pass: bool,
    ) -> Result<SubmissionSweep> {
        let aged = tx.execute(
            "DELETE FROM submissions WHERE at < ?1",
            [submission_time(before)],
        )?;
        let mut sweep = SubmissionSweep {
            aged,
            evicted: 0,
            oldest_evicted: None,
        };
        if !size_pass {
            return Ok(sweep);
        }
        let kept: i64 = tx.query_row("SELECT COUNT(*) FROM submissions", [], |r| r.get(0))?;
        let overflow = kept - max_rows as i64;
        if overflow <= 0 {
            return Ok(sweep);
        }
        // The oldest surviving row is the oldest one about to be evicted:
        // read it before the delete, so the report can name how far into the
        // window the eviction reached.
        sweep.oldest_evicted = tx
            .query_row(
                "SELECT at FROM submissions ORDER BY at ASC, key ASC LIMIT 1",
                [],
                |r| r.get::<_, String>(0),
            )
            .optional()?;
        sweep.evicted = tx.execute(Self::OVERFLOW_DELETE_SQL, [overflow])?;
        Ok(sweep)
    }

    /// Drive one sweep with explicit bounds. Production sweeps ride the
    /// commit that recorded a submission; this exists so a test can reach the
    /// same code with a small window and a small cap.
    #[cfg(test)]
    pub(super) fn prune_submissions(
        &self,
        before: Timestamp,
        max_rows: usize,
    ) -> Result<SubmissionSweep> {
        let conn = self.conn.lock();
        let tx = conn.unchecked_transaction()?;
        let sweep = Self::prune_submissions_in(&tx, before, max_rows, true)?;
        tx.commit()?;
        Ok(sweep)
    }

    /// Sweep retention for `submissions` and say what the size pass had to
    /// evict, in the same transaction as the record that triggered it.
    pub(super) fn sweep_submissions(&self, tx: &Connection) -> Result<SubmissionSweep> {
        let due = self
            .submissions_since_size_pass
            .fetch_add(1, Ordering::Relaxed)
            + 1
            >= aether_core::idempotency::SUBMISSION_SIZE_PASS_EVERY;
        if due {
            self.submissions_since_size_pass.store(0, Ordering::Relaxed);
        }
        // Wall clock, not the record's own `at`: retention is about how long
        // ago a submission was made, and a record stamped in the future by a
        // skewed clock must not sweep away good ones.
        Self::prune_submissions_in(
            tx,
            now() - chrono::Duration::days(aether_core::idempotency::SUBMISSION_RETENTION_DAYS),
            self.max_submissions,
            due,
        )
    }
}
