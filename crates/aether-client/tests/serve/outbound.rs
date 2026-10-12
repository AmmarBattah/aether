use super::*;

/// One POST as the receiver saw it: the body, the signature on it, and the
/// stamp the signature covers.
type Posted = (String, Option<String>, Option<signature::Stamp>);

/// A receiver that fails the first `fail_first` requests, then succeeds.
struct Receiver {
    address: SocketAddr,
    received: Arc<Mutex<Vec<Posted>>>,
    shutdown: CancellationToken,
}

impl Receiver {
    async fn start(fail_first: usize, always_fail: bool) -> Receiver {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let address = listener.local_addr().expect("addr");
        let received: Arc<Mutex<Vec<Posted>>> = Arc::default();
        let shutdown = CancellationToken::new();
        let recording = received.clone();
        let token = shutdown.clone();
        tokio::spawn(async move {
            loop {
                let stream = tokio::select! {
                    accepted = listener.accept() => match accepted {
                        Ok((stream, _)) => stream,
                        Err(_) => break,
                    },
                    _ = token.cancelled() => break,
                };
                let recording = recording.clone();
                tokio::spawn(async move {
                    let service = hyper::service::service_fn(
                        move |request: hyper::Request<hyper::body::Incoming>| {
                            let recording = recording.clone();
                            async move {
                                let head = |name: &str| {
                                    request
                                        .headers()
                                        .get(name)
                                        .and_then(|v| v.to_str().ok())
                                        .map(str::to_owned)
                                };
                                let signature = head(signature::HEADER);
                                let stamp = head(signature::HEADER_TIMESTAMP)
                                    .zip(head(signature::HEADER_NONCE))
                                    .and_then(|(timestamp, nonce)| {
                                        signature::Stamp::parse(&timestamp, &nonce).ok()
                                    });
                                let body = request
                                    .into_body()
                                    .collect()
                                    .await
                                    .map(|b| String::from_utf8_lossy(&b.to_bytes()).into_owned())
                                    .unwrap_or_default();
                                let attempt = {
                                    let mut seen = recording.lock().expect("lock");
                                    seen.push((body, signature, stamp));
                                    seen.len()
                                };
                                let status = if always_fail || attempt <= fail_first {
                                    hyper::StatusCode::INTERNAL_SERVER_ERROR
                                } else {
                                    hyper::StatusCode::OK
                                };
                                Ok::<_, std::convert::Infallible>(
                                    hyper::Response::builder()
                                        .status(status)
                                        .body(Full::new(Bytes::new()))
                                        .expect("response"),
                                )
                            }
                        },
                    );
                    let _ = hyper::server::conn::http1::Builder::new()
                        .serve_connection(hyper_util::rt::TokioIo::new(stream), service)
                        .await;
                });
            }
        });
        Receiver {
            address,
            received,
            shutdown,
        }
    }

    fn attempts(&self) -> Vec<Posted> {
        self.received.lock().expect("lock").clone()
    }
}

fn an_event() -> Event {
    Event {
        id: aether_core::id::EventId::new(),
        seq: 7,
        at: aether_core::time::now(),
        actor: Actor::System,
        agent_id: None,
        parent_agent_id: None,
        run: None,
        task_id: None,
        causation: None,
        kind: EventKind::DaemonStarted {
            version: "test".into(),
            pid: 1,
        },
    }
}

async fn post_one(subscription: Subscription, event: Event) {
    assert_eq!(
        deliver_one(subscription, event).await.expect("deliver"),
        aether_client::serve::Stopped::StreamEnded,
        "the test's stream is finite; `deliver` says so rather than calling it a clean stop"
    );
}

async fn deliver_one(
    subscription: Subscription,
    event: Event,
) -> anyhow::Result<aether_client::serve::Stopped> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    tx.send(event).expect("send");
    drop(tx);
    aether_client::serve::deliver(
        vec![subscription],
        tokio_stream::wrappers::UnboundedReceiverStream::new(rx),
        CancellationToken::new(),
    )
    .await
}

