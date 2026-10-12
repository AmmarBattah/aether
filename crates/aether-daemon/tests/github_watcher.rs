#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

mod support;

use std::path::Path;
use std::time::Duration;

use aether_client::{Client, ClientError};
use aether_core::github::{
    GithubDispatchMode, GithubRepositoryConfig, GithubWatcherConfig, GithubWatcherStatus,
};
use aether_core::id::AgentId;
use aether_core::inbox::InboxKind;
use aether_core::model::ModelRef;
use aether_protocol::{CreateAgent, ErrorCode};
use aether_store::{SqliteStore, Store};
use support::TestDaemon;
use tokio_util::sync::CancellationToken;

async fn target(client: &Client) -> AgentId {
    client
        .create_agent(CreateAgent {
            start: false,
            model: Some(ModelRef::new("claude-code", "haiku")),
            ..Default::default()
        })
        .await
        .expect("create dormant target")
        .id
}

fn configuration(agent: AgentId) -> GithubWatcherConfig {
    GithubWatcherConfig {
        repositories: vec![GithubRepositoryConfig {
            repository: "Example/Project".into(),
            enabled: true,
            agent: Some(agent),
            dispatch: GithubDispatchMode::Fixed,
            allowed_users: None,
        }],
        ..Default::default()
    }
}

