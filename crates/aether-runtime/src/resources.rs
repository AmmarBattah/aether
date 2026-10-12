//! Cross-agent resource leases: two agents never hold a mutating tool call
//! on overlapping resource keys at once.
//!
//! A turn takes every key its mutating calls will touch in one all-or-nothing
//! acquisition and releases them when the batch is done. Because a turn never
//! holds one acquisition while waiting for another, there is no hold-and-wait
//! and so no deadlock; the guard frees the keys even if the turn panics.
//!
//! A wait for a key is bounded by the scheduler's wait deadline: past it the
//! waiter gives up, the agent is told who held the key, and its turn ends
//! rather than parking out of sight of the wait-timeout machinery.
//!
//! Waiters queue by priority, then by how long they have waited (VISION §3.6
//! limit 6), and a turn arriving fresh joins the tail of that queue instead of
//! barging past it, so a wait for a hot key is bounded by the work already
//! claiming it.
//!
//! The table is live state, not history. It dies with the daemon, so recovery
//! re-classifies a turn that was waiting on a key rather than trusting the
//! Blocked status it left behind (see `recovery.rs`).

use std::sync::Arc;
use std::time::Duration;

use aether_core::id::AgentId;
use aether_core::lease::ResourceHolding;
use aether_core::task::Priority;
use aether_core::time::now;
use aether_core::tool::ResourceKey;
use parking_lot::Mutex;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

/// Who has the claim that is in the way, and on which key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceConflict {
    /// The agent holding the key, or queued ahead of us for it.
    pub holder: AgentId,
    /// The key we asked for that the holder has claimed.
    pub key: ResourceKey,
}

impl ResourceConflict {
    /// One line for a Blocked status and its event.
    pub fn describe(&self) -> String {
        format!("waiting for {} claimed by {}", self.key.0, self.holder)
    }
}

/// Where a turn stands in the queue: higher priority first, then whoever
/// asked earlier.
type Rank = (std::cmp::Reverse<Priority>, u64);

struct Waiter {
    ticket: u64,
    agent: AgentId,
    priority: Priority,
    keys: Vec<ResourceKey>,
}

/// One live acquisition. A quarantine hold is the same claim taken for a
/// different reason: no guard owns it, and only clearing the inspection that
/// caused it lets go (§15).
struct Held {
    id: u64,
    holding: ResourceHolding,
}

#[derive(Default)]
struct Table {
    held: Vec<Held>,
    queue: Vec<Waiter>,
    next_id: u64,
}

impl Table {
    fn id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }
}

#[derive(Debug, Default)]
pub struct ResourceLeases {
    table: Mutex<Table>,
    released: Notify,
}

impl std::fmt::Debug for Table {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Table")
            .field("held", &self.held.len())
            .field("queued", &self.queue.len())
            .finish()
    }
}

impl ResourceLeases {
    pub fn new() -> Self {
        Self::default()
    }

    /// Join the wait queue for `keys`. The returned ticket is the caller's
    /// place in it and leaves the queue when dropped.
    pub fn enter(
        self: &Arc<Self>,
        agent: AgentId,
        priority: Priority,
        keys: &[ResourceKey],
    ) -> Waiting {
        let mut table = self.table.lock();
        let ticket = table.id();
        table.queue.push(Waiter {
            ticket,
            agent,
            priority,
            keys: keys.to_vec(),
        });
        drop(table);
        Waiting {
            leases: self.clone(),
            agent,
            priority,
            keys: keys.to_vec(),
            ticket,
        }
    }

    /// What is held right now, for status and the live graph.
    pub fn holdings(&self) -> Vec<ResourceHolding> {
        self.table
            .lock()
            .held
            .iter()
            .map(|h| h.holding.clone())
            .collect()
    }

    /// VISION §3.9/§15: hold `keys` for an agent parked for inspection, with
    /// no guard and no expiry. A tool process that outlived the daemon may
    /// still be writing to them, so nobody else takes them until a person has
    /// looked. Replaces any hold this agent already had, so recovering twice
    /// quarantines once.
    pub fn quarantine(&self, agent: AgentId, keys: Vec<ResourceKey>) {
        if keys.is_empty() {
            return;
        }
        let mut table = self.table.lock();
        // Re-quarantining with a narrower set frees the keys it dropped, so
        // anyone blocked on one has to be woken.
        let mut freed = false;
        table.held.retain(|h| {
            let mine = h.holding.quarantined && h.holding.agent == agent;
            freed |= mine && h.holding.keys.iter().any(|k| !keys.contains(k));
            !mine
        });
        let id = table.id();
        table.held.push(Held {
            id,
            holding: ResourceHolding {
                agent,
                keys,
                since: now(),
                quarantined: true,
            },
        });
        drop(table);
        if freed {
            self.released.notify_waiters();
        }
    }

