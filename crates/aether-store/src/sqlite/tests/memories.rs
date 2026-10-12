use super::*;

#[test]
fn memory_provenance_filter_precedes_limit_and_preserves_scope_and_status() {
    let store = SqliteStore::open_in_memory().unwrap();
    let author = AgentId::new();
    let other = AgentId::new();
    let mut commit = Commit::new();
    let mut authored = Vec::new();
    for i in 0..208 {
        let mut record = MemoryRecord::from_candidate(
            MemoryCandidate {
                scope: if i == 1 {
                    MemoryScope::Agent { id: other }
                } else {
                    MemoryScope::Global
                },
                kind: MemoryKind::Fact,
                text: format!("record {i}"),
                structured: None,
                importance: if i < 3 { 0.1 } else { 0.9 },
                confidence: 0.8,
                tags: vec![],
                provenance: MemoryProvenance {
                    agent: Some(if i < 3 { author } else { other }),
                    ..Default::default()
                },
                supersedes: None,
            },
            now(),
        );
        if i == 1 {
            record.forgotten_at = Some(now());
        }
        if i == 2 {
            record.superseded_by = Some(MemoryId::new());
        }
        if i < 3 {
            authored.push(record.id);
        }
        commit = commit.mutate(Mutation::PutMemory(record));
    }
    store.commit(commit).unwrap();
    let query = MemoryQuery {
        provenance_agent: Some(author),
        limit: 200,
        include_forgotten: true,
        include_superseded: true,
        ..Default::default()
    };
    let records = store.memories(&query).unwrap();
    assert_eq!(records.len(), 3);
    assert!(records.iter().all(|r| authored.contains(&r.id)));
    assert_eq!(
        store
            .memories(&MemoryQuery {
                provenance_agent: None,
                ..query.clone()
            })
            .unwrap()
            .len(),
        200
    );
    assert_eq!(
        store
            .memories(&MemoryQuery {
                include_forgotten: false,
                include_superseded: false,
                ..query.clone()
            })
            .unwrap()
            .len(),
        1
    );
    let scoped = store
        .memories(&MemoryQuery {
            scope: Some(MemoryScope::Agent { id: other }),
            exact_scope: true,
            ..query
        })
        .unwrap();
    assert_eq!(scoped.len(), 1);
    assert_eq!(scoped[0].id, authored[1]);
}

#[test]
fn memory_search_and_supersession() {
    let store = SqliteStore::open_in_memory().expect("open");
    let repo = MemoryScope::Repository {
        id: "aether".into(),
    };
    let make = |text: &str, scope: MemoryScope, tags: &[&str]| {
        MemoryRecord::from_candidate(
            MemoryCandidate {
                scope,
                kind: MemoryKind::Fact,
                text: text.into(),
                structured: None,
                importance: 0.7,
                confidence: 0.9,
                tags: tags.iter().map(|t| t.to_string()).collect(),
                provenance: MemoryProvenance::default(),
                supersedes: None,
            },
            now(),
        )
    };
    let old = make(
        "The build uses cargo with a local CI script",
        repo.clone(),
        &["build"],
    );
    let global = make(
        "User prefers concise commit messages",
        MemoryScope::Global,
        &["git"],
    );
    let other = make(
        "Unrelated repo uses bazel",
        MemoryScope::Repository { id: "other".into() },
        &[],
    );
    store
        .commit(
            Commit::new()
                .mutate(Mutation::PutMemory(old.clone()))
                .mutate(Mutation::PutMemory(global.clone()))
                .mutate(Mutation::PutMemory(other)),
        )
        .expect("commit");
    let visible = store
        .memories(&MemoryQuery {
            scope: Some(repo.clone()),
            ..Default::default()
        })
        .expect("memories");
    assert_eq!(visible.len(), 2, "repo scope sees its own plus global");
    let found = store
        .memories(&MemoryQuery {
            text: Some("cargo build".into()),
            ..Default::default()
        })
        .expect("search");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, old.id);

    let mut new = make(
        "The build uses cargo; CI runs via scripts/ci.sh",
        repo.clone(),
        &["build"],
    );
    new.supersedes = Some(old.id);
    let mut old_updated = old.clone();
    old_updated.superseded_by = Some(new.id);
    store
        .commit(
            Commit::new()
                .mutate(Mutation::PutMemory(new.clone()))
                .mutate(Mutation::PutMemory(old_updated)),
        )
        .expect("commit");
    let current = store
        .memories(&MemoryQuery {
            text: Some("cargo".into()),
            ..Default::default()
        })
        .expect("search");
    assert_eq!(current.len(), 1);
    assert_eq!(current[0].id, new.id);
    let with_old = store
        .memories(&MemoryQuery {
            text: Some("cargo".into()),
            include_superseded: true,
            ..Default::default()
        })
        .expect("search");
    assert_eq!(with_old.len(), 2);
    let tagged = store
        .memories(&MemoryQuery {
            tags: vec!["git".into()],
            ..Default::default()
        })
        .expect("tags");
    assert_eq!(tagged[0].id, global.id);
}

