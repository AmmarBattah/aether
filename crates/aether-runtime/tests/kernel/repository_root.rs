//! A client naming the checkout an agent's memory belongs to.
use super::*;

/// A fresh repository outside the harness workspace, with `origin` if given.
fn checkout(origin: Option<&str>) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    git_in(dir.path(), &["init", "-q"]);
    if let Some(origin) = origin {
        git_in(dir.path(), &["remote", "add", "origin", origin]);
    }
    dir
}

fn named(objective: &str, root: &std::path::Path) -> CreateAgentSpec {
    CreateAgentSpec {
        repository_root: Some(root.to_path_buf()),
        ..spec(objective)
    }
}

fn working_in(objective: &str, root: &std::path::Path) -> CreateAgentSpec {
    CreateAgentSpec {
        scope: Some(Scope {
            workspace_root: Some(root.to_path_buf()),
            ..Default::default()
        }),
        ..spec(objective)
    }
}

/// Archon's agents work in worktrees of the operator's checkout. Naming that
/// checkout must land them in the scope a session started in it gets.
#[tokio::test]
async fn a_named_checkout_gets_the_id_an_agent_working_there_gets() {
    let h = harness(Vec::new()).await;
    let repo = checkout(Some("git@github.com:acme/widget.git"));
    let (direct, _) = h
        .runtime
        .create_agent(working_in("work there", repo.path()), Actor::user())
        .expect("direct");
    let (via_root, _) = h
        .runtime
        .create_agent(named("work elsewhere", repo.path()), Actor::user())
        .expect("named checkout");
    assert_eq!(
        via_root.scope.repository.as_deref(),
        Some("github.com/acme/widget")
    );
    assert_eq!(via_root.scope.repository, direct.scope.repository);
    assert_eq!(
        via_root.scope.workspace_root.as_deref(),
        Some(h.workspace_root.as_path()),
        "the checkout names memory, not where the agent works"
    );
    assert_eq!(via_root.scope.network, h.runtime.config().network_default);
}

/// Without `origin` the id is the canonical path, so a symlink to the
/// checkout names the same scope as the checkout itself.
#[tokio::test]
async fn a_checkout_without_origin_is_named_by_its_canonical_path() {
    let h = harness(Vec::new()).await;
    let repo = checkout(None);
    let links = tempfile::tempdir().expect("tempdir");
    let link = links.path().join("widget");
    std::os::unix::fs::symlink(repo.path(), &link).expect("symlink");
    let (via_link, _) = h
        .runtime
        .create_agent(named("work elsewhere", &link), Actor::user())
        .expect("named through a symlink");
    let canonical = repo.path().canonicalize().expect("canonical");
    assert_eq!(
        via_link.scope.repository.as_deref(),
        Some(canonical.to_string_lossy().as_ref())
    );
    let (direct, _) = h
        .runtime
        .create_agent(working_in("work there", repo.path()), Actor::user())
        .expect("direct");
    assert_eq!(via_link.scope.repository, direct.scope.repository);
}

/// Every refusal names what was wrong and creates nothing.
#[tokio::test]
async fn a_checkout_that_cannot_name_a_repository_is_refused() {
    let h = harness(Vec::new()).await;
    let repo = checkout(Some("git@github.com:acme/widget.git"));
    let file = repo.path().join("README.md");
    std::fs::write(&file, "widget").expect("write");
    let both = CreateAgentSpec {
        scope: Some(Scope {
            repository: Some("acme/widget".into()),
            ..Default::default()
        }),
        ..named("both", repo.path())
    };
    let cases = [
        (
            named("relative", std::path::Path::new("widget")),
            "absolute",
        ),
        (named("missing", &repo.path().join("gone")), "gone"),
        (named("file", &file), "not a directory"),
        (both, "not both"),
    ];
    let before = h
        .store
        .agents(&AgentFilter::default())
        .expect("agents")
        .len();
    for (case, expected) in cases {
        let objective = case.objective.clone().expect("objective");
        let err = h
            .runtime
            .create_agent(case, Actor::user())
            .expect_err(&objective);
        assert!(
            matches!(err, aether_runtime::RuntimeError::Invalid(_)),
            "{objective}: {err}"
        );
        assert!(err.to_string().contains(expected), "{objective}: {err}");
    }
    let after = h
        .store
        .agents(&AgentFilter::default())
        .expect("agents")
        .len();
    assert_eq!(before, after, "a refused create leaves no agent behind");
}

/// The derived id is the parent's scope, so the usual narrowing applies: a
/// child inherits it, and naming another checkout is not narrower.
#[tokio::test]
async fn a_child_of_a_named_checkout_inherits_its_repository() {
    let h = harness(Vec::new()).await;
    let repo = checkout(Some("git@github.com:acme/widget.git"));
    let (parent, _) = h
        .runtime
        .create_agent(named("supervise", repo.path()), Actor::user())
        .expect("parent");
    let mut child = spec("work");
    child.parent = Some(parent.id);
    let (child, _) = h.runtime.create_agent(child, Actor::user()).expect("child");
    assert_eq!(
        child.scope.repository.as_deref(),
        Some("github.com/acme/widget")
    );

    let mut same = named("work in the same checkout", repo.path());
    same.parent = Some(parent.id);
    let (same, _) = h
        .runtime
        .create_agent(same, Actor::user())
        .expect("the same checkout narrows");
    assert_eq!(same.scope.repository, parent.scope.repository);

    let other = checkout(Some("git@github.com:acme/gadget.git"));
    let mut escaped = named("work in another checkout", other.path());
    escaped.parent = Some(parent.id);
    let err = h
        .runtime
        .create_agent(escaped, Actor::user())
        .expect_err("another repository is not narrower");
    assert!(err.to_string().contains("`repository` widens"), "{err}");
}
