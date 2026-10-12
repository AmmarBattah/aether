#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

mod support;

use aether_protocol::PROTOCOL_VERSION;
use serde_json::{Value, json};
use support::{RawConn, TestDaemon};

#[tokio::test]
async fn protocol_child_creation_requires_explicit_effort_including_keyed_requests() {
    let daemon = TestDaemon::start().await;
    let mut conn = RawConn::connect(&daemon.socket).await.unwrap();
    conn.hello(PROTOCOL_VERSION).await.unwrap();
    let root = conn
        .call(json!({
            "cmd": "create_agent", "role": "parent", "start": false,
            "model": {"provider": "claude-code", "model": "haiku"}
        }))
        .await;
    let parent = &root["result"]["Ok"]["data"];
    assert!(parent["id"].is_string(), "{root}");
    assert!(parent["config"]["effort"].is_null(), "{root}");
    for config in [Value::Null, json!({})] {
        for keyed in [false, true] {
            let mut request = json!({
                "cmd": "create_agent", "role": "child", "parent": parent["id"],
                "objective": "must not be written", "config": config, "start": false
            });
            if keyed {
                request["idempotency_key"] = json!("missing-child-effort");
            }
            let refused = conn.call(request).await;
            let error = &refused["result"]["Err"];
            assert_eq!(error["code"], "invalid_argument", "{refused}");
            assert!(
                error["message"].as_str().unwrap().contains(
                    "a sub-agent needs an explicit effort: low, medium, high, xhigh or max"
                ),
                "{refused}"
            );
        }
    }
    let agents = conn.call(json!({"cmd":"list_agents", "query": {}})).await;
    assert_eq!(agents["result"]["Ok"]["data"].as_array().unwrap().len(), 1);
    let tasks = conn.call(json!({"cmd":"list_tasks", "query": {}})).await;
    assert!(tasks["result"]["Ok"]["data"].as_array().unwrap().is_empty());
    let child = conn
        .call(json!({
            "cmd": "create_agent", "role": "child", "parent": parent["id"],
            "config": {"effort": "low"}, "start": false,
            "idempotency_key": "missing-child-effort"
        }))
        .await;
    assert_eq!(
        child["result"]["Ok"]["data"]["config"]["effort"], "low",
        "{child}"
    );
    daemon.stop().await;
}
