//! Events out: one subscription posts to a URL, with bounded retries and a
//! dead letter for what never landed.

use std::io::Write as _;
use std::path::PathBuf;
use std::time::Duration;

use aether_net::{AddressPolicy, Vetted};

use aether_core::event::Event;
use futures::{Stream, StreamExt as _};
use tokio_util::sync::CancellationToken;

use super::signature;

/// First wait between attempts; doubled each time.
const FIRST_BACKOFF: Duration = Duration::from_millis(250);

/// Ceiling on that doubling, so a long outage does not park a delivery for
/// an unbounded time.
const MAX_BACKOFF: Duration = Duration::from_secs(30);

/// How long one POST may take before it counts as a failed attempt.
const POST_TIMEOUT: Duration = Duration::from_secs(30);

/// One outbound subscription.
#[derive(Clone)]
pub struct Subscription {
    pub url: String,
    /// Event kinds to post, by their `kind.type` string. Empty posts all.
    pub events: Vec<String>,
    /// Signs the delivery the way an inbound hook is signed. Required:
    /// without it the receiver cannot tell this fleet from anyone else who
    /// knows the URL, and every event body would leave the box unsigned.
    pub secret: String,
    /// Attempts per event, including the first.
    pub max_attempts: u32,
    /// Where an event that never landed is appended, as one JSON line.
    pub dead_letter: Option<PathBuf>,
}

/// Manual, so that one `{:?}` added later cannot put a signing secret in a log.
impl std::fmt::Debug for Subscription {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Subscription")
            .field("url", &self.url)
            .field("events", &self.events)
            .field("secret", &"<redacted>")
            .field("max_attempts", &self.max_attempts)
            .field("dead_letter", &self.dead_letter)
            .finish()
    }
}

/// What happened to one event on one subscription.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Delivery {
    Posted { attempts: u32 },
    DeadLettered { attempts: u32, reason: String },
}

/// Why delivery stopped. The caller decides what each means: from a daemon
/// subscription, a stream that ended is a dropped connection and this process
/// silently no longer delivering, which is not a clean stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stopped {
    /// Nothing was configured to deliver to.
    Nothing,
    /// The event stream ended.
    StreamEnded,
    /// `shutdown` fired.
    Shutdown,
}

/// Post every matching event until the stream ends or `shutdown` fires.
pub async fn deliver<S>(
    subscriptions: Vec<Subscription>,
    events: S,
    shutdown: CancellationToken,
) -> anyhow::Result<Stopped>
where
    S: Stream<Item = Event> + Send + 'static,
{
    if subscriptions.is_empty() {
        return Ok(Stopped::Nothing);
    }
    // Vetted once, at start: a receiver named in the config that cannot be
    // reached safely is a configuration error, and reporting it when the
    // first event happens to arrive would report it into a log nobody reads.
    let mut targets = Vec::with_capacity(subscriptions.len());
    for subscription in &subscriptions {
        targets.push(Target::open(subscription).await?);
    }
    futures::pin_mut!(events);
    loop {
        let event = tokio::select! {
            next = events.next() => match next {
                Some(event) => event,
                None => return Ok(Stopped::StreamEnded),
            },
            _ = shutdown.cancelled() => return Ok(Stopped::Shutdown),
        };
        for (subscription, target) in subscriptions.iter().zip(&targets) {
            if !wanted(subscription, &event) {
                continue;
            }
            // Serially, per event: a receiver that is slow must slow the
            // stream down rather than have deliveries pile up behind it.
            post_with_retries(target, subscription, &event, &shutdown).await;
        }
    }
}

fn wanted(subscription: &Subscription, event: &Event) -> bool {
    if subscription.events.is_empty() {
        return true;
    }
    match event_type(event) {
        Some(kind) => subscription.events.contains(&kind),
        // An event whose kind will not serialize cannot be matched against a
        // filter, and dropping it here would be the one place a delivery
        // disappears without a word. Sent instead: the same failure stops it
        // in `post_with_retries`, where it becomes a dead letter that says so.
        None => true,
    }
}

/// The event's `kind.type` tag, as the config names it, or `None` when the
/// kind will not serialize at all.
fn event_type(event: &Event) -> Option<String> {
    serde_json::to_value(&event.kind)
        .ok()?
        .get("type")
        .and_then(|t| t.as_str())
        .map(str::to_owned)
}

/// One receiver, checked and pinned. `aether serve` posts event bodies -
/// model content, task objectives, memory - so the address the config named
/// is the address the delivery reaches, over a connection that follows no
/// redirect: a receiver that answered 307 would otherwise forward the body
/// and its signature to whatever host it liked.
struct Target {
    client: reqwest::Client,
    vetted: Vetted,
}

