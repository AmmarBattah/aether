#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

//! Version skew against a real daemon: N and N-1 are served, anything else is
//! refused at hello with both bounds named.

mod support;

use aether_core::github::GithubWatcherConfig;
use aether_protocol::PROTOCOL_VERSION;
use aether_protocol::compat::MIN_PROTOCOL_VERSION;
use serde_json::{Value, json};
use support::{RawConn, TestDaemon};

async fn save_watcher(conn: &mut RawConn, config: &Value, revision: u64) -> Value {
    conn.call(json!({
        "cmd":"configure_github_watcher",
        "config":config,
        "expected_revision":revision
    }))
    .await
}

fn revision_of(reply: &Value) -> u64 {
    reply["result"]["Ok"]["data"]["revision"]
        .as_u64()
        .unwrap_or_else(|| panic!("a watcher reply with a revision: {reply}"))
}

fn assert_needs_protocol_17(reply: &Value) {
    let error = &reply["result"]["Err"];
    assert_eq!(error["code"], "incompatible", "{reply}");
    let message = error["message"].as_str().unwrap_or_default();
    assert!(message.contains("protocol 17"), "{reply}");
}

#[tokio::test]
async fn previous_version_preserves_watcher_allowlists() {
    let daemon = TestDaemon::start().await;
    let mut old = RawConn::connect(&daemon.socket).await.unwrap();
    old.hello(MIN_PROTOCOL_VERSION).await.unwrap();
    let initial = old.call(json!({"cmd":"get_github_watcher"})).await;
    let revision = initial["result"]["Ok"]["data"]["revision"]
        .as_u64()
        .unwrap();
    let mut config = serde_json::to_value(GithubWatcherConfig::default()).unwrap();
    config["repositories"] = json!([{"repository":"o/r", "enabled":true,
        "dispatch":{"mode":"automatic"}, "allowed_users":["octocat"]}]);
    let saved = old.call(json!({"cmd":"configure_github_watcher", "config":config, "expected_revision":revision})).await;
    assert_eq!(
        saved["result"]["Ok"]["data"]["config"]["repositories"][0]["allowed_users"],
        json!(["octocat"]),
        "{saved}"
    );
    let read = old.call(json!({"cmd":"get_github_watcher"})).await;
    assert_eq!(
        read["result"]["Ok"]["data"]["config"]["repositories"][0]["allowed_users"],
        json!(["octocat"])
    );
    daemon.stop().await;
}

#[tokio::test]
async fn v16_watcher_config_stays_editable_until_operator_users_are_present() {
    let daemon = TestDaemon::start().await;
    let mut old = RawConn::connect(&daemon.socket)
        .await
        .expect("v16 connection");
    old.hello(16).await.expect("v16 served");
    let initial = old.call(json!({"cmd":"get_github_watcher"})).await;
    assert_eq!(
        initial["result"]["Ok"]["reply"], "github_watcher",
        "{initial}"
    );
    // A v16 client keeps everything it can represent, allowlists included.
    let mut narrowed = serde_json::to_value(GithubWatcherConfig::default()).unwrap();
    assert!(narrowed.get("operator_users").is_none(), "{narrowed}");
    narrowed["repositories"] = json!([{
        "repository":"o/r", "enabled":true,
        "dispatch":{"mode":"automatic"},
        "allowed_users":["octoCat"]
    }]);
    let legacy = save_watcher(&mut old, &narrowed, revision_of(&initial)).await;
    let config = &legacy["result"]["Ok"]["data"]["config"];
    assert_eq!(
        config["repositories"][0]["allowed_users"],
        json!(["octocat"]),
        "{legacy}"
    );
    assert!(config.get("operator_users").is_none(), "{legacy}");

    let mut current = RawConn::connect(&daemon.socket)
        .await
        .expect("v17 connection");
    current.hello(PROTOCOL_VERSION).await.expect("v17 served");
    let mut with_operators = narrowed.clone();
    with_operators["operator_users"] = json!(["Mengsig", "octoCat"]);
    let saved = save_watcher(&mut current, &with_operators, revision_of(&legacy)).await;
    assert_eq!(
        saved["result"]["Ok"]["data"]["config"]["operator_users"],
        json!(["mengsig", "octocat"]),
        "{saved}"
    );
    let saved_revision = revision_of(&saved);

    let unreadable = old.call(json!({"cmd":"get_github_watcher"})).await;
    assert_needs_protocol_17(&unreadable);
    // The v16 save carries no operator_users and would drop the list.
    let overwrite = save_watcher(&mut old, &narrowed, saved_revision).await;
    assert_needs_protocol_17(&overwrite);
    let kept = current.call(json!({"cmd":"get_github_watcher"})).await;
    assert_eq!(revision_of(&kept), saved_revision);
    assert_eq!(
        kept["result"]["Ok"]["data"]["config"]["operator_users"],
        json!(["mengsig", "octocat"])
    );
    assert_eq!(
        old.call(json!({"cmd":"ping"})).await["result"]["Ok"]["reply"],
        "ok"
    );
    // What version 16 itself added still answers while the list is set.
    let preview = old.call(json!({"cmd":"preview_retention"})).await;
    assert_eq!(
        preview["result"]["Ok"]["reply"], "retention_preview",
        "{preview}"
    );
    daemon.stop().await;
}

