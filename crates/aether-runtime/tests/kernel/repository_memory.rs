//! repository memory scenarios and fixtures.
use super::*;

/// A CI clone's remote carries a job token. The repository id derived from it
/// reaches the system prompt, the immutable event log and every memory row,
/// and none of those can be retracted - so the token must never get into it.
#[tokio::test]
async fn a_token_in_the_origin_url_reaches_no_prompt_event_or_memory_scope() {
    const TOKEN: &str = "glcbt-0123456789abcdef";
    let h = harness(vec![ScriptedClient::text("I will use scripts/ci.sh.")]).await;
    let notes = h.workspace_root.join(aether_memory::repo::KNOWLEDGE_DIR);
    std::fs::create_dir_all(&notes).expect("mkdir");
    std::fs::write(
        notes.join("build.md"),
        "# Build\n\n- **[convention]** The whole build runs through scripts/ci.sh #build\n",
    )
    .expect("write");
    git_in(&h.workspace_root, &["init", "-q"]);
    git_in(
        &h.workspace_root,
        &["config", "user.email", "f@example.com"],
    );
    git_in(&h.workspace_root, &["config", "user.name", "fixture"]);
    git_in(&h.workspace_root, &["add", "-A"]);
    git_in(&h.workspace_root, &["commit", "-q", "-m", "notes"]);
    git_in(
        &h.workspace_root,
        &[
            "remote",
            "add",
            "origin",
            &format!("https://gitlab-ci-token:{TOKEN}@gitlab.example.com/org/repo.git"),
        ],
    );

    let (agent, _) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                role: "assistant".into(),
                start: true,
                config: Some(AgentConfig {
                    long_lived: true,
                    approval: ApprovalPolicy::none(),
                    ..Default::default()
                }),
                ..Default::default()
            },
            Actor::user(),
        )
        .expect("create");
    assert_eq!(
        agent.scope.repository.as_deref(),
        Some("gitlab.example.com/org/repo"),
        "the id is host and path, not the remote URL"
    );
    wait_for_seeding(&h, 1).await;
    h.wait_until(agent.id, |s| s.kind() == AgentStatusKind::Waiting)
        .await;
    h.runtime
        .send_message(
            agent.id,
            vec![ContentBlock::text("how do I run the build?")],
            Actor::user(),
        )
        .expect("send");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while h.client.remaining() > 0 {
        assert!(tokio::time::Instant::now() < deadline, "turn did not run");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    // Every surface the id reaches: the prompt the provider was sent, the
    // durable event log, and the memory rows keyed by it.
    for snapshot in h
        .store
        .invocations_for_agent(agent.id)
        .expect("inv")
        .into_iter()
        .map(|i| {
            h.store
                .snapshot(i.snapshot_id)
                .expect("snap")
                .expect("exists")
        })
    {
        let text = serde_json::to_string(&snapshot).expect("json");
        assert!(!text.contains(TOKEN), "the token reached a prompt: {text}");
    }
    let events = h
        .store
        .events(0, 10_000, &EventFilter::default())
        .expect("events");
    assert!(!events.is_empty());
    for event in &events {
        let text = serde_json::to_string(event).expect("json");
        assert!(
            !text.contains(TOKEN),
            "the token reached the event log: {text}"
        );
    }
    let memories = h
        .store
        .memories(&aether_store::MemoryQuery {
            include_superseded: true,
            include_forgotten: true,
            limit: 100,
            ..Default::default()
        })
        .expect("memories");
    assert!(!memories.is_empty(), "the notes were seeded");
    for memory in &memories {
        let text = serde_json::to_string(memory).expect("json");
        assert!(!text.contains(TOKEN), "the token reached memory: {text}");
    }
}

