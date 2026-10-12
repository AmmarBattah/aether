#![allow(clippy::unwrap_used)]
mod support;

use aether_client::Client;
use aether_core::model::ModelRef;
use aether_protocol::{ArtifactQuery, CreateAgent, PublishArtifact};
use support::TestDaemon;

#[tokio::test]
async fn client_evidence_is_attributed_scoped_and_ordered_by_publication() {
    let daemon = TestDaemon::start().await;
    let client = Client::connect(&daemon.socket).await.unwrap();
    let agent = client
        .create_agent(CreateAgent {
            objective: Some("capture evidence".into()),
            start: false,
            model: Some(ModelRef::new("claude-code", "haiku")),
            ..Default::default()
        })
        .await
        .unwrap();
    let task = agent.current_task.unwrap();
    let request = PublishArtifact {
        task,
        name: "verification".into(),
        summary: "observed checks".into(),
        content_type: "application/json".into(),
        text: "{\"passed\":true}".into(),
    };
    let first = client.publish_artifact(request.clone()).await.unwrap();
    let second = client.publish_artifact(request.clone()).await.unwrap();
    assert_ne!(first.id, second.id);
    assert_eq!(first.digest, second.digest);
    assert_eq!(first.task_id, Some(task));
    assert_eq!(first.agent_id, Some(agent.id));
    assert!(!matches!(
        first.created_by,
        aether_core::event::Actor::Agent { .. }
    ));
    let (_, bytes, truncated) = client.artifact_bytes(second.id, 1024).await.unwrap();
    assert_eq!(bytes, request.text.as_bytes());
    assert!(!truncated);
    let latest = client
        .list_artifacts(ArtifactQuery {
            task: Some(task),
            name: Some("verification".into()),
            limit: 1,
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(latest[0].id, second.id);
    assert!(
        client
            .list_artifacts(ArtifactQuery {
                name: Some("absent".into()),
                ..Default::default()
            })
            .await
            .unwrap()
            .is_empty()
    );
    let mut invalid = request.clone();
    invalid.name.clear();
    assert!(client.publish_artifact(invalid).await.is_err());
    let mut invalid = request.clone();
    invalid.task = aether_core::id::TaskId::new();
    assert!(client.publish_artifact(invalid).await.is_err());
    let mut invalid = request;
    invalid.text = "x".repeat(1024 * 1024 + 1);
    assert!(client.publish_artifact(invalid).await.is_err());
    assert_eq!(
        client
            .list_artifacts(ArtifactQuery {
                task: Some(task),
                ..Default::default()
            })
            .await
            .unwrap()
            .len(),
        2
    );
    daemon.stop().await;
}