/// A v16 client may not introduce the list either: it could not read back or
/// preserve what it set. Once the list is cleared it edits the watcher again.
#[tokio::test]
async fn a_v16_client_can_neither_set_operator_users_nor_edit_around_them() {
    let daemon = TestDaemon::start().await;
    let mut old = RawConn::connect(&daemon.socket).await.expect("connect");
    old.hello(16).await.expect("v16 served");
    let initial = old.call(json!({"cmd":"get_github_watcher"})).await;
    let plain = serde_json::to_value(GithubWatcherConfig::default()).unwrap();
    let mut granting = plain.clone();
    granting["operator_users"] = json!(["mallory"]);
    let refused = save_watcher(&mut old, &granting, revision_of(&initial)).await;
    assert_needs_protocol_17(&refused);
    let unchanged = old.call(json!({"cmd":"get_github_watcher"})).await;
    assert_eq!(revision_of(&unchanged), revision_of(&initial));
    assert!(
        unchanged["result"]["Ok"]["data"]["config"]
            .get("operator_users")
            .is_none(),
        "{unchanged}"
    );

    let mut current = RawConn::connect(&daemon.socket).await.expect("connect");
    current.hello(PROTOCOL_VERSION).await.expect("v17 served");
    let mut listed = plain.clone();
    listed["operator_users"] = json!(["mengsig"]);
    let set = save_watcher(&mut current, &listed, revision_of(&initial)).await;
    let cleared = save_watcher(&mut current, &plain, revision_of(&set)).await;
    assert!(
        cleared["result"]["Ok"]["data"]["config"]
            .get("operator_users")
            .is_none(),
        "{cleared}"
    );
    let again = save_watcher(&mut old, &plain, revision_of(&cleared)).await;
    assert_eq!(again["result"]["Ok"]["reply"], "github_watcher", "{again}");
    daemon.stop().await;
}

/// Create an agent and pause it, returning the raw `paused` reply data.
async fn pause_reply(conn: &mut RawConn) -> serde_json::Value {
    let created = conn
        .call(json!({"cmd": "create_agent", "role": "skew", "start": false, "model": {"provider": "claude-code", "model": "haiku"}}))
        .await;
    let agent = created["result"]["Ok"]["data"]["id"]
        .as_str()
        .unwrap_or_else(|| panic!("an agent id: {created}"))
        .to_owned();
    let paused = conn.call(json!({"cmd": "pause", "agent": agent})).await;
    assert_eq!(
        paused["result"]["Ok"]["reply"], "paused",
        "expected a paused reply: {paused}"
    );
    paused["result"]["Ok"]["data"].clone()
}

#[tokio::test]
async fn the_current_version_is_matched_and_the_window_reported() {
    let daemon = TestDaemon::start().await;
    let mut conn = RawConn::connect(&daemon.socket).await.expect("connect");
    let welcome = conn.hello(PROTOCOL_VERSION).await.expect("served");
    assert_eq!(welcome["protocol_version"], PROTOCOL_VERSION);
    assert_eq!(welcome["daemon_protocol_version"], PROTOCOL_VERSION);
    assert_eq!(welcome["min_protocol_version"], MIN_PROTOCOL_VERSION);

    // At the current version the reply keeps its current shape.
    let data = pause_reply(&mut conn).await;
    assert!(data.get("agents").is_some(), "{data}");
    assert!(data.get("after_current_turn").is_none(), "{data}");
    daemon.stop().await;
}

/// N-1 is served rather than refused, and it goes on working: the reply it
/// gets is the one its own version reads. v10 and v11 share reply/event shapes;
/// v11's new operator requests are never sent by an older client.
#[tokio::test]
async fn a_client_one_version_back_is_served_and_keeps_working() {
    let daemon = TestDaemon::start().await;
    let mut conn = RawConn::connect(&daemon.socket).await.expect("connect");
    let welcome = conn.hello(MIN_PROTOCOL_VERSION).await.expect("served");
    assert_eq!(
        welcome["protocol_version"], MIN_PROTOCOL_VERSION,
        "the welcome states the version in effect: {welcome}"
    );
    assert_eq!(welcome["daemon_protocol_version"], PROTOCOL_VERSION);

    let data = pause_reply(&mut conn).await;
    assert!(data.get("agents").is_some(), "{data}");
    assert!(data.get("mid_turn").is_some(), "{data}");
    daemon.stop().await;
}