    /// The keys an agent's quarantine hold covers, without letting go of
    /// them: the release is recorded in the log before the hold comes off.
    pub fn quarantined_keys(&self, agent: AgentId) -> Vec<ResourceKey> {
        self.table
            .lock()
            .held
            .iter()
            .filter(|h| h.holding.quarantined && h.holding.agent == agent)
            .flat_map(|h| h.holding.keys.iter().cloned())
            .collect()
    }

    /// Let go of an agent's quarantine hold, returning the keys freed. Called
    /// when a person clears the inspection or the agent ends.
    pub fn release_quarantine(&self, agent: AgentId) -> Vec<ResourceKey> {
        let mut table = self.table.lock();
        let mut freed = Vec::new();
        table.held.retain(|h| {
            if h.holding.quarantined && h.holding.agent == agent {
                freed.extend(h.holding.keys.iter().cloned());
                return false;
            }
            true
        });
        drop(table);
        if !freed.is_empty() {
            self.released.notify_waiters();
        }
        freed
    }

    /// Take every key or none, for the waiter holding `ticket`.
    fn take(
        &self,
        agent: AgentId,
        rank: Rank,
        keys: &[ResourceKey],
    ) -> Result<u64, ResourceConflict> {
        let mut table = self.table.lock();
        for key in keys {
            if let Some(holder) = table
                .held
                .iter()
                .filter(|h| h.holding.agent != agent)
                .find_map(|h| {
                    h.holding
                        .conflict_with(key)
                        .map(|k| (h.holding.agent, k.clone()))
                })
            {
                return Err(ResourceConflict {
                    holder: holder.0,
                    key: holder.1,
                });
            }
        }
        // Fairness: someone ranked ahead of us wants an overlapping key.
        if let Some(ahead) = table
            .queue
            .iter()
            .filter(|w| w.agent != agent && (std::cmp::Reverse(w.priority), w.ticket) < rank)
            .find_map(|w| {
                w.keys
                    .iter()
                    .find(|k| keys.iter().any(|mine| k.conflicts_with(mine)))
                    .map(|k| (w.agent, k.clone()))
            })
        {
            return Err(ResourceConflict {
                holder: ahead.0,
                key: ahead.1,
            });
        }
        let id = table.id();
        table.held.push(Held {
            id,
            holding: ResourceHolding {
                agent,
                keys: keys.to_vec(),
                since: now(),
                quarantined: false,
            },
        });
        Ok(id)
    }

    fn leave_queue(&self, ticket: u64) {
        self.table.lock().queue.retain(|w| w.ticket != ticket);
        // A departing waiter may have been the one holding others back.
        self.released.notify_waiters();
    }

    fn release(&self, id: u64) {
        self.table.lock().held.retain(|h| h.id != id);
        self.released.notify_waiters();
    }
}

/// A turn's place in the wait queue. Leaves the queue when dropped, so a
/// cancelled or panicking turn never holds the line.
#[derive(Debug)]
pub struct Waiting {
    leases: Arc<ResourceLeases>,
    agent: AgentId,
    priority: Priority,
    keys: Vec<ResourceKey>,
    ticket: u64,
}

impl Waiting {
    fn rank(&self) -> Rank {
        (std::cmp::Reverse(self.priority), self.ticket)
    }

    /// Take the keys now, or say who is in the way.
    pub fn try_take(&self) -> Result<ResourceGuard, ResourceConflict> {
        let id = self.leases.take(self.agent, self.rank(), &self.keys)?;
        Ok(ResourceGuard {
            leases: self.leases.clone(),
            agent: self.agent,
            keys: self.keys.clone(),
            id,
        })
    }

    /// Wait until the keys are free, giving up after `deadline`.
    pub async fn wait(self, cancel: &CancellationToken, deadline: Duration) -> WaitOutcome {
        let expiry = tokio::time::sleep(deadline);
        tokio::pin!(expiry);
        loop {
            // Register before testing, so a release between the two is not lost.
            let wakeup = self.leases.released.notified();
            tokio::pin!(wakeup);
            wakeup.as_mut().enable();
            if let Ok(guard) = self.try_take() {
                return WaitOutcome::Acquired(guard);
            }
            tokio::select! {
                _ = wakeup => {}
                _ = &mut expiry => return WaitOutcome::TimedOut,
                _ = cancel.cancelled() => return WaitOutcome::Cancelled,
            }
        }
    }
}