/// Writes an executable mock without this process ever holding a write fd on
/// it: a sibling test's fork inherits any such fd until its exec, and running
/// the mock meanwhile fails with ETXTBSY. A rename cannot help - the inherited
/// fd is on the same inode - so a child writes it.
fn write_script(path: &Path, contents: &str) {
    use std::os::unix::fs::PermissionsExt;

    let status = std::process::Command::new("/bin/sh")
        .args(["-c", r#"printf '%s' "$1" > "$2""#, "sh", contents])
        .arg(path)
        .status()
        .expect("spawn sh to write a mock");
    assert!(
        status.success(),
        "writing {} failed: {status}",
        path.display()
    );
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
        .expect("make the mock executable");
}

fn protocol_error(error: ClientError, code: ErrorCode, message: &str) {
    let ClientError::Protocol(error) = error else {
        panic!("expected protocol error, got {error:?}");
    };
    assert_eq!(error.code, code, "{error}");
    assert!(error.message.contains(message), "{error}");
}

#[tokio::test]
async fn invalid_targets_and_patterns_do_not_change_configuration() {
    let daemon = TestDaemon::start().await;
    let client = Client::connect(&daemon.socket).await.expect("client");
    let initial = client.github_watcher().await.expect("initial status");
    assert_eq!(initial, GithubWatcherStatus::default());

    let absent = AgentId::new();
    protocol_error(
        client
            .configure_github_watcher(configuration(absent), initial.revision)
            .await
            .expect_err("missing target"),
        ErrorCode::NotFound,
        &absent.to_string(),
    );

    let agent = target(&client).await;
    let mut config = configuration(agent);
    config.mention_regex = "[".into();
    protocol_error(
        client
            .configure_github_watcher(config, initial.revision)
            .await
            .expect_err("invalid regex"),
        ErrorCode::InvalidArgument,
        "mention_regex",
    );

    client.pause(agent, false).await.expect("pause target");
    let mut config = configuration(agent);
    config.enabled = true;
    protocol_error(
        client
            .configure_github_watcher(config.clone(), initial.revision)
            .await
            .expect_err("paused target"),
        ErrorCode::InvalidArgument,
        "paused or terminal",
    );
    client.cancel(agent, None).await.expect("cancel target");
    protocol_error(
        client
            .configure_github_watcher(config, initial.revision)
            .await
            .expect_err("terminal target"),
        ErrorCode::InvalidArgument,
        "paused or terminal",
    );
    assert_eq!(client.github_watcher().await.expect("status"), initial);
    daemon.stop().await;
}

#[tokio::test]
async fn allowlist_is_normalized_cleared_and_validated_through_the_daemon() {
    let daemon = TestDaemon::start().await;
    let client = Client::connect(&daemon.socket).await.expect("client");
    let agent = target(&client).await;
    let initial = client.github_watcher().await.expect("initial status");
    let mut config = configuration(agent);
    config.repositories[0].allowed_users =
        Some(vec!["octoCat".into(), "MonaLisa".into(), "OCTOCAT".into()]);
    let narrowed = client
        .configure_github_watcher(config.clone(), initial.revision)
        .await
        .expect("set allowlist");
    assert_eq!(
        narrowed.config.repositories[0].allowed_users,
        Some(vec!["octocat".into(), "monalisa".into()])
    );

    for (users, message) in [
        (Vec::new(), "allowed_users is empty"),
        (vec!["Some--Stranger".to_owned()], "invalid GitHub login"),
    ] {
        config.repositories[0].allowed_users = Some(users);
        protocol_error(
            client
                .configure_github_watcher(config.clone(), narrowed.revision)
                .await
                .expect_err("invalid allowlist"),
            ErrorCode::InvalidArgument,
            message,
        );
    }
    assert_eq!(client.github_watcher().await.expect("unchanged"), narrowed);

    config.repositories[0].allowed_users = None;
    let cleared = client
        .configure_github_watcher(config, narrowed.revision)
        .await
        .expect("clear allowlist");
    assert_eq!(cleared.config.repositories[0].allowed_users, None);
    assert_eq!(cleared.revision, narrowed.revision + 1);
    daemon.stop().await;
}

#[tokio::test]
async fn operator_users_are_normalized_cleared_and_validated_through_the_daemon() {
    let daemon = TestDaemon::start().await;
    let client = Client::connect(&daemon.socket).await.expect("client");
    let agent = target(&client).await;
    let initial = client.github_watcher().await.expect("initial status");
    assert_eq!(initial.config.operator_users, None, "nobody by default");
    let mut config = configuration(agent);
    config.operator_users = Some(vec!["MengSig".into(), "octoCat".into(), "MENGSIG".into()]);
    let named = client
        .configure_github_watcher(config.clone(), initial.revision)
        .await
        .expect("set operator logins");
    assert_eq!(
        named.config.operator_users,
        Some(vec!["mengsig".into(), "octocat".into()])
    );
    assert_eq!(named.config.repositories[0].allowed_users, None);

    for (users, message) in [
        (Vec::new(), "operator_users is empty"),
        (vec!["robot[bot]".to_owned()], "invalid GitHub login"),
    ] {
        config.operator_users = Some(users);
        protocol_error(
            client
                .configure_github_watcher(config.clone(), named.revision)
                .await
                .expect_err("invalid operator logins"),
            ErrorCode::InvalidArgument,
            message,
        );
    }
    config.operator_users = Some(vec!["mallory".into()]);
    protocol_error(
        client
            .configure_github_watcher(config.clone(), initial.revision)
            .await
            .expect_err("stale writer"),
        ErrorCode::InvalidArgument,
        "changed",
    );
    assert_eq!(client.github_watcher().await.expect("unchanged"), named);

    config.operator_users = None;
    let cleared = client
        .configure_github_watcher(config, named.revision)
        .await
        .expect("clear operator logins");
    assert_eq!(cleared.config.operator_users, None);
    assert_eq!(cleared.revision, named.revision + 1);
    daemon.stop().await;
}

/// A liveness bound, not a speed check: CI shares a host whose disk
/// can stall for tens of seconds.
const DAEMON_DEADLINE: Duration = Duration::from_secs(60);

struct RunningDaemon {
    client: Client,
    shutdown: CancellationToken,
    serving: tokio::task::JoinHandle<anyhow::Result<()>>,
}

impl RunningDaemon {
    async fn start(root: &Path) -> Self {
        std::fs::create_dir_all(root.join("workspace")).expect("workspace");
        let socket = root.join("aetherd.sock");
        let config = aether_daemon::DaemonConfig {
            socket: socket.clone(),
            store_path: root.join("aether.db"),
            config_dir: Some(root.join("config")),
            workspace_root: Some(root.join("workspace")),
            max_concurrent_turns: None,
            yolo: false,
            tcp: None,
            allow_remote: false,
        };
        let shutdown = CancellationToken::new();
        let serving = tokio::spawn(aether_daemon::run(config, shutdown.clone()));
        let client = tokio::time::timeout(DAEMON_DEADLINE, async {
            loop {
                assert!(!serving.is_finished(), "daemon exited before answering");
                if let Ok(client) = Client::connect(&socket).await {
                    break client;
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        })
        .await
        .expect("daemon answers within the deadline");
        Self {
            client,
            shutdown,
            serving,
        }
    }

    async fn stop(self) {
        self.shutdown.cancel();
        tokio::time::timeout(DAEMON_DEADLINE, self.serving)
            .await
            .expect("daemon stops within the deadline")
            .expect("daemon task")
            .expect("clean daemon shutdown");
    }
}

#[tokio::test]
async fn configuration_revision_and_repository_settings_survive_restart() {
    let dir = tempfile::tempdir().expect("tempdir");
    let daemon = RunningDaemon::start(dir.path()).await;
    let agent = target(&daemon.client).await;
    let initial = daemon.client.github_watcher().await.expect("status");
    let mut config = configuration(agent);
    config.mention_regex = "@test-watcher".into();
    config.label_regex = "^test-watcher$".into();
    config.poll_interval_secs = 600;
    config.repositories[0].allowed_users = Some(vec!["octocat".into(), "monalisa".into()]);
    config.operator_users = Some(vec!["mengsig".into()]);
    let saved = daemon
        .client
        .configure_github_watcher(config.clone(), initial.revision)
        .await
        .expect("save disabled configuration");
    config.repositories[0].repository = "example/project".into();
    assert_eq!(saved.config, config);
    assert_eq!(saved.revision, initial.revision + 1);
    assert_eq!(saved.config.repositories[0].repository, "example/project");
    assert_eq!(saved.config.repositories[0].agent, Some(agent));
    assert!(!saved.config.enabled);
    protocol_error(
        daemon
            .client
            .configure_github_watcher(initial.config.clone(), initial.revision)
            .await
            .expect_err("stale writer"),
        ErrorCode::InvalidArgument,
        "changed",
    );
    assert_eq!(daemon.client.github_watcher().await.expect("status"), saved);
    daemon.stop().await;

    let daemon = RunningDaemon::start(dir.path()).await;
    assert_eq!(
        daemon.client.github_watcher().await.expect("restored"),
        saved
    );
    protocol_error(
        daemon
            .client
            .configure_github_watcher(initial.config, initial.revision)
            .await
            .expect_err("stale revision stays stale after restart"),
        ErrorCode::InvalidArgument,
        "changed",
    );
    let updated = daemon
        .client
        .configure_github_watcher(GithubWatcherConfig::default(), saved.revision)
        .await
        .expect("current revision remains editable");
    assert_eq!(updated.revision, saved.revision + 1);
    assert!(updated.config.repositories.is_empty());
    daemon.stop().await;
}

/// Re-execute one ignored scenario with a private environment. Neither PATH
/// nor credentials are changed in the parallel test runner, and no real gh or
/// model executable is reachable by the child daemon.
async fn run_isolated(scenario: &str, gh: &str) {
    let dir = tempfile::tempdir().expect("fixture directory");
    write_script(&dir.path().join("gh"), gh);
    let output = tokio::time::timeout(
        Duration::from_secs(90),
        tokio::process::Command::new(std::env::current_exe().expect("test executable"))
            .args(["--ignored", "--exact", scenario, "--nocapture"])
            .env_clear()
            .env("PATH", dir.path())
            .env("HOME", dir.path())
            .env("AETHER_TEST_GH_FIXTURE", dir.path())
            .kill_on_drop(true)
            .output(),
    )
    .await
    .expect("isolated watcher scenario finishes")
    .expect("run isolated watcher scenario");
    assert!(
        output.status.success(),
        "isolated scenario failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
async fn mocked_gh_reaches_the_daemon_inbox_without_live_network() {
    run_isolated("isolated_github_watcher_scenario", MOCK_GH).await;
}

#[tokio::test]
async fn mocked_gh_verifies_an_operators_request_through_graphql() {
    run_isolated("isolated_operator_standing_scenario", MOCK_OPERATOR_GH).await;
}

const MOCK_GH: &str = r#"#!/bin/sh
set -eu
printf '%s\n' "$*" >> "$AETHER_TEST_GH_FIXTURE/calls"
[ "$#" -eq 7 ] && [ "$1" = api ] && [ "$2" = --hostname ] && [ "$3" = github.com ] && [ "$4" = --include ] && [ "$5" = -H ]
printf 'HTTP/2.0 200 OK\r\nX-RateLimit-Remaining: 4999\r\n\r\n'
case "$7" in
    repos/example/project/issues\?*)
        attempts=0
        while [ ! -f "$AETHER_TEST_GH_FIXTURE/issues.json" ]; do
            attempts=$((attempts + 1))
            [ "$attempts" -le 1000 ] || exit 1
            /bin/sleep 0.01
        done
        /bin/cat "$AETHER_TEST_GH_FIXTURE/issues.json" ;;
    repos/example/project/issues/comments\?*|repos/example/project/pulls/comments\?*|repos/example/project/pulls\?*) printf '[]' ;;
    repos/example/project/collaborators/maintainer/permission) printf '{"permission":"write"}' ;;
    *) printf 'unexpected endpoint: %s\n' "$7" >&2; exit 1 ;;
esac
"#;

#[tokio::test]
#[ignore = "run by mocked_gh_reaches_the_daemon_inbox_without_live_network with isolated PATH"]
async fn isolated_github_watcher_scenario() {
    let fixture = std::env::var_os("AETHER_TEST_GH_FIXTURE").expect("isolated fixture environment");
    let fixture = Path::new(&fixture);
    let dir = tempfile::tempdir().expect("daemon directory");
    let daemon = RunningDaemon::start(dir.path()).await;
    let initial = daemon.client.github_watcher().await.expect("status");
    assert_eq!(initial, GithubWatcherStatus::default());
    tokio::time::sleep(Duration::from_millis(5200)).await;
    assert!(
        !fixture.join("calls").exists(),
        "disabled default invokes no gh"
    );

    let agent = target(&daemon.client).await;
    let disabled = daemon
        .client
        .configure_github_watcher(configuration(agent), initial.revision)
        .await
        .expect("configure repository while globally disabled");
    tokio::time::sleep(Duration::from_millis(5200)).await;
    assert!(
        !fixture.join("calls").exists(),
        "global toggle prevents polling"
    );

    let mut config = disabled.config;
    config.enabled = true;
    config.repositories[0].enabled = false;
    let disabled_repo = daemon
        .client
        .configure_github_watcher(config, disabled.revision)
        .await
        .expect("enable watcher with repository disabled");
    tokio::time::sleep(Duration::from_millis(5200)).await;
    assert!(
        !fixture.join("calls").exists(),
        "repository toggle prevents polling"
    );

    let mut config = disabled_repo.config;
    config.repositories[0].enabled = true;
    let enabled = daemon
        .client
        .configure_github_watcher(config, disabled_repo.revision)
        .await
        .expect("enable repository");
    // Use the public baseline timestamp, not a wall-clock guess that might be
    // older than activation or newer than the polling pass's upper bound.
    let timestamp = enabled.repositories[0]
        .next_poll_at
        .expect("initial poll time");
    let issue = serde_json::json!([{
        "id": 101,
        "number": 7,
        "title": "A test request",
        "body": "@aether please investigate this regression",
        "created_at": timestamp,
        "updated_at": timestamp,
        "html_url": "https://github.com/example/project/issues/7",
        "user": {"login": "maintainer", "type": "User"},
        "labels": []
    }]);
    std::fs::write(fixture.join("issues.tmp"), issue.to_string()).expect("issue response");
    std::fs::rename(fixture.join("issues.tmp"), fixture.join("issues.json"))
        .expect("publish complete response atomically");
    let polled = tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let status = daemon.client.github_watcher().await.expect("poll status");
            let repo = &status.repositories[0];
            assert!(repo.last_error.is_none(), "poll failed: {repo:?}");
            if repo.last_success_at.is_some() {
                break status;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("watcher completes a real gh subprocess pass");
    assert_eq!(polled.repositories[0].dispatched, 1);
    assert_eq!(polled.repositories[0].ignored, 0);
    let calls = std::fs::read_to_string(fixture.join("calls")).expect("gh invocation log");
    assert!(calls.contains("repos/example/project/issues?"), "{calls}");
    assert!(
        calls.contains("repos/example/project/collaborators/maintainer/permission"),
        "{calls}"
    );
    daemon.stop().await;

    // Inspect the real durable inbox, not a fake source's return value or a
    // dispatch counter. A GitHub request must retain external provenance.
    let store = SqliteStore::open(dir.path().join("aether.db")).expect("store");
    let inbox = store.inbox(agent, 0).expect("inbox");
    let triggers: Vec<_> = inbox
        .iter()
        .filter_map(|item| match &item.kind {
            InboxKind::ExternalTrigger {
                source,
                payload,
                operator,
            } => Some((source, payload, operator)),
            _ => None,
        })
        .collect();
    assert_eq!(triggers.len(), 1, "{inbox:?}");
    let (source, payload, operator) = triggers[0];
    assert_eq!(source, "github");
    assert!(!operator, "no operator_users are set");
    assert_eq!(payload["operator"], false);
    assert_eq!(payload["repository"], "example/project");
    assert_eq!(payload["number"], 7);
    assert_eq!(payload["requester"], "maintainer");
    assert_eq!(payload["kind"], "issue");
    assert!(payload["text"].as_str().expect("text").contains("@aether"));
    assert!(
        payload["trust"]
            .as_str()
            .expect("trust")
            .contains("not authenticated operator")
    );
    drop(store);

    let daemon = RunningDaemon::start(dir.path()).await;
    assert_eq!(
        daemon
            .client
            .github_watcher()
            .await
            .expect("restored health"),
        polled
    );
    daemon.stop().await;
}

/// Like `MOCK_GH`, plus the authorship lookup: the request body arrives on
/// stdin and is kept for the scenario to inspect.
const MOCK_OPERATOR_GH: &str = r#"#!/bin/sh
set -eu
printf '%s\n' "$*" >> "$AETHER_TEST_GH_FIXTURE/calls"
[ "$#" -eq 7 ] && [ "$1" = api ] && [ "$2" = --hostname ] && [ "$3" = github.com ] && [ "$4" = --include ]
printf 'HTTP/2.0 200 OK\r\nX-RateLimit-Remaining: 4999\r\n\r\n'
if [ "$5" = graphql ]; then
    [ "$6" = --input ] && [ "$7" = - ]
    /bin/cat > "$AETHER_TEST_GH_FIXTURE/lookup.json"
    /bin/cat "$AETHER_TEST_GH_FIXTURE/nodes.json"
    exit 0
fi
[ "$5" = -H ]
case "$7" in
    repos/example/project/issues\?*)
        attempts=0
        while [ ! -f "$AETHER_TEST_GH_FIXTURE/issues.json" ]; do
            attempts=$((attempts + 1))
            [ "$attempts" -le 1000 ] || exit 1
            /bin/sleep 0.01
        done
        /bin/cat "$AETHER_TEST_GH_FIXTURE/issues.json" ;;
    repos/example/project/issues/comments\?*|repos/example/project/pulls/comments\?*|repos/example/project/pulls\?*) printf '[]' ;;
    repos/example/project/collaborators/mengsig/permission) printf '{"permission":"admin"}' ;;
    *) printf 'unexpected endpoint: %s\n' "$7" >&2; exit 1 ;;