fn to(url: String) -> Subscription {
    Subscription {
        url,
        events: Vec::new(),
        secret: SECRET.into(),
        max_attempts: 1,
        dead_letter: None,
    }
}

/// Event bodies carry model content, task objectives and memory. Off this
/// machine that needs TLS, and it must not be forwardable by a 307.
#[tokio::test]
async fn a_plaintext_receiver_off_this_machine_is_refused_before_anything_is_posted() {
    let refused = deliver_one(to("http://example.com/hook".into()), an_event())
        .await
        .expect_err("plaintext off this machine");
    assert!(refused.to_string().contains("https"), "{refused}");

    // The same host over TLS, and a plaintext loopback receiver, are fine.
    for allowed in ["https://example.com/hook", "http://127.0.0.1:9/hook"] {
        let opened = deliver_one(to(allowed.into()), an_event()).await;
        assert!(
            opened.is_ok(),
            "{allowed} must be allowed to start: {opened:?}"
        );
    }
}

/// A receiver that cannot be resolved or is not http at all is a
/// configuration error reported at start, not when the first event arrives.
#[tokio::test]
async fn a_receiver_that_cannot_be_reached_stops_the_start() {
    for bad in ["ftp://example.com/hook", "not a url"] {
        let refused = deliver_one(to(bad.into()), an_event())
            .await
            .expect_err(bad);
        assert!(refused.to_string().contains("subscription"), "{refused}");
    }
}

/// A receiver that answers 307 must not be able to forward an event body and
/// its signature to a host nothing vetted.
#[tokio::test]
async fn a_redirect_is_not_followed() {
    let elsewhere = Receiver::start(0, false).await;
    let redirector = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let address = redirector.local_addr().expect("addr");
    let to_elsewhere = format!("http://{}/hook", elsewhere.address);
    tokio::spawn(async move {
        while let Ok((stream, _)) = redirector.accept().await {
            let target = to_elsewhere.clone();
            tokio::spawn(async move {
                let service =
                    hyper::service::service_fn(move |_: hyper::Request<hyper::body::Incoming>| {
                        let target = target.clone();
                        async move {
                            Ok::<_, std::convert::Infallible>(
                                hyper::Response::builder()
                                    .status(hyper::StatusCode::TEMPORARY_REDIRECT)
                                    .header(hyper::header::LOCATION, target)
                                    .body(Full::new(Bytes::new()))
                                    .expect("response"),
                            )
                        }
                    });
                let _ = hyper::server::conn::http1::Builder::new()
                    .serve_connection(hyper_util::rt::TokioIo::new(stream), service)
                    .await;
            });
        }
    });

    let dir = tempfile::tempdir().expect("tempdir");
    let dead = dir.path().join("dead-letter.ndjson");
    post_one(
        Subscription {
            max_attempts: 1,
            dead_letter: Some(dead.clone()),
            ..to(format!("http://{address}/hook"))
        },
        an_event(),
    )
    .await;
    assert!(
        elsewhere.attempts().is_empty(),
        "the redirect target must never see the event"
    );
    let written = std::fs::read_to_string(&dead).expect("the dead letter exists");
    assert!(written.contains("307"), "the 307 is reported: {written}");
    elsewhere.shutdown.cancel();
}

/// The dead letter holds whole event bodies, so it is created 0600 like the
/// daemon's socket and the credential store, not at the umask's mercy.
#[cfg(unix)]
#[tokio::test]
async fn the_dead_letter_is_written_readable_only_by_this_user() {
    use std::os::unix::fs::PermissionsExt as _;

    let receiver = Receiver::start(0, true).await;
    let dir = tempfile::tempdir().expect("tempdir");
    let dead = dir.path().join("dead-letter.ndjson");
    post_one(
        Subscription {
            max_attempts: 1,
            dead_letter: Some(dead.clone()),
            ..to(format!("http://{}/hook", receiver.address))
        },
        an_event(),
    )
    .await;
    let mode = std::fs::metadata(&dead)
        .expect("the dead letter exists")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600, "mode {:o}", mode & 0o777);
    receiver.shutdown.cancel();
}