/// How a bounded wait for the keys ended.
#[derive(Debug)]
pub enum WaitOutcome {
    Acquired(ResourceGuard),
    /// The turn was cancelled or interrupted.
    Cancelled,
    /// The holder kept the keys past the wait deadline.
    TimedOut,
}

impl Drop for Waiting {
    fn drop(&mut self) {
        self.leases.leave_queue(self.ticket);
    }
}

/// Frees this acquisition's keys when the batch is done, or if the turn
/// unwinds. Keyed to the acquisition, never to the agent.
#[derive(Debug)]
pub struct ResourceGuard {
    leases: Arc<ResourceLeases>,
    agent: AgentId,
    keys: Vec<ResourceKey>,
    id: u64,
}

impl ResourceGuard {
    pub fn agent(&self) -> AgentId {
        self.agent
    }

    /// The keys this acquisition took.
    pub fn keys(&self) -> &[ResourceKey] {
        &self.keys
    }
}

impl Drop for ResourceGuard {
    fn drop(&mut self) {
        self.leases.release(self.id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(paths: &[&str]) -> Vec<ResourceKey> {
        paths.iter().map(ResourceKey::file).collect()
    }

    fn take(
        leases: &Arc<ResourceLeases>,
        agent: AgentId,
        keys: &[ResourceKey],
    ) -> Result<ResourceGuard, ResourceConflict> {
        leases.enter(agent, Priority::Normal, keys).try_take()
    }

    #[test]
    fn overlapping_keys_name_their_holder() {
        let leases = Arc::new(ResourceLeases::new());
        let first = AgentId::new();
        let second = AgentId::new();
        let held = take(&leases, first, &keys(&["/r/a.rs"])).expect("first holds");
        let conflict = take(&leases, second, &keys(&["/r/a.rs"])).expect_err("second must wait");
        assert_eq!(conflict.holder, first);
        assert!(conflict.describe().contains("/r/a.rs"));
        assert!(
            take(&leases, second, &keys(&["/r/b.rs"])).is_ok(),
            "disjoint keys never wait"
        );
        drop(held);
        assert!(take(&leases, second, &keys(&["/r/a.rs"])).is_ok());
    }

    #[test]
    fn a_repository_contains_the_files_under_it() {
        let leases = Arc::new(ResourceLeases::new());
        let first = AgentId::new();
        let _held = take(&leases, first, &[ResourceKey::repo("/r")]).expect("held");
        let conflict = take(&leases, AgentId::new(), &keys(&["/r/src/a.rs"]))
            .expect_err("a file under a held repository waits");
        assert_eq!(conflict.holder, first);
    }

    #[test]
    fn a_directory_contains_the_files_under_it() {
        let leases = Arc::new(ResourceLeases::new());
        let first = AgentId::new();
        let _held = take(&leases, first, &[ResourceKey::dir("/r/src")]).expect("held");
        let conflict = take(&leases, AgentId::new(), &keys(&["/r/src/a.rs"]))
            .expect_err("a file under a held directory waits");
        assert_eq!(conflict.holder, first);
    }

    #[test]
    fn acquisition_is_all_or_nothing() {
        let leases = Arc::new(ResourceLeases::new());
        let _held = take(&leases, AgentId::new(), &keys(&["/r/a.rs"])).expect("held");
        let waiter = AgentId::new();
        assert!(take(&leases, waiter, &keys(&["/r/b.rs", "/r/a.rs"])).is_err());
        assert!(
            leases.holdings().iter().all(|h| h.agent != waiter),
            "a refused acquisition holds nothing"
        );
    }

    #[test]
    fn release_frees_one_acquisition_not_every_key_the_agent_has() {
        let leases = Arc::new(ResourceLeases::new());
        let agent = AgentId::new();
        let first = take(&leases, agent, &keys(&["/r/a.rs"])).expect("first");
        let _second = take(&leases, agent, &keys(&["/r/b.rs"])).expect("second");
        drop(first);
        let still: Vec<ResourceKey> = leases.holdings().into_iter().flat_map(|h| h.keys).collect();
        assert_eq!(still, keys(&["/r/b.rs"]), "{still:?}");
    }

    #[test]
    fn a_queued_waiter_is_not_barged_by_a_newcomer() {
        let leases = Arc::new(ResourceLeases::new());
        let held = take(&leases, AgentId::new(), &keys(&["/r/a.rs"])).expect("held");
        let queued = AgentId::new();
        let waiting = leases.enter(queued, Priority::Normal, &keys(&["/r/a.rs"]));
        assert!(waiting.try_take().is_err(), "the key is held");
        drop(held);
        let newcomer = take(&leases, AgentId::new(), &keys(&["/r/a.rs"]))
            .expect_err("a newcomer waits behind the queue");
        assert_eq!(newcomer.holder, queued);
        assert!(waiting.try_take().is_ok(), "the waiter's turn came");
    }

    #[test]
    fn priority_overtakes_a_lower_priority_waiter() {
        let leases = Arc::new(ResourceLeases::new());
        let held = take(&leases, AgentId::new(), &keys(&["/r/a.rs"])).expect("held");
        let low = leases.enter(AgentId::new(), Priority::Low, &keys(&["/r/a.rs"]));
        drop(held);
        let urgent = leases.enter(AgentId::new(), Priority::High, &keys(&["/r/a.rs"]));
        assert!(urgent.try_take().is_ok(), "priority goes first");
        assert!(
            low.try_take().is_err(),
            "the low-priority waiter still waits"
        );
    }

    #[tokio::test]
    async fn a_waiter_is_woken_by_the_release() {
        let leases = Arc::new(ResourceLeases::new());
        let held = take(&leases, AgentId::new(), &keys(&["/r/a.rs"])).expect("held");
        let waiting = leases.clone();
        let waiter = AgentId::new();
        let task = tokio::spawn(async move {
            let outcome = waiting
                .enter(waiter, Priority::Normal, &keys(&["/r/a.rs"]))
                .wait(&CancellationToken::new(), Duration::from_secs(30))
                .await;
            matches!(outcome, WaitOutcome::Acquired(_))
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        drop(held);
        assert!(task.await.expect("join"));
    }

    #[tokio::test]
    async fn a_wait_for_a_key_gives_up_at_its_deadline() {
        let leases = Arc::new(ResourceLeases::new());
        let _held = take(&leases, AgentId::new(), &keys(&["/r/a.rs"])).expect("held");
        let outcome = leases
            .enter(AgentId::new(), Priority::Normal, &keys(&["/r/a.rs"]))
            .wait(&CancellationToken::new(), Duration::from_millis(20))
            .await;
        assert!(matches!(outcome, WaitOutcome::TimedOut));
        assert!(
            leases.table.lock().queue.is_empty(),
            "a waiter that gave up must not hold the line"
        );
    }

    #[tokio::test]
    async fn cancelling_the_turn_ends_the_wait_and_leaves_the_queue() {
        let leases = Arc::new(ResourceLeases::new());
        let _held = take(&leases, AgentId::new(), &keys(&["/r/a.rs"])).expect("held");
        let cancel = CancellationToken::new();
        cancel.cancel();
        let outcome = leases
            .enter(AgentId::new(), Priority::Normal, &keys(&["/r/a.rs"]))
            .wait(&cancel, Duration::from_secs(30))
            .await;
        assert!(matches!(outcome, WaitOutcome::Cancelled));
        assert!(
            leases.table.lock().queue.is_empty(),
            "a cancelled waiter must not hold the line"
        );
    }

    /// A re-quarantine with an empty set kept the hold: `unreleased_keys`
    /// returning nothing means "found no keys", never "let these go". A
    /// narrower set does free what it drops, so waiters have to be woken.
    #[tokio::test]
    async fn requarantining_keeps_a_hold_it_was_given_no_keys_for_and_wakes_on_a_narrower_one() {
        let leases = Arc::new(ResourceLeases::new());
        let agent = AgentId::new();
        leases.quarantine(agent, keys(&["/r/a.rs", "/r/b.rs"]));
        leases.quarantine(agent, Vec::new());
        assert_eq!(
            leases.quarantined_keys(agent),
            keys(&["/r/a.rs", "/r/b.rs"]),
            "an empty set is not a release"
        );

        let waiter = {
            let leases = leases.clone();
            tokio::spawn(async move {
                leases
                    .enter(AgentId::new(), Priority::Normal, &keys(&["/r/b.rs"]))
                    .wait(&CancellationToken::new(), Duration::from_secs(5))
                    .await
            })
        };
        tokio::time::sleep(Duration::from_millis(50)).await;
        leases.quarantine(agent, keys(&["/r/a.rs"]));
        let outcome = tokio::time::timeout(Duration::from_secs(2), waiter)
            .await
            .expect("the dropped key woke the waiter")
            .expect("join");
        assert!(matches!(outcome, WaitOutcome::Acquired(_)), "{outcome:?}");
    }
}