esac
"#;

/// Publish an operator's own unedited issue: the REST listing the mock
/// serves, and the node its GraphQL lookup answers with.
fn publish_operator_issue(fixture: &Path, timestamp: aether_core::time::Timestamp) {
    let nodes = serde_json::json!({"data": {"nodes": [{
        "__typename": "Issue", "id": "I_kwDOexample7",
        "author": {"__typename": "User", "login": "mengsig"},
        "editor": null, "lastEditedAt": null,
        "title": "A test request",
        "body": "@aether please investigate this regression",
        "timelineItems": {"nodes": []}
    }]}});
    std::fs::write(fixture.join("nodes.json"), nodes.to_string()).expect("lookup response");
    let issue = serde_json::json!([{
        "id": 101, "node_id": "I_kwDOexample7", "number": 7,
        "title": "A test request",
        "body": "@aether please investigate this regression",
        "created_at": timestamp, "updated_at": timestamp,
        "html_url": "https://github.com/example/project/issues/7",
        "user": {"login": "mengsig", "type": "User"},
        "labels": []
    }]);
    std::fs::write(fixture.join("issues.tmp"), issue.to_string()).expect("issue response");
    std::fs::rename(fixture.join("issues.tmp"), fixture.join("issues.json"))
        .expect("publish complete response atomically");
}