/// `OpenOptions::mode` only applies when the file is
/// created, so a dead letter left behind by an earlier build (or
/// pre-created by an operator) at a wider mode went on receiving whole event
/// bodies at that mode forever.
#[tokio::test]
async fn a_pre_existing_dead_letter_at_a_wider_mode_is_tightened() {
    use std::os::unix::fs::PermissionsExt as _;

    let receiver = Receiver::start(0, true).await;
    let dir = tempfile::tempdir().expect("tempdir");
    let dead = dir.path().join("dead-letter.ndjson");
    std::fs::write(&dead, "").expect("pre-create");
    std::fs::set_permissions(&dead, std::fs::Permissions::from_mode(0o644)).expect("chmod");

    post_one(
        Subscription {
            max_attempts: 1,
            dead_letter: Some(dead.clone()),
            ..to(format!("http://{}/hook", receiver.address))
        },
        an_event(),
    )
    .await;
    let mode = std::fs::metadata(&dead)
        .expect("the dead letter exists")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600, "mode {:o}", mode & 0o777);
    receiver.shutdown.cancel();
}

#[tokio::test]
async fn an_event_is_posted_signed_and_retried_until_it_lands() {
    let receiver = Receiver::start(2, false).await;
    post_one(
        Subscription {
            url: format!("http://{}/hook", receiver.address),
            events: Vec::new(),
            secret: SECRET.into(),
            max_attempts: 5,
            dead_letter: None,
        },
        an_event(),
    )
    .await;
    let attempts = receiver.attempts();
    assert_eq!(attempts.len(), 3, "two failures then the delivery");
    let (body, presented, stamp) = attempts.last().expect("an attempt");
    assert!(body.contains("daemon.started"), "{body}");
    let presented = presented.as_deref().expect("the post is signed");
    let stamp = stamp.as_ref().expect("the post is stamped");
    assert!(
        signature::verify(
            SECRET,
            &signature::signed_payload(stamp, body.as_bytes()),
            presented
        ),
        "the receiver can verify what it was sent"
    );
    // Each attempt carries its own nonce, so a receiver's replay guard sees
    // three deliveries rather than one arriving three times.
    let nonces: std::collections::HashSet<_> = attempts
        .iter()
        .filter_map(|(_, _, stamp)| stamp.as_ref().map(|s| s.nonce.clone()))
        .collect();
    assert_eq!(nonces.len(), 3, "a retry is stamped afresh");
    receiver.shutdown.cancel();
}

#[tokio::test]
async fn an_event_that_never_lands_is_dead_lettered() {
    let receiver = Receiver::start(0, true).await;
    let dir = tempfile::tempdir().expect("tempdir");
    let dead = dir.path().join("dead-letter.ndjson");
    post_one(
        Subscription {
            url: format!("http://{}/hook", receiver.address),
            events: Vec::new(),
            secret: SECRET.into(),
            max_attempts: 2,
            dead_letter: Some(dead.clone()),
        },
        an_event(),
    )
    .await;
    assert_eq!(receiver.attempts().len(), 2, "both attempts were made");
    let written = std::fs::read_to_string(&dead).expect("the dead letter exists");
    let record: serde_json::Value =
        serde_json::from_str(written.trim()).expect("one JSON line per event");
    assert_eq!(record["attempts"], 2);
    assert_eq!(record["event"]["kind"]["type"], "daemon.started");
    assert!(
        record["reason"]
            .as_str()
            .unwrap_or_default()
            .contains("500"),
        "the reason says what the receiver answered: {record}"
    );
    receiver.shutdown.cancel();
}

#[tokio::test]
async fn a_subscription_posts_only_the_kinds_it_asked_for() {
    let receiver = Receiver::start(0, false).await;
    post_one(
        Subscription {
            url: format!("http://{}/hook", receiver.address),
            events: vec!["agent.failed".into()],
            secret: SECRET.into(),
            max_attempts: 1,
            dead_letter: None,
        },
        an_event(),
    )
    .await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(
        receiver.attempts().is_empty(),
        "daemon.started is not agent.failed"
    );
    receiver.shutdown.cancel();
}
