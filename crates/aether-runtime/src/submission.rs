//! Client-supplied idempotency keys on the commands that create work.
//!
//! A client that loses the answer to a submission cannot tell a request that
//! never landed from one that landed and whose reply was dropped. Retrying
//! blind creates the work twice; not retrying loses it. A key on the command
//! settles it: the key, a fingerprint of the request and what it created are
//! committed in the same transaction as the work, so a retry under the same
//! key is answered with the first submission's identity and creates nothing.
//!
//! Two things make that hold. The key is checked and recorded inside one
//! [`Runtime::hold`], so two retries in flight at once cannot both find
//! nothing; and `submissions.key` is a primary key written with a plain
//! INSERT, so a duplicate that somehow reached the store fails its whole
//! commit rather than landing beside the first.

use aether_core::idempotency::{
    FINGERPRINT_SCHEME, RecordedSubmission, Submission, SubmissionOutcome,
};

use crate::{CommitHold, Result, Runtime, RuntimeError};

impl Runtime {
    /// What `submission` already created, if this store has seen its key.
    /// `None` means the caller must do the work and commit the record with it.
    ///
    /// `hold` is the proof the commit lock is held: the check and the record
    /// it decides on must be one critical section, or two retries in flight
    /// at once both find nothing and both do the work.
    pub(crate) fn replayed_submission(
        &self,
        _hold: &CommitHold<'_>,
        submission: Option<&Submission>,
    ) -> Result<Option<SubmissionOutcome>> {
        self.recorded_submission(submission)
    }

    /// The commit lock, taken now, but only for a command that carries a key.
    ///
    /// A keyed command reads its key and writes its record inside one critical
    /// section, so it holds the lock across the reads in between. An unkeyed
    /// one has nothing to check and takes the lock where it always did: the
    /// guarantee is paid for by the callers that ask for it, not by every
    /// writer in the daemon.
    pub(crate) fn hold_for(&self, submission: Option<&Submission>) -> Option<CommitHold<'_>> {
        submission.map(|_| self.hold())
    }

    /// The same lookup without the hold, for a command with expensive work to
    /// do before it can take one. Sound as a fast path and useless as a
    /// guarantee: a record is durable and never rewritten, so finding one is
    /// final - but a submission committing right now is not visible yet, which
    /// is what the held check exists for.
    pub(crate) fn recorded_submission(
        &self,
        submission: Option<&Submission>,
    ) -> Result<Option<SubmissionOutcome>> {
        let Some(submission) = submission else {
            return Ok(None);
        };
        let Some(record) = self.store.submission(&submission.key)? else {
            return Ok(None);
        };
        // Before the fingerprint compare, not after: two digests taken over
        // different field sets are not comparable, and reporting a mismatch
        // between them as "a different request" would blame the client for
        // this daemon's upgrade.
        if record.scheme != FINGERPRINT_SCHEME {
            return Err(stale_scheme(&record));
        }
        if record.fingerprint != submission.fingerprint {
            return Err(conflict(&record));
        }
        Ok(Some(record.outcome))
    }
}

/// A record whose fingerprint was taken over a different field set - written
/// by a daemon before or after this one's fingerprint scheme. Neither replayed
/// (the requests may differ) nor reported as a different request (they may
/// not), so it says what actually happened and what to do.
fn stale_scheme(record: &RecordedSubmission) -> RuntimeError {
    RuntimeError::Invalid(format!(
        "idempotency_key `{}` was recorded on {} by a daemon using request-fingerprint scheme {}, \
         and this daemon uses {}; it cannot tell a retry from a new request under that key - \
         submit under a new one",
        record.key,
        record.at.to_rfc3339(),
        record.scheme,
        FINGERPRINT_SCHEME
    ))
}

/// What a recorded submission created is gone from the store. Not `Internal`:
/// nothing purges agents, tasks or schedules today, so this can only follow an
/// operator or a future sweep removing the work while its record stands, and
/// the client's next move is a new key rather than a retry.
pub(crate) fn created_work_gone(what: &str, id: &dyn std::fmt::Display) -> RuntimeError {
    RuntimeError::NotFound(format!(
        "{what} {id} was created under this idempotency_key and is no longer in the store; the \
         retry cannot be answered - submit again under a new key"
    ))
}

/// A key reused for different work. Actionable rather than merely refused: it
/// names the key and what it was spent on, which is what a client needs to
/// tell a genuine collision from its own retry of a request it edited.
fn conflict(record: &RecordedSubmission) -> RuntimeError {
    RuntimeError::Invalid(format!(
        "idempotency_key `{}` already created a {} on {}; a different request needs a new key",
        record.key,
        record.outcome.created(),
        record.at.to_rfc3339()
    ))
}

/// A recorded outcome that is not the shape the replaying command expects.
///
/// Unreachable through the socket - the fingerprint covers the command tag, so
/// a key reused across two commands conflicts before it gets here - and
/// reported rather than unwrapped, because the alternative is answering one
/// command with another's identity.
pub(crate) fn wrong_shape(outcome: &SubmissionOutcome, wanted: &str) -> RuntimeError {
    RuntimeError::Internal(format!(
        "idempotency record created a {}, not a {wanted}",
        outcome.created()
    ))
}
