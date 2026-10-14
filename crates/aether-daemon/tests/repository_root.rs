#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

//! `create_agent.repository_root` over a real socket.

mod support;

use aether_protocol::PROTOCOL_VERSION;
use serde_json::{Value, json};
use support::{RawConn, TestDaemon};

fn git(root: &std::path::Path, args: &[&str]) {
    let out = std::process::Command::new("git")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("git runs");
    assert!(out.status.success(), "git {args:?}: {out:?}");
}

fn checkout(origin: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    git(dir.path(), &["init", "-q"]);
    git(dir.path(), &["remote", "add", "origin", origin]);
    dir
}

fn create(root: &std::path::Path, key: &str) -> Value {
    json!({"cmd":"create_agent", "role":"checkout-test", "start":false,
        "model":{"provider":"claude-code","model":"haiku"},
        "repository_root":root, "idempotency_key":key})
}

/// The feature is advertised, the daemon derives the id from the checkout,
/// and one key cannot be replayed for a different checkout.
#[tokio::test]
async fn a_named_checkout_sets_the_repository_over_the_socket() {
    let daemon = TestDaemon::start().await;
    let mut conn = RawConn::connect(&daemon.socket).await.unwrap();
    let welcome = conn.hello(PROTOCOL_VERSION).await.unwrap();
    assert!(
        welcome["features"]
            .as_array()
            .unwrap()
            .contains(&json!("repository_root")),
        "{welcome}"
    );
    let widget = checkout("git@github.com:acme/widget.git");
    let created = conn.call(create(widget.path(), "checkout-1")).await;
    let agent = &created["result"]["Ok"]["data"];
    assert_eq!(
        agent["scope"]["repository"], "github.com/acme/widget",
        "{created}"
    );
    assert!(
        agent.get("repository_root").is_none() && agent["scope"].get("repository_root").is_none(),
        "the checkout is input only: {agent}"
    );

    let replayed = conn.call(create(widget.path(), "checkout-1")).await;
    assert_eq!(replayed["result"]["Ok"]["data"]["id"], agent["id"]);
    let gadget = checkout("git@github.com:acme/gadget.git");
    let reused = conn.call(create(gadget.path(), "checkout-1")).await;
    let message = reused["result"]["Err"]["message"]
        .as_str()
        .unwrap_or_else(|| panic!("a reused key is refused: {reused}"));
    assert!(message.contains("needs a new key"), "{message}");

    let both = conn
        .call(
            json!({"cmd":"create_agent", "role":"checkout-test", "start":false,
            "model":{"provider":"claude-code","model":"haiku"},
            "repository_root":widget.path(), "scope":{"repository":"acme/widget"}}),
        )
        .await;
    assert_eq!(both["result"]["Err"]["code"], "invalid_argument", "{both}");
    daemon.stop().await;
}