/// A repository documents itself for the people who work in it. An agent
/// working there starts from the same notes, and can check any of them
/// against the file and commit it came from.
#[tokio::test]
async fn a_repositorys_committed_notes_reach_the_agents_context() {
    let h = harness(vec![ScriptedClient::text("I will use scripts/ci.sh.")]).await;
    let notes = h.workspace_root.join(aether_memory::repo::KNOWLEDGE_DIR);
    std::fs::create_dir_all(&notes).expect("mkdir");
    std::fs::write(
        notes.join("build.md"),
        "# Build\n\n- **[convention]** The whole build runs through scripts/ci.sh #build\n",
    )
    .expect("write");

    let (agent, _) = h
        .runtime
        .create_agent(
            CreateAgentSpec {
                role: "assistant".into(),
                start: true,
                config: Some(AgentConfig {
                    long_lived: true,
                    approval: ApprovalPolicy::none(),
                    ..Default::default()
                }),
                ..Default::default()
            },
            Actor::user(),
        )
        .expect("create");
    assert!(
        agent.scope.repository.is_some(),
        "a workspace names a repository, or its memory is unreachable"
    );
    wait_for_seeding(&h, 1).await;
    h.wait_until(agent.id, |s| s.kind() == AgentStatusKind::Waiting)
        .await;
    h.runtime
        .send_message(
            agent.id,
            vec![ContentBlock::text("how do I run the build?")],
            Actor::user(),
        )
        .expect("send");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while h.client.remaining() > 0 {
        assert!(tokio::time::Instant::now() < deadline, "turn did not run");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    let turns = h.store.invocations_for_agent(agent.id).expect("inv");
    let snapshot = h
        .store
        .snapshot(turns.last().expect("turn").snapshot_id)
        .expect("snap")
        .expect("exists");
    let layer = snapshot
        .layers
        .iter()
        .find(|l| l.kind == aether_core::snapshot::LayerKind::RetrievedMemory)
        .and_then(|l| l.text.as_deref())
        .expect("the memory layer carries the repository's notes");
    assert!(layer.contains("scripts/ci.sh"), "{layer}");
    assert!(
        layer.contains(".aether/knowledge/build.md"),
        "the model is told where the fact came from: {layer}"
    );
    let seeded = h
        .store
        .memory(*snapshot.memory_ids.first().expect("one memory"))
        .expect("memory")
        .expect("exists");
    assert_eq!(seeded.kind, aether_core::memory::MemoryKind::Preference);
    assert_eq!(seeded.tags, vec!["build"]);
}

/// A supervisor fanning out narrows each worker to a subdirectory. In a
/// workspace with no `origin` the id falls back to the canonical path, so a
/// child that derived its own would land in a different memory scope: unable
/// to read the repository's notes, and writing memory its siblings cannot see.
#[tokio::test]
async fn a_narrowed_child_shares_its_parents_repository_scope() {
    let h = harness(Vec::new()).await;
    // A repository with no `origin` (and its own git dir, so the scan does
    // not walk up into whatever repository the tempdir happens to sit in).
    // The id is then the path, which the child's own path is not.
    git_in(&h.workspace_root, &["init", "-q"]);
    let (parent, _) = h
        .runtime
        .create_agent(spec("index the repository"), Actor::user())
        .expect("parent");
    let repository = parent
        .scope
        .repository
        .clone()
        .expect("a workspace names a repository");
    assert_eq!(repository, h.workspace_root.to_string_lossy());

    let sub = h.workspace_root.join("crates").join("worker");
    std::fs::create_dir_all(&sub).expect("mkdir");
    let mut child = spec("index one crate");
    child.parent = Some(parent.id);
    child.scope = Some(Scope {
        workspace_root: Some(sub),
        ..parent.scope.clone()
    });
    child.scope.as_mut().expect("scope").repository = None;
    let (child, _) = h.runtime.create_agent(child, Actor::user()).expect("child");
    assert_eq!(
        child.scope.repository.as_deref(),
        Some(repository.as_str()),
        "the child derived its own repository scope"
    );

    // And naming a different one is not a narrowing.
    let mut escaped = spec("index someone else's repository");
    escaped.parent = Some(parent.id);
    escaped.scope = Some(Scope {
        repository: Some("somewhere/else".into()),
        ..parent.scope.clone()
    });
    let err = h
        .runtime
        .create_agent(escaped, Actor::user())
        .expect_err("a different memory scope is not a narrower one");
    assert!(err.to_string().contains("narrow"), "{err}");
}

/// A caller-named repository decides what memory an agent can read and
/// write, so an id that will not normalize must be refused with what was
/// asked for, not silently swapped for the parent's scope or a derived one -
/// a relative, non-URL id carrying `@` (a scoped package directory, not a
/// credential) normalizes to nothing today.
#[tokio::test]
async fn a_repository_id_that_will_not_normalize_is_refused_not_substituted() {
    let h = harness(Vec::new()).await;
    let (parent, _) = h
        .runtime
        .create_agent(spec("index the repository"), Actor::user())
        .expect("parent");

    let mut child = spec("index a package directory");
    child.parent = Some(parent.id);
    child.scope = Some(Scope {
        repository: Some("weird/@bad".into()),
        ..parent.scope.clone()
    });
    let err = h
        .runtime
        .create_agent(child, Actor::user())
        .expect_err("an id this cannot store safely is refused, not silently dropped");
    assert!(err.to_string().contains("weird/@bad"), "{err}");
}

/// The narrowing check must compare what the two ids resolve to, not their
/// literal spelling: a child naming the same repository as its parent in an
/// equivalent but unnormalized form must narrow, not be refused as if it
/// named a different repository.
#[tokio::test]
async fn naming_the_same_repository_in_an_unnormalized_form_still_narrows() {
    let h = harness(Vec::new()).await;
    let mut root = spec("index the repository");
    root.scope = Some(Scope {
        repository: Some("git@example.com:org/repo.git".into()),
        ..Default::default()
    });
    let (parent, _) = h.runtime.create_agent(root, Actor::user()).expect("parent");
    assert_eq!(
        parent.scope.repository.as_deref(),
        Some("example.com/org/repo")
    );

    let mut child = spec("index one crate");
    child.parent = Some(parent.id);
    child.scope = Some(Scope {
        repository: Some("https://example.com/org/repo.git".into()),
        ..parent.scope.clone()
    });
    let (child, _) = h
        .runtime
        .create_agent(child, Actor::user())
        .expect("the same repository in another spelling narrows");
    assert_eq!(
        child.scope.repository.as_deref(),
        Some("example.com/org/repo")
    );
}