/// The retraction sweep asks for a repository with every role scope in
/// it, and must get no other repository's: not one whose id is a prefix
/// of this one, and not one whose id and role split differently.
#[test]
fn a_repository_query_reaches_its_nested_role_scopes_only_when_asked() {
    use std::collections::BTreeSet;
    let store = SqliteStore::open_in_memory().expect("open");
    let put = |text: &str, scope: MemoryScope| {
        let record = MemoryRecord::from_candidate(
            MemoryCandidate {
                scope,
                kind: MemoryKind::Fact,
                text: text.into(),
                structured: None,
                importance: 0.7,
                confidence: 0.9,
                tags: Vec::new(),
                provenance: MemoryProvenance::default(),
                supersedes: None,
            },
            now(),
        );
        store
            .commit(Commit::new().mutate(Mutation::PutMemory(record)))
            .expect("commit");
    };
    let repo_role = |id: &str, role: &str| MemoryScope::RepositoryRole {
        id: id.into(),
        role: role.into(),
    };
    put(
        "repository wide",
        MemoryScope::Repository { id: "a".into() },
    );
    put("for stewards", repo_role("a", "team.steward"));
    put("for workers", repo_role("a", "team.worker"));
    put("longer id", repo_role("ab", "team.steward"));
    // `a:x` + `y` and `a` + `x:y` must not share a key.
    put("colon in id", repo_role("a:x", "y"));
    put("colon in role", repo_role("a", "x:y"));
    put("global", MemoryScope::Global);

    let texts = |query: MemoryQuery| -> BTreeSet<String> {
        store
            .memories(&query)
            .expect("memories")
            .into_iter()
            .map(|m| m.text)
            .collect()
    };
    let exact = texts(MemoryQuery {
        scope: Some(MemoryScope::Repository { id: "a".into() }),
        exact_scope: true,
        ..Default::default()
    });
    assert_eq!(exact, BTreeSet::from(["repository wide".to_owned()]));
    let nested = texts(MemoryQuery {
        scope: Some(MemoryScope::Repository { id: "a".into() }),
        exact_scope: true,
        nested_scopes: true,
        ..Default::default()
    });
    assert_eq!(
        nested,
        BTreeSet::from(
            [
                "repository wide",
                "for stewards",
                "for workers",
                "colon in role"
            ]
            .map(str::to_owned)
        )
    );
    let one_role = texts(MemoryQuery {
        scope: Some(repo_role("a", "x:y")),
        exact_scope: true,
        ..Default::default()
    });
    assert_eq!(one_role, BTreeSet::from(["colon in role".to_owned()]));
    // Nothing nests in a role scope; the flag changes nothing there.
    let role_nested = texts(MemoryQuery {
        scope: Some(repo_role("a", "team")),
        exact_scope: true,
        nested_scopes: true,
        ..Default::default()
    });
    assert!(role_nested.is_empty(), "{role_nested:?}");
}

#[test]
fn memory_tag_filters_use_exact_case_sensitive_membership() {
    let store = SqliteStore::open_in_memory().unwrap();
    let tag_sets = [
        vec!["gitlab", "ci"],
        vec!["git", "ci"],
        vec!["axb"],
        vec!["a_b"],
        vec!["%"],
        vec!["GIT"],
        vec!["two words"],
        vec!["two", "words"],
        vec!["quote\"", "back\\slash"],
    ];
    let mut ids = Vec::new();
    let mut commit = Commit::new();
    for tags in tag_sets {
        let record = MemoryRecord::from_candidate(
            MemoryCandidate {
                scope: MemoryScope::Global,
                kind: MemoryKind::Fact,
                text: "durable memory".into(),
                structured: None,
                importance: 0.7,
                confidence: 0.9,
                tags: tags.into_iter().map(str::to_owned).collect(),
                provenance: MemoryProvenance::default(),
                supersedes: None,
            },
            now(),
        );
        ids.push(record.id);
        commit.push_mutation(Mutation::PutMemory(record));
    }
    store.commit(commit).unwrap();
    for (tags, expected) in [
        (vec!["git"], vec![ids[1]]),
        (vec!["git", "ci"], vec![ids[1]]),
        (vec!["a_b"], vec![ids[3]]),
        (vec!["%"], vec![ids[4]]),
        (vec!["GIT"], vec![ids[5]]),
        (vec!["two words"], vec![ids[6]]),
        (vec!["quote\"", "back\\slash"], vec![ids[8]]),
        (vec!["missing"], vec![]),
    ] {
        let found = store
            .memories(&MemoryQuery {
                tags: tags.iter().map(|t| t.to_string()).collect(),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(
            found.iter().map(|r| r.id).collect::<Vec<_>>(),
            expected,
            "{tags:?}"
        );
    }
}

#[test]
fn memory_limits_preserve_zero_small_and_large_requests() {
    let store = SqliteStore::open_in_memory().unwrap();
    let mut commit = Commit::new();
    for i in 0..5001 {
        commit.push_mutation(Mutation::PutMemory(MemoryRecord::from_candidate(
            MemoryCandidate {
                scope: MemoryScope::Global,
                kind: MemoryKind::Fact,
                text: format!("memory {i}"),
                structured: None,
                importance: 0.7,
                confidence: 0.9,
                tags: vec![],
                provenance: MemoryProvenance::default(),
                supersedes: None,
            },
            now(),
        )));
    }
    store.commit(commit).unwrap();
    for (limit, expected) in [(0, 0), (2, 2), (5001, 5001), (usize::MAX, 5001)] {
        assert_eq!(
            store
                .memories(&MemoryQuery {
                    limit,
                    ..Default::default()
                })
                .unwrap()
                .len(),
            expected,
            "limit {limit}"
        );
    }
}