#[tokio::test]
async fn a_version_outside_the_window_is_refused_with_both_bounds() {
    let daemon = TestDaemon::start().await;
    // `saturating_sub`: MIN_PROTOCOL_VERSION is itself floored at 1, so a
    // plain `- 1` would underflow if the window ever starts there; the low
    // case is then MIN_PROTOCOL_VERSION itself, inside the window, so it is
    // skipped rather than asserted as a refusal.
    let too_low = MIN_PROTOCOL_VERSION.saturating_sub(1);
    let cases = [too_low, PROTOCOL_VERSION + 1]
        .into_iter()
        .filter(|&asked| asked != MIN_PROTOCOL_VERSION);
    for asked in cases {
        let mut conn = RawConn::connect(&daemon.socket).await.expect("connect");
        let refusal = conn.hello(asked).await.expect_err("refused");
        assert_eq!(refusal["code"], "incompatible", "{refusal}");
        let message = refusal["message"].as_str().unwrap_or_default().to_owned();
        assert!(
            message.contains(&MIN_PROTOCOL_VERSION.to_string())
                && message.contains(&PROTOCOL_VERSION.to_string()),
            "the refusal must name the served window: {message}"
        );
        assert!(
            conn.recv().await.is_none(),
            "a refused connection is closed, not left open"
        );
    }
    daemon.stop().await;
}

/// Spawn schedules arrived in v9, which is now the oldest version served, so a
/// client one version back works them like any other: nothing is refused and
/// nothing is hidden.
#[tokio::test]
async fn a_client_one_version_back_works_a_spawn_schedule() {
    let daemon = TestDaemon::start().await;
    let mut conn = RawConn::connect(&daemon.socket).await.expect("connect");
    conn.hello(MIN_PROTOCOL_VERSION).await.expect("served");
    let created = conn
        .call(json!({"cmd": "create_schedule", "spec": {
            "name": "nightly",
            "spawn": {
                "role": "coordinator",
                "objective": "Summarise overnight failures",
                "model": {"provider": "claude-code", "model": "haiku"}
            },
            "recurrence": {"kind": "interval", "seconds": 3600}
        }}))
        .await;
    let id = created["result"]["Ok"]["data"]["id"]
        .as_str()
        .unwrap_or_else(|| panic!("a schedule id: {created}"))
        .to_owned();

    let listed = conn.call(json!({"cmd": "list_schedules"})).await;
    assert_eq!(
        listed["result"]["Ok"]["data"][0]["id"],
        json!(id),
        "the schedule must be listed for N-1: {listed}"
    );
    let cancel = conn
        .call(json!({"cmd": "update_schedule", "id": id, "action": "cancel"}))
        .await;
    assert_eq!(cancel["result"]["Ok"]["reply"], "schedule", "{cancel}");
    let still = conn.call(json!({"cmd": "get_schedule", "id": id})).await;
    assert_eq!(
        still["result"]["Ok"]["data"]["status"]["state"], "cancelled",
        "the change must have applied: {still}"
    );
    daemon.stop().await;
}

#[tokio::test]
async fn operator_commands_round_trip_and_do_not_fabricate_authorship() {
    let daemon = TestDaemon::start().await;
    let mut conn = RawConn::connect(&daemon.socket).await.unwrap();
    conn.hello(PROTOCOL_VERSION).await.unwrap();
    let created = conn.call(json!({"cmd":"create_agent", "role":"operator-test", "start":false, "model":{"provider":"claude-code","model":"haiku"}})).await;
    let id = created["result"]["Ok"]["data"]["id"].as_str().unwrap();
    let renamed = conn
        .call(json!({"cmd":"update_agent", "id":id, "patch":{"name":" 名前 café "}}))
        .await;
    assert_eq!(renamed["result"]["Ok"]["data"]["name"], "名前 café");
    let invalid = conn
        .call(json!({"cmd":"update_agent", "id":id, "patch":{"name":"   "}}))
        .await;
    assert_eq!(invalid["result"]["Err"]["code"], "invalid_argument");
    let request = json!({"cmd":"create_memory", "spec":{"scope":{"scope":"agent","id":id},"kind":"fact","text":"Prefer deterministic checks before publishing changes."}});
    let admitted = conn.call(request.clone()).await;
    assert_eq!(admitted["result"]["Ok"]["reply"], "memory", "{admitted}");
    assert!(
        admitted["result"]["Ok"]["data"]["provenance"]
            .get("agent")
            .is_none()
    );
    let provenance = &admitted["result"]["Ok"]["data"]["provenance"];
    assert_eq!(provenance["actor"]["actor"], "user");
    assert_eq!(provenance["actor"]["origin"]["transport"], "local_socket");
    assert!(
        !provenance["actor"]["origin"]["connection"]
            .as_str()
            .unwrap()
            .is_empty()
    );
    let memories = conn.call(json!({"cmd":"memories", "query":{}})).await;
    assert_eq!(
        memories["result"]["Ok"]["data"][0]["provenance"],
        *provenance
    );
    let mut forged = request.clone();
    forged["spec"]["provenance"] = json!({"actor":{"actor":"user","name":"forged"}});
    forged["req"] = json!(1000);
    conn.send(&forged).await;
    let refused = conn.await_reply(1000).await;
    assert!(
        refused["result"]["Err"]["message"]
            .as_str()
            .unwrap()
            .contains("unknown field `provenance`"),
        "{refused}"
    );
    assert_eq!(admitted["result"]["Ok"]["data"]["scope"]["id"], id);
    let duplicate = conn.call(request).await;
    assert!(
        duplicate["result"]["Err"]["message"]
            .as_str()
            .unwrap()
            .contains("redundant")
    );
    daemon.stop().await;
}