/// The lookup names the text by id in variables; no text reaches the query.
fn assert_one_lookup_by_id(fixture: &Path) {
    let lookup: serde_json::Value = serde_json::from_slice(
        &std::fs::read(fixture.join("lookup.json")).expect("gh received the lookup on stdin"),
    )
    .expect("lookup is JSON");
    assert_eq!(
        lookup["variables"],
        serde_json::json!({"ids": ["I_kwDOexample7"]})
    );
    assert!(
        lookup["query"].as_str().expect("query").contains("editor"),
        "{lookup}"
    );
    let calls = std::fs::read_to_string(fixture.join("calls")).expect("gh invocation log");
    assert_eq!(calls.matches("graphql --input -").count(), 1, "{calls}");
}

#[tokio::test]
#[ignore = "run by mocked_gh_verifies_an_operators_request_through_graphql with isolated PATH"]
async fn isolated_operator_standing_scenario() {
    let fixture = std::env::var_os("AETHER_TEST_GH_FIXTURE").expect("isolated fixture environment");
    let fixture = Path::new(&fixture);
    let dir = tempfile::tempdir().expect("daemon directory");
    let daemon = RunningDaemon::start(dir.path()).await;
    let initial = daemon.client.github_watcher().await.expect("status");
    let agent = target(&daemon.client).await;
    let mut config = configuration(agent);
    config.enabled = true;
    config.operator_users = Some(vec!["MengSig".into()]);
    let enabled = daemon
        .client
        .configure_github_watcher(config, initial.revision)
        .await
        .expect("enable with an operator login");
    let baseline = enabled.repositories[0]
        .next_poll_at
        .expect("initial poll time");
    publish_operator_issue(fixture, baseline);
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let status = daemon.client.github_watcher().await.expect("poll status");
            let repo = &status.repositories[0];
            assert!(repo.last_error.is_none(), "poll failed: {repo:?}");
            if repo.last_success_at.is_some() {
                assert_eq!((repo.dispatched, repo.ignored), (1, 0));
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("watcher completes a real gh subprocess pass");
    daemon.stop().await;
    assert_one_lookup_by_id(fixture);

    let store = SqliteStore::open(dir.path().join("aether.db")).expect("store");
    let inbox = store.inbox(agent, 0).expect("inbox");
    assert_eq!(inbox.len(), 1, "{inbox:?}");
    let InboxKind::ExternalTrigger {
        source,
        payload,
        operator,
    } = &inbox[0].kind
    else {
        panic!("a GitHub trigger: {inbox:?}");
    };
    assert_eq!(source, "github");
    assert!(*operator, "{payload}");
    assert_eq!(payload["operator"], true);
    assert_eq!(payload["requester"], "mengsig");
    let trust = payload["trust"].as_str().expect("trust");
    assert!(
        trust.starts_with("`mengsig` is listed in the watcher's operator_users"),
        "{trust}"
    );
}

#[tokio::test]
async fn automatic_dispatch_clones_and_reuses_a_thread_without_live_services() {
    let fixture = tempfile::tempdir().expect("fixture directory");
    for (name, script) in [
        ("gh", MOCK_AUTOMATIC_GH),
        ("git", MOCK_AUTOMATIC_GIT),
        ("claude", MOCK_AUTOMATIC_CLAUDE),
    ] {
        write_script(&fixture.path().join(name), script);
    }
    let output = tokio::time::timeout(
        Duration::from_secs(60),
        tokio::process::Command::new(std::env::current_exe().expect("test executable"))
            .args([
                "--ignored",
                "--exact",
                "isolated_automatic_dispatch_scenario",
                "--nocapture",
            ])
            .env_clear()
            .env("PATH", fixture.path())
            .env("HOME", fixture.path())
            .env("AETHER_TEST_GH_FIXTURE", fixture.path())
            .kill_on_drop(true)
            .output(),
    )
    .await
    .expect("isolated automatic scenario finishes")
    .expect("run isolated automatic scenario");
    assert!(
        output.status.success(),
        "isolated scenario failed:\nstdout: {}\nstderr: {}\nclaude calls: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
        std::fs::read_to_string(fixture.path().join("claude-calls")).unwrap_or_default()
    );
}

const MOCK_AUTOMATIC_GH: &str = r#"#!/bin/sh
set -eu
printf 'HTTP/2.0 200 OK\r\nX-RateLimit-Remaining: 4999\r\n\r\n'
case "$7" in
    repos/example/project/issues/comments\?*)
        attempts=0
        while [ ! -f "$AETHER_TEST_GH_FIXTURE/comments.json" ]; do
            attempts=$((attempts + 1))
            [ "$attempts" -le 1000 ] || exit 1
            /bin/sleep 0.01
        done
        /bin/cat "$AETHER_TEST_GH_FIXTURE/comments.json" ;;
    repos/example/project/collaborators/maintainer/permission) printf '{"permission":"write"}' ;;
    repos/example/project/issues\?*|repos/example/project/pulls/comments\?*|repos/example/project/pulls\?*) printf '[]' ;;
    *) printf 'unexpected endpoint: %s\n' "$7" >&2; exit 1 ;;