impl Target {
    async fn open(subscription: &Subscription) -> anyhow::Result<Target> {
        // `Configured`, like the search endpoint: the operator wrote this URL
        // in their own file, and a receiver on the LAN is a normal setup.
        let vetted = aether_net::vet(&subscription.url, AddressPolicy::Configured)
            .await
            .map_err(|e| anyhow::anyhow!("[serve] subscription {}: {e}", subscription.url))?;
        // Aether terminates no TLS and signs the body, not the connection:
        // over plaintext off this machine, the event is readable in transit.
        if vetted.url.scheme() != "https" && !vetted.is_loopback() {
            anyhow::bail!(
                "[serve] subscription {} is plaintext http to an address off this machine; \
                 event bodies carry model content, so use https or a loopback receiver",
                subscription.url
            );
        }
        let client = aether_net::pinned_client(
            &vetted,
            POST_TIMEOUT,
            concat!("aether/", env!("CARGO_PKG_VERSION")),
        )?;
        Ok(Target { client, vetted })
    }
}

async fn post_with_retries(
    target: &Target,
    subscription: &Subscription,
    event: &Event,
    shutdown: &CancellationToken,
) -> Delivery {
    let body = match serde_json::to_vec(event) {
        Ok(body) => body,
        // An event that cannot be serialized is a bug here, not a transport
        // failure, and retrying it would never help.
        Err(e) => {
            let reason = format!("could not serialize the event: {e}");
            return dead_letter(subscription, event, 0, reason).await;
        }
    };
    let attempts = subscription.max_attempts.max(1);
    let mut backoff = FIRST_BACKOFF;
    let mut last = String::new();
    let mut made = 0;
    for attempt in 1..=attempts {
        made = attempt;
        match post_once(target, subscription, &body).await {
            Ok(()) => {
                tracing::debug!(url = %subscription.url, attempt, "posted an event");
                return Delivery::Posted { attempts: attempt };
            }
            Err(e) => last = e,
        }
        if attempt == attempts {
            break;
        }
        let mut interrupted = false;
        tokio::select! {
            _ = tokio::time::sleep(backoff) => {}
            _ = shutdown.cancelled() => interrupted = true,
        }
        if interrupted {
            // The record is what someone replays from, so it says what was
            // actually tried and why it stopped - not the budget it had.
            return dead_letter(
                subscription,
                event,
                made,
                format!("{last} (interrupted by shutdown before the next attempt)"),
            )
            .await;
        }
        backoff = (backoff * 2).min(MAX_BACKOFF);
    }
    dead_letter(subscription, event, made, last).await
}

async fn post_once(
    target: &Target,
    subscription: &Subscription,
    body: &[u8],
) -> Result<(), String> {
    // A fresh stamp per attempt: a retry is a new delivery to the receiver's
    // replay guard, not the same one arriving twice.
    let stamp = signature::Stamp::now();
    let signed = signature::sign(
        &subscription.secret,
        &signature::signed_payload(&stamp, body),
    )
    .ok_or_else(|| "the configured secret cannot sign a body".to_owned())?;
    let response = target
        .client
        .post(target.vetted.url.clone())
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header(signature::HEADER, signed)
        .header(signature::HEADER_TIMESTAMP, stamp.timestamp)
        .header(signature::HEADER_NONCE, &stamp.nonce)
        .body(body.to_vec())
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = response.status();
    if status.is_success() {
        return Ok(());
    }
    Err(format!("the receiver answered {status}"))
}

/// Record what never landed. A dead letter that cannot itself be written is
/// logged loudly rather than dropped quietly.
///
/// The write is blocking, so it goes to a blocking thread: delivery already
/// stalls the event stream behind it, and a slow disk must not also stall the
/// runtime worker every other task on this thread is sharing.
async fn dead_letter(
    subscription: &Subscription,
    event: &Event,
    attempts: u32,
    reason: String,
) -> Delivery {
    tracing::warn!(
        url = %subscription.url,
        attempts,
        reason = %reason,
        "an event was not delivered"
    );
    let Some(path) = &subscription.dead_letter else {
        return Delivery::DeadLettered { attempts, reason };
    };
    let line = serde_json::json!({
        "at": aether_core::time::now(),
        "url": subscription.url,
        "attempts": attempts,
        "reason": reason,
        "event": event,
    });
    // 0600, like the daemon's socket and the credential store: the file
    // holds whole event bodies, and wider would leave them world-readable.
    // `OpenOptions::mode` only sets that at creation, so it is enforced again
    // after opening - an existing file left wider by an earlier build or an
    // operator must not go on being read that way.
    let mut options = std::fs::OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let writing = path.clone();
    let written = tokio::task::spawn_blocking(move || {
        let mut file = options.open(&writing)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        writeln!(file, "{line}")
    })
    .await
    .unwrap_or_else(|e| Err(std::io::Error::other(format!("dead letter task: {e}"))));
    if let Err(e) = written {
        tracing::error!(
            path = %path.display(),
            error = %e,
            "could not write the dead letter; the event is only in this log"
        );
    }
    Delivery::DeadLettered { attempts, reason }
}
