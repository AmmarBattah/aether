//! Serve MCP over streamable HTTP: one endpoint, one JSON-RPC message per
//! POST, the answer as a JSON body.
//!
//! A token is mandatory. The endpoint drives the whole fleet - it can create
//! agents that run shell commands on this machine - so an unauthenticated one
//! would be a way in for anything that can reach the port. A request without a
//! matching bearer token is refused before it is parsed, and everything a peer
//! can hold without one (connections, header reads, guesses) is bounded.
//!
//! The token also decides *how much*: the operator's reaches the fleet, a
//! running turn's reaches that turn's own tools and nothing else. Fleet
//! authority must not rest on a credential that travels in a child process's
//! argv.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use aether_core::id::AgentId;
use aether_protocol::auth::constant_time_eq;
use aether_protocol::{Command, Reply};
use http_body_util::{BodyExt as _, Full, Limited};
use hyper::body::{Bytes, Incoming};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::{TokioIo, TokioTimer};
use tokio::net::TcpListener;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

use super::{Answer, CommandSink, Surface, TurnSurface};

/// Largest request body accepted. A JSON-RPC call is small; anything larger
/// is a mistake or an attack, and reading it would be the vulnerability.
const MAX_BODY_BYTES: usize = 1 << 20;

/// Shortest operator token accepted, the same rule every tokened transport
/// holds to.
pub use aether_protocol::auth::MIN_TOKEN_CHARS;

/// Connections served at once. A peer that opens more waits in the kernel's
/// backlog instead of costing a task and a pair of file descriptors each.
const MAX_CONNECTIONS: usize = 64;

/// How long a peer may take to send its request head.
const HEADER_READ_TIMEOUT: Duration = Duration::from_secs(5);

/// How long any one connection may stay open. Every call here is a small
/// request and a small answer, so a connection that outlives this is holding
/// a slot rather than using it; a client that needs another opens one.
const CONNECTION_MAX_LIFETIME: Duration = Duration::from_secs(60);

/// Failed authentication doubles the wait from here, up to the ceiling.
const AUTH_BACKOFF_BASE: Duration = Duration::from_millis(250);
const AUTH_BACKOFF_MAX: Duration = Duration::from_secs(30);
/// Peers tracked for backoff. Bounded so the map is not itself a way to
/// spend the daemon's memory.
const MAX_TRACKED_PEERS: usize = 1024;

/// Where the endpoint listens and what it accepts.
#[derive(Debug, Clone)]
pub struct HttpConfig {
    pub addr: SocketAddr,
    /// The operator's long-lived bearer token.
    pub token: String,
    /// Allow a bind outside loopback. Off by default: this is a fleet
    /// control plane behind one static string, and Aether terminates no TLS.
    pub allow_remote: bool,
}

/// Serve until `shutdown` fires. Returns the address actually bound, through
/// `bound`, so a caller that asked for port 0 can learn it.
pub async fn serve(
    sink: Arc<dyn CommandSink>,
    config: HttpConfig,
    shutdown: CancellationToken,
    bound: impl FnOnce(SocketAddr),
) -> anyhow::Result<()> {
    let token = config.token.trim().to_owned();
    if token.chars().count() < MIN_TOKEN_CHARS {
        anyhow::bail!(
            "the MCP HTTP endpoint needs a token of at least {MIN_TOKEN_CHARS} characters; \
             this one has {}",
            token.chars().count()
        );
    }
    let listener = TcpListener::bind(config.addr).await?;
    let local = listener.local_addr()?;
    if !local.ip().is_loopback() {
        if !config.allow_remote {
            anyhow::bail!(
                "{local} is not a loopback address, and this endpoint can create agents that \
                 run commands on this machine; pass --allow-remote to mean it"
            );
        }
        tracing::warn!(
            address = %local,
            "mcp server: bound outside loopback over plain HTTP - Aether terminates no TLS, so \
             the bearer token crosses the network in cleartext unless something in front of it \
             does"
        );
    }
    tracing::info!(address = %local, "mcp server listening");
    bound(local);

    let auth = Arc::new(Auth {
        token,
        backoff: Backoff::default(),
    });
    let slots = Arc::new(Semaphore::new(MAX_CONNECTIONS));
    loop {
        // Taken before accepting, so a flood queues in the kernel's backlog
        // rather than in tasks and file descriptors here.
        let Ok(slot) = slots.clone().acquire_owned().await else {
            break;
        };
        let (stream, peer) = tokio::select! {
            accepted = listener.accept() => accepted?,
            _ = shutdown.cancelled() => break,
        };
        let sink = sink.clone();
        let auth = auth.clone();
        let connection = shutdown.clone();
        tokio::spawn(async move {
            let _slot = slot;
            let service = service_fn(move |req| {
                let sink = sink.clone();
                let auth = auth.clone();
                async move {
                    Ok::<_, std::convert::Infallible>(answer(&*sink, &auth, peer.ip(), req).await)
                }
            });
            let served = http1::Builder::new()
                // header_read_timeout needs a timer to arm against.
                .timer(TokioTimer::new())
                .header_read_timeout(HEADER_READ_TIMEOUT)
                .serve_connection(TokioIo::new(stream), service);
            tokio::select! {
                result = served => {
                    if let Err(e) = result {
                        tracing::debug!(%peer, error = %e, "mcp server: connection ended");
                    }
                }
                _ = tokio::time::sleep(CONNECTION_MAX_LIFETIME) => {
                    tracing::debug!(%peer, "mcp server: connection outlived its budget");
                }
                _ = connection.cancelled() => {}
            }
        });
    }
    Ok(())
}

