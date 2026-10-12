//! What stops a captured webhook being delivered twice.
//!
//! A signature over a body alone is valid forever. Signing a timestamp bounds
//! how long a captured request is useful; remembering the nonce closes the
//! remaining window, so inside it a delivery lands exactly once.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Mutex;
use std::time::Duration;

use super::signature::Stamp;

/// How far a delivery's timestamp may sit from this clock, in either
/// direction - senders run their own clocks and some of them are behind.
/// The nonce, not this window, is what makes a replay impossible; the window
/// is what makes remembering nonces bounded.
pub const REPLAY_WINDOW: Duration = Duration::from_secs(300);

/// Nonces held at once, per hook path. Only a caller that already signs
/// correctly reaches the guard, so this bounds a misbehaving partner rather
/// than an attacker; per path, so one noisy partner can only saturate its
/// own hook. Each entry is a `path\nnonce` key held twice (`HashSet` and
/// `VecDeque`), roughly 2 * `MAX_NONCE_CHARS` bytes plus overhead - tens of
/// megabytes at this cap, not the ~200 KB "~2 KiB per thousand" once implied
/// here.
const MAX_REMEMBERED: usize = 100_000;

/// Why a stamped delivery was not admitted. Each is a refusal the caller can
/// act on, so each says what to do differently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    Stale { by: i64 },
    Ahead { by: i64 },
    Replayed,
    Saturated,
    Unavailable,
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let window = REPLAY_WINDOW.as_secs();
        match self {
            Refused::Stale { by } => write!(
                f,
                "the delivery is {by}s old and the window is {window}s; sign and send it fresh"
            ),
            Refused::Ahead { by } => write!(
                f,
                "the delivery is timestamped {by}s ahead of this clock and the window is \
                 {window}s; check the sender's clock"
            ),
            Refused::Replayed => write!(f, "this delivery was already accepted"),
            Refused::Saturated => write!(
                f,
                "too many deliveries are being remembered at once; retry with a fresh stamp"
            ),
            Refused::Unavailable => write!(f, "the replay guard is unavailable"),
        }
    }
}

/// The nonces seen inside the window, one [`Seen`] per hook path.
///
/// Bounded twice over: entries older than the window are dropped on every
/// admission, and each path's set refuses rather than grows past
/// [`MAX_REMEMBERED`] - forgetting a nonce early would be forgetting that a
/// delivery already happened, which is the one thing this must not do.
#[derive(Debug)]
pub struct ReplayGuard {
    window: Duration,
    by_path: Mutex<HashMap<String, Seen>>,
}

#[derive(Debug, Default)]
struct Seen {
    /// Arrival time and nonce, oldest arrival first, so pruning is a walk
    /// from the front. Ordered by arrival rather than by the stamp: a
    /// sender's clock may sit anywhere inside the window, so stamps do not
    /// arrive in order and pruning on them would stop early.
    order: VecDeque<(i64, String)>,
    nonces: HashSet<String>,
}

impl Default for ReplayGuard {
    fn default() -> Self {
        ReplayGuard::new(REPLAY_WINDOW)
    }
}

impl ReplayGuard {
    pub fn new(window: Duration) -> ReplayGuard {
        ReplayGuard {
            window,
            by_path: Mutex::new(HashMap::new()),
        }
    }