esac
"#;

const MOCK_AUTOMATIC_GIT: &str = r#"#!/bin/sh
set -eu
printf '%s\n' "$*" >> "$AETHER_TEST_GH_FIXTURE/git-calls"
case "$*" in
    *"clone --quiet"*)
        for arg do destination="$arg"; done
        /bin/mkdir -p "$destination/.git" ;;
    *"remote get-url origin"*) printf 'https://github.com/example/project.git\n' ;;
    *) printf 'unexpected git command: %s\n' "$*" >&2; exit 1 ;;
esac
"#;

const MOCK_AUTOMATIC_CLAUDE: &str = r#"#!/bin/sh
printf '%s\n' "$*" >> "$AETHER_TEST_GH_FIXTURE/claude-calls"
case "$*" in
    *"--version"*) printf '2.1.0\n' ;;
    *"auth status"*) printf 'Logged in\n' ;;
    *) exec /bin/sleep 30 ;;
esac
"#;

#[tokio::test]
#[ignore = "run by automatic_dispatch_clones_and_reuses_a_thread_without_live_services with isolated PATH"]
async fn isolated_automatic_dispatch_scenario() {
    let fixture = std::env::var_os("AETHER_TEST_GH_FIXTURE").expect("isolated fixture environment");
    let fixture = Path::new(&fixture);
    let dir = tempfile::tempdir().expect("daemon directory");
    std::fs::create_dir_all(dir.path().join("config")).expect("config directory");
    std::fs::write(
        dir.path().join("config/config.toml"),
        "[model]\ndefault = \"claude-code/haiku\"\n",
    )
    .expect("default model");
    let daemon = RunningDaemon::start(dir.path()).await;
    let status = daemon
        .client
        .github_watcher()
        .await
        .expect("initial status");
    let enabled = daemon
        .client
        .configure_github_watcher(
            GithubWatcherConfig {
                enabled: true,
                repositories: vec![GithubRepositoryConfig::automatic("example/project".into())],
                ..Default::default()
            },
            status.revision,
        )
        .await
        .expect("select repository");
    let timestamp = enabled.repositories[0]
        .next_poll_at
        .expect("initial poll time");
    let comments = serde_json::json!([
        {
            "id": 101,
            "body": "@Aether please inspect",
            "issue_url": "https://api.github.com/repos/example/project/issues/7",
            "html_url": "https://github.com/example/project/issues/7#issuecomment-101",
            "updated_at": timestamp,
            "user": {"login": "maintainer", "type": "User"}
        },
        {
            "id": 102,
            "body": "@aether follow up on this thread",
            "issue_url": "https://api.github.com/repos/example/project/issues/7",
            "html_url": "https://github.com/example/project/issues/7#issuecomment-102",
            "updated_at": timestamp,
            "user": {"login": "maintainer", "type": "User"}
        }
    ]);
    std::fs::write(fixture.join("comments.tmp"), comments.to_string()).expect("comments response");
    std::fs::rename(fixture.join("comments.tmp"), fixture.join("comments.json"))
        .expect("publish complete comments atomically");
    let polled = tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let status = daemon.client.github_watcher().await.expect("poll status");
            let repo = &status.repositories[0];
            assert!(repo.last_error.is_none(), "poll failed: {repo:?}");
            if repo.dispatched == 2 {
                break status;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("both comments reach one automatic thread");
    assert_eq!(polled.repositories[0].ignored, 0);
    let store = SqliteStore::open(dir.path().join("aether.db")).expect("store");
    let id: AgentId = store
        .meta("github.watcher.automatic-agent:example/project:7")
        .expect("mapping")
        .expect("mapped agent")
        .parse()
        .expect("agent id");
    let agent = store.agent(id).expect("agent lookup").expect("agent");
    let checkout = dir
        .path()
        .join("aether.github-workspaces/example/project/7/g0");
    assert_eq!(
        agent.scope.workspace_root.as_deref(),
        Some(checkout.as_path())
    );
    assert_eq!(
        agent.scope.repository.as_deref(),
        Some("github.com/example/project")
    );
    assert!(checkout.join(".git").is_dir());
    let triggers = store
        .inbox(id, 0)
        .expect("inbox")
        .into_iter()
        .filter(|item| matches!(item.kind, InboxKind::ExternalTrigger { .. }))
        .count();
    assert_eq!(triggers, 2);
    let git_calls = std::fs::read_to_string(fixture.join("git-calls")).expect("git calls");
    assert_eq!(git_calls.matches("clone --quiet").count(), 1, "{git_calls}");
    drop(store);
    daemon.stop().await;
}