/// What a request must present, and what guessing costs.
struct Auth {
    token: String,
    backoff: Backoff,
}

/// One line naming who authenticated, for the log. The surface itself
/// carries the authority.
fn describe(surface: &Surface) -> String {
    match surface {
        Surface::Fleet => "operator".to_owned(),
        Surface::Turn(turn) => format!("turn of {}", turn.agent),
    }
}

async fn answer(
    sink: &dyn CommandSink,
    auth: &Auth,
    peer: IpAddr,
    request: Request<Incoming>,
) -> Response<Full<Bytes>> {
    if auth.backoff.blocked(peer) {
        return status(
            StatusCode::TOO_MANY_REQUESTS,
            "too many failed authentications from here; wait and try again",
        );
    }
    // No legitimate caller here is a browser, so every origin is refused
    // rather than allowlisted: it closes DNS rebinding without a setting.
    if request.headers().contains_key(hyper::header::ORIGIN) {
        return status(
            StatusCode::FORBIDDEN,
            "this endpoint does not serve browser origins",
        );
    }
    // Before the method check, so an unauthenticated probe cannot tell this
    // port from a closed one by the status it gets back.
    let Some(surface) = authorize(sink, &auth.token, &request).await else {
        auth.backoff.failed(peer);
        return status(StatusCode::UNAUTHORIZED, "a valid bearer token is required");
    };
    auth.backoff.cleared(peer);
    tracing::debug!(%peer, caller = %describe(&surface), "mcp server: authorized");
    if request.method() != Method::POST {
        return status(StatusCode::METHOD_NOT_ALLOWED, "only POST is served");
    }
    // Limited stops reading at the cap rather than buffering first, so an
    // oversized body is refused instead of held in memory.
    let body = match Limited::new(request.into_body(), MAX_BODY_BYTES)
        .collect()
        .await
        .map(|collected| collected.to_bytes())
    {
        Ok(bytes) => bytes,
        Err(_) => {
            return status(
                StatusCode::PAYLOAD_TOO_LARGE,
                "the request body was too large, or ended early",
            );
        }
    };
    let message: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => return status(StatusCode::BAD_REQUEST, &format!("malformed JSON: {e}")),
    };
    match super::handle(sink, &surface, &message).await {
        // A notification is accepted and answered with no content, as the
        // streamable HTTP transport requires.
        Answer::None => Response::builder()
            .status(StatusCode::ACCEPTED)
            .body(Full::default())
            .unwrap_or_else(|_| Response::new(Full::default())),
        Answer::Denied(why) => status(StatusCode::FORBIDDEN, &why),
        Answer::Reply(response) => {
            let bytes = serde_json::to_vec(&response).unwrap_or_default();
            Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "application/json")
                .body(Full::from(bytes))
                .unwrap_or_else(|_| Response::new(Full::default()))
        }
    }
}

/// What the bearer token authorizes, or `None`. The operator's token is
/// compared here; a per-turn one is `<agent-id>.<token>` and only the daemon
/// can say whether that turn is still running, so it is asked - and its
/// answer, that turn's own tool list, is the surface the token reaches.
async fn authorize(
    sink: &dyn CommandSink,
    token: &str,
    request: &Request<Incoming>,
) -> Option<Surface> {
    let presented = bearer(request)?;
    if constant_time_eq(presented, token) {
        return Some(Surface::Fleet);
    }
    let (agent, turn_token) = presented.split_once('.')?;
    let agent: AgentId = agent.parse().ok()?;
    match sink
        .call(Command::EngineTools {
            agent,
            token: turn_token.to_owned(),
        })
        .await
    {
        Ok(Reply::EngineTools { tools }) => Some(Surface::Turn(TurnSurface {
            agent,
            token: turn_token.to_owned(),
            tools,
        })),
        _ => None,
    }
}