    /// Admit this delivery once. `now` is unix seconds, passed in so a test
    /// can move the clock without sleeping.
    pub fn admit(&self, path: &str, stamp: &Stamp, now: i64) -> Result<(), Refused> {
        let window = self.window.as_secs();
        if stamp.timestamp <= now {
            let age = now.abs_diff(stamp.timestamp);
            if age > window {
                return Err(Refused::Stale {
                    by: age.min(i64::MAX as u64) as i64,
                });
            }
        } else {
            let ahead = stamp.timestamp.abs_diff(now);
            if ahead > window {
                return Err(Refused::Ahead {
                    by: ahead.min(i64::MAX as u64) as i64,
                });
            }
        }
        let mut by_path = match self.by_path.lock() {
            Ok(by_path) => by_path,
            // A poisoned lock means a panic mid-admission; the set may have
            // lost an entry, and admitting on that basis would be admitting a
            // possible replay.
            Err(_) => return Err(Refused::Unavailable),
        };
        // Each hook holds its own budget, so a partner that saturates one
        // path's set - by signing correctly and fast - cannot refuse
        // deliveries on any other path.
        let seen = by_path.entry(path.to_owned()).or_default();
        // A delivery admitted at arrival `a` carries a stamp within `window`
        // of it, so a replay of it can be admitted no later than `a + 2 *
        // window`. Held exactly that long: shorter would forget that a
        // delivery happened while it could still be replayed.
        let keep = window.saturating_mul(2);
        while let Some((arrived, _)) = seen.order.front() {
            if now < *arrived || now.abs_diff(*arrived) <= keep {
                break;
            }
            let Some((_, old)) = seen.order.pop_front() else {
                break;
            };
            seen.nonces.remove(&old);
        }
        if seen.nonces.len() >= MAX_REMEMBERED {
            return Err(Refused::Saturated);
        }
        if !seen.nonces.insert(stamp.nonce.clone()) {
            return Err(Refused::Replayed);
        }
        seen.order.push_back((now, stamp.nonce.clone()));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    fn stamp(nonce: &str, at: i64) -> Stamp {
        Stamp {
            timestamp: at,
            nonce: nonce.into(),
        }
    }

    const NOW: i64 = 1_700_000_000;

    #[test]
    fn a_delivery_is_admitted_once_and_never_again() {
        let guard = ReplayGuard::default();
        let stamp = stamp("n1", NOW);
        assert_eq!(guard.admit("/hooks/ci", &stamp, NOW), Ok(()));
        assert_eq!(
            guard.admit("/hooks/ci", &stamp, NOW + 1),
            Err(Refused::Replayed)
        );
    }

    #[test]
    fn the_same_nonce_on_another_hook_is_another_delivery() {
        let guard = ReplayGuard::default();
        let stamp = stamp("n1", NOW);
        assert_eq!(guard.admit("/hooks/ci", &stamp, NOW), Ok(()));
        assert_eq!(guard.admit("/hooks/deploy", &stamp, NOW), Ok(()));
    }

    #[test]
    fn a_stamp_outside_the_window_is_refused_in_either_direction() {
        let guard = ReplayGuard::default();
        let window = REPLAY_WINDOW.as_secs() as i64;
        assert!(matches!(
            guard.admit("/h", &stamp("old", NOW - window - 1), NOW),
            Err(Refused::Stale { .. })
        ));
        assert!(matches!(
            guard.admit("/h", &stamp("new", NOW + window + 1), NOW),
            Err(Refused::Ahead { .. })
        ));
        // The edges themselves are inside it: a sender's clock is allowed to
        // be exactly as far off as the window says.
        assert_eq!(
            guard.admit("/h", &stamp("edge-old", NOW - window), NOW),
            Ok(())
        );
        assert_eq!(
            guard.admit("/h", &stamp("edge-new", NOW + window), NOW),
            Ok(())
        );
    }

    #[test]
    fn extreme_timestamps_are_refused_without_overflowing() {
        let guard = ReplayGuard::default();
        assert_eq!(
            guard.admit("/h", &stamp("old", i64::MIN), NOW),
            Err(Refused::Stale { by: i64::MAX })
        );
        assert_eq!(
            guard.admit("/h", &stamp("new", i64::MAX), NOW),
            Err(Refused::Ahead { by: i64::MAX - NOW })
        );
    }

    #[test]
    fn pruning_handles_arrival_times_across_the_i64_boundary() {
        let guard = ReplayGuard::new(Duration::from_secs(10));
        assert_eq!(guard.admit("/h", &stamp("old", i64::MIN), i64::MIN), Ok(()));
        assert_eq!(guard.admit("/h", &stamp("new", i64::MAX), i64::MAX), Ok(()));
        assert_eq!(nonces_remembered(&guard, "/h"), 1);
    }

    /// The set does not grow without bound: once a nonce is too old to be
    /// admitted anyway, remembering it buys nothing.
    #[test]
    fn nonces_are_forgotten_only_once_the_window_refuses_them_anyway() {
        let guard = ReplayGuard::new(Duration::from_secs(10));
        assert_eq!(guard.admit("/h", &stamp("n1", NOW), NOW), Ok(()));
        assert_eq!(nonces_remembered(&guard, "/h"), 1);
        // Still remembered one window on: a stamp from `NOW + 10` could have
        // been signed at `NOW` and is only now too old.
        assert_eq!(guard.admit("/h", &stamp("n2", NOW + 15), NOW + 15), Ok(()));
        assert_eq!(nonces_remembered(&guard, "/h"), 2);
        assert_eq!(guard.admit("/h", &stamp("n3", NOW + 21), NOW + 21), Ok(()));
        assert_eq!(
            nonces_remembered(&guard, "/h"),
            2,
            "the first nonce aged out once no stamp could still carry it"
        );
        // And it is unusable by then regardless.
        assert!(matches!(
            guard.admit("/h", &stamp("n1", NOW), NOW + 21),
            Err(Refused::Stale { .. })
        ));
    }

    fn nonces_remembered(guard: &ReplayGuard, path: &str) -> usize {
        guard
            .by_path
            .lock()
            .expect("lock")
            .get(path)
            .map_or(0, |seen| seen.nonces.len())
    }

    /// The budget used to be one pool shared by every
    /// hook, so a partner that signs correctly for its own hook could
    /// saturate the guard and get every other hook refused with it. Each
    /// path now owns its own budget.
    #[test]
    fn saturating_one_hook_does_not_refuse_another() {
        let guard = ReplayGuard::default();
        for n in 0..MAX_REMEMBERED {
            assert_eq!(
                guard.admit("/hooks/noisy", &stamp(&format!("n{n}"), NOW), NOW),
                Ok(())
            );
        }
        assert_eq!(
            guard.admit("/hooks/noisy", &stamp("one-more", NOW), NOW),
            Err(Refused::Saturated)
        );
        // A different path's budget is untouched by the noisy one.
        assert_eq!(guard.admit("/hooks/quiet", &stamp("n0", NOW), NOW), Ok(()));
    }
}