/// The bearer credential as presented. The scheme is case-insensitive
/// (RFC 7235); the value is not trimmed, so a token that differs only in
/// whitespace is a different token.
fn bearer(request: &Request<Incoming>) -> Option<&str> {
    request
        .headers()
        .get(hyper::header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .split_once(' ')
        .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
        .map(|(_, presented)| presented)
}

/// How long a peer that failed to authenticate must wait. A token holder
/// gets in on its first try and is never delayed; guessing costs
/// exponentially more per attempt.
#[derive(Debug, Default)]
struct Backoff {
    peers: Mutex<HashMap<IpAddr, Penalty>>,
}

#[derive(Debug, Clone, Copy)]
struct Penalty {
    failures: u32,
    until: Instant,
}

impl Backoff {
    fn blocked(&self, peer: IpAddr) -> bool {
        // A poisoned lock means a panic while recording a failure; refuse
        // rather than let the refusal be the thing that opens the door.
        let Ok(peers) = self.peers.lock() else {
            return true;
        };
        peers
            .get(&peer)
            .is_some_and(|penalty| Instant::now() < penalty.until)
    }

    fn failed(&self, peer: IpAddr) {
        let Ok(mut peers) = self.peers.lock() else {
            return;
        };
        let failures = peers.get(&peer).map_or(1, |p| p.failures.saturating_add(1));
        let wait = AUTH_BACKOFF_BASE
            .saturating_mul(1u32 << failures.min(8))
            .min(AUTH_BACKOFF_MAX);
        let penalty = Penalty {
            failures,
            until: Instant::now() + wait,
        };
        if peers.len() >= MAX_TRACKED_PEERS && !peers.contains_key(&peer) {
            evict_one(&mut peers);
        }
        peers.insert(peer, penalty);
    }

    /// A peer that authenticated owes nothing.
    fn cleared(&self, peer: IpAddr) {
        if let Ok(mut peers) = self.peers.lock() {
            peers.remove(&peer);
        }
    }
}

/// Drop the peer whose penalty expires first, so the map stays bounded and
/// the peer still being slowed keeps its penalty.
fn evict_one(peers: &mut HashMap<IpAddr, Penalty>) {
    let Some(soonest) = peers
        .iter()
        .min_by_key(|(_, penalty)| penalty.until)
        .map(|(addr, _)| *addr)
    else {
        return;
    };
    peers.remove(&soonest);
}

fn status(code: StatusCode, message: &str) -> Response<Full<Bytes>> {
    Response::builder()
        .status(code)
        .header("content-type", "text/plain")
        .body(Full::from(message.to_owned()))
        .unwrap_or_else(|_| Response::new(Full::default()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_guess_is_slowed_and_a_success_clears_it() {
        let backoff = Backoff::default();
        let peer: IpAddr = "10.0.0.1".parse().expect("addr");
        assert!(!backoff.blocked(peer), "a peer starts unpenalized");
        backoff.failed(peer);
        assert!(backoff.blocked(peer), "a guess costs a wait");
        backoff.cleared(peer);
        assert!(!backoff.blocked(peer), "authenticating clears it");
    }

    #[test]
    fn the_wait_grows_and_stops_at_the_ceiling() {
        let backoff = Backoff::default();
        let peer: IpAddr = "10.0.0.2".parse().expect("addr");
        for _ in 0..40 {
            backoff.failed(peer);
        }
        let peers = backoff.peers.lock().expect("lock");
        let penalty = peers.get(&peer).expect("tracked");
        assert!(penalty.failures >= 40);
        assert!(
            penalty.until <= Instant::now() + AUTH_BACKOFF_MAX,
            "the wait is capped, so a peer is never locked out forever"
        );
    }

    #[test]
    fn the_peer_table_is_bounded() {
        let backoff = Backoff::default();
        for i in 0..(MAX_TRACKED_PEERS + 50) {
            let peer: IpAddr = format!("10.{}.{}.{}", i / 65536, (i / 256) % 256, i % 256)
                .parse()
                .expect("addr");
            backoff.failed(peer);
        }
        assert!(
            backoff.peers.lock().expect("lock").len() <= MAX_TRACKED_PEERS,
            "tracking must not itself be a way to spend memory"
        );
    }
}
