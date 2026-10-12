use super::*;

#[test]
fn a_bullet_is_a_fact_and_its_kind_and_tags_are_read() {
    let notes = parse(
        r#"# Build

- **[convention]** Run scripts/ci.sh, never the steps by hand. #build #ci
- **[invariant]** Every state change commits with its event.
- A plain bullet is a fact.
- **[nonsense]** An unknown kind stays a fact and keeps its text.
  - an indented sub-point is prose

Prose is prose.
"#,
    );
    assert_eq!(notes.len(), 4);
    assert_eq!(notes[0].kind, MemoryKind::Preference);
    assert_eq!(notes[0].text, "Run scripts/ci.sh, never the steps by hand.");
    assert_eq!(notes[0].tags, vec!["build", "ci"]);
    assert_eq!(notes[1].kind, MemoryKind::Constraint);
    assert_eq!(notes[2].kind, MemoryKind::Fact);
    assert_eq!(notes[2].text, "A plain bullet is a fact.");
    assert_eq!(notes[3].kind, MemoryKind::Fact);
    assert!(notes[3].text.starts_with("**[nonsense]**"));
}

/// A note's label is a promise that checking out that commit shows the
/// line. It does not hold for a file that is untracked, or tracked and
/// edited since: the last commit that touched it says something else.
#[test]
fn an_uncommitted_note_is_stamped_with_no_commit_at_all() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    let notes = root.join(KNOWLEDGE_DIR);
    std::fs::create_dir_all(&notes).expect("mkdir");
    std::fs::write(
        notes.join("committed.md"),
        "- The whole build runs through scripts/ci.sh\n",
    )
    .expect("write");
    for args in [
        vec!["init", "-q"],
        vec!["config", "user.email", "f@example.com"],
        vec!["config", "user.name", "fixture"],
        vec!["add", "-A"],
        vec!["commit", "-q", "-m", "notes"],
    ] {
        test_git(root, &args);
    }
    // Tracked and edited since the commit that last touched it.
    std::fs::write(
        notes.join("committed.md"),
        "- The whole build runs through scripts/ci.sh\n\
         - Deploys need a signed tag on the release branch\n",
    )
    .expect("write");
    // Never committed at all.
    std::fs::write(
        notes.join("untracked.md"),
        "- Migrations are applied by the deploy job, never by hand\n",
    )
    .expect("write");

    let notes = scan(root, "r").expect("scan");
    for note in &notes {
        let source = note.provenance.source.as_ref().expect("a file");
        assert!(
            source.commit.is_none(),
            "{} claims commit {:?}, which does not contain it",
            source.file,
            source.commit
        );
        assert_eq!(source.label(), source.file);
    }

    // Committed and untouched: the label is checkable, so it is stamped.
    test_git(root, &["add", "-A"]);
    test_git(root, &["commit", "-q", "-m", "the rest"]);
    for note in scan(root, "r").expect("scan") {
        let commit = note
            .provenance
            .source
            .as_ref()
            .and_then(|s| s.commit.as_deref())
            .expect("a committed, unedited note names its commit");
        assert_eq!(commit.len(), 40, "{commit}");
    }
}

/// `is_file` and `read_to_string` follow links and git commits them, so a
/// link is a way to read a file from outside the tree into the prompt and
/// write the host path into its provenance.
#[test]
fn a_symlinked_note_is_refused_rather_than_followed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    std::fs::create_dir_all(root.join(KNOWLEDGE_DIR)).expect("mkdir");
    std::fs::write(root.join("outside.md"), "- A secret from outside\n").expect("write");
    std::os::unix::fs::symlink(
        root.join("outside.md"),
        root.join(KNOWLEDGE_DIR).join("linked.md"),
    )
    .expect("symlink");

    let err = scan(root, "r").expect_err("refused");
    assert!(matches!(err, RepoKnowledgeError::Symlink { .. }), "{err:?}");
}

/// `symlink_metadata` does not follow the last component but does follow
/// every one before it, and git commits a symlinked directory as happily
/// as a symlinked file: `.aether -> /elsewhere` reports a real directory
/// holding real files, so a leaf-only check reads them into the prompt.
#[test]
fn a_symlinked_knowledge_parent_is_refused_rather_than_followed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let outside = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(outside.path().join("knowledge")).expect("mkdir");
    std::fs::write(
        outside.path().join("knowledge").join("notes.md"),
        "- A secret from outside the tree\n",
    )
    .expect("write");

    let root = dir.path();
    std::os::unix::fs::symlink(outside.path(), root.join(".aether")).expect("symlink");
    // The shape the leaf-only check passed: a real directory, real files.
    assert!(root.join(KNOWLEDGE_DIR).is_dir());
    assert!(
        !std::fs::symlink_metadata(root.join(KNOWLEDGE_DIR))
            .expect("lstat")
            .is_symlink()
    );

    let err = scan(root, "r").expect_err("refused");
    assert!(matches!(err, RepoKnowledgeError::Symlink { .. }), "{err:?}");
}

/// Anyone who can land a branch can write these files and what they say
/// reaches the system prompt, so the surface has a stated size.
#[test]
fn an_oversized_note_set_is_refused_with_the_cap_named() {
    let dir = tempfile::tempdir().expect("tempdir");
    let notes = dir.path().join(KNOWLEDGE_DIR);
    std::fs::create_dir_all(&notes).expect("mkdir");
    let line = "- ".to_owned() + &"a".repeat(200) + "\n";
    std::fs::write(notes.join("huge.md"), line.repeat(1_000)).expect("write");
    let err = scan(dir.path(), "r").expect_err("refused");
    assert!(matches!(err, RepoKnowledgeError::TooMuch(_)), "{err:?}");
    assert!(err.to_string().contains("capped at"), "{err}");

    // Too many notes, each one small.
    std::fs::remove_file(notes.join("huge.md")).expect("rm");
    let mut many = String::new();
    for i in 0..(MAX_NOTES + 10) {
        many.push_str(&format!("- Observation number {i} about this repository\n"));
    }
    std::fs::write(notes.join("many.md"), many).expect("write");
    let err = scan(dir.path(), "r").expect_err("refused");
    assert!(err.to_string().contains("notes under"), "{err}");
}

fn test_git(root: &Path, args: &[&str]) {
    let out = Command::new("git")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("git is on this machine");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn a_tree_with_no_notes_yields_none() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert!(!has_knowledge(dir.path()));
    assert!(scan(dir.path(), "r").expect("scan").is_empty());
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(path, text).expect("write");
}

/// A repository files notes for one level of the agent hierarchy by
/// putting them in a directory named for that role family; the notes at
/// the top stay the whole repository's.
#[test]
fn a_role_directory_scopes_its_notes_to_that_role_within_the_repository() {
    let dir = tempfile::tempdir().expect("tempdir");
    let notes = dir.path().join(KNOWLEDGE_DIR);
    write(
        &notes.join("build.md"),
        "- The build runs through scripts/ci.sh\n",
    );
    write(
        &notes.join("team.steward").join("merge.md"),
        "- Stewards merge only after hosted checks pass\n",
    );
    write(
        &notes.join("team").join("all.md"),
        "- Every team agent reports through its steward\n",
    );
    write(&notes.join("team").join("draft.txt"), "- not markdown\n");

    let found = scan(dir.path(), "github.com/o/r").expect("scan");
    let by_text = |text: &str| {
        found
            .iter()
            .find(|c| c.text == text)
            .unwrap_or_else(|| panic!("{text} was not read: {found:?}"))
    };
    assert_eq!(found.len(), 3, "{found:?}");
    assert_eq!(
        by_text("The build runs through scripts/ci.sh").scope,
        MemoryScope::Repository {
            id: "github.com/o/r".into()
        }
    );
    let steward = by_text("Stewards merge only after hosted checks pass");
    assert_eq!(
        steward.scope,
        MemoryScope::RepositoryRole {
            id: "github.com/o/r".into(),
            role: "team.steward".into()
        }
    );
    assert_eq!(
        steward.provenance.source.as_ref().map(|s| s.file.as_str()),
        Some(".aether/knowledge/team.steward/merge.md")
    );
    assert_eq!(
        by_text("Every team agent reports through its steward").scope,
        MemoryScope::RepositoryRole {
            id: "github.com/o/r".into(),
            role: "team".into()
        }
    );
}

/// The dotted name carries the hierarchy, so a directory inside a role
/// directory has no meaning; its notes would silently reach nobody.
#[test]
fn a_directory_inside_a_role_directory_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let notes = dir.path().join(KNOWLEDGE_DIR);
    write(
        &notes.join("build.md"),
        "- The build runs through scripts/ci.sh\n",
    );
    write(
        &notes.join("team").join("steward").join("merge.md"),
        "- Stewards merge only after hosted checks pass\n",
    );
    let err = scan(dir.path(), "r").expect_err("refused");
    assert!(matches!(err, RepoKnowledgeError::Layout { .. }), "{err:?}");
    assert!(err.to_string().contains("team.<name>"), "{err}");
}

/// The name becomes a scope key, a listing label and a word in the
/// prompt, and a name no role can have would file notes for nobody.
#[test]
fn a_role_directory_with_a_name_no_role_family_has_is_refused() {
    for name in ["has space", "a..b", "émoji", "trailing."] {
        let dir = tempfile::tempdir().expect("tempdir");
        write(
            &dir.path().join(KNOWLEDGE_DIR).join(name).join("n.md"),
            "- A note filed under a bad name\n",
        );
        let err = scan(dir.path(), "r").expect_err(name);
        assert!(
            matches!(err, RepoKnowledgeError::Layout { .. }),
            "{name}: {err:?}"
        );
    }
}

/// An editor's hidden directory (`.obsidian/`) is not a role and was never
/// read; it must not stop the notes beside it from seeding.
#[test]
fn a_hidden_directory_is_skipped_not_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let notes = dir.path().join(KNOWLEDGE_DIR);
    write(
        &notes.join("build.md"),
        "- The build runs through scripts/ci.sh\n",
    );
    write(
        &notes.join(".obsidian").join("nested").join("workspace.md"),
        "- Editor state, not a note\n",
    );
    write(
        &notes.join("team").join("all.md"),
        "- Every team agent reports through its steward\n",
    );
    // Inside a role directory too: skipped, not refused as nested.
    write(
        &notes.join("team").join(".drafts").join("wip.md"),
        "- Not yet a note\n",
    );
    // Nor do hidden directories count toward the role directory cap.
    for i in 1..MAX_ROLE_DIRS {
        std::fs::create_dir_all(notes.join(format!("role{i}"))).expect("mkdir");
    }
    for i in 0..MAX_ROLE_DIRS {
        std::fs::create_dir_all(notes.join(format!(".hidden{i}"))).expect("mkdir");
    }
    let found = scan(dir.path(), "r").expect("scan");
    let texts: Vec<&str> = found.iter().map(|c| c.text.as_str()).collect();
    assert_eq!(
        texts,
        vec![
            "The build runs through scripts/ci.sh",
            "Every team agent reports through its steward"
        ]
    );
}

/// A role directory is a directory read into the prompt: a link named
/// like one would read a directory from outside the tree, and a link
/// inside one is as dangerous as a link at the top.
#[test]
fn a_symlink_in_or_as_a_role_directory_is_refused_rather_than_followed() {
    let outside = tempfile::tempdir().expect("tempdir");
    write(
        &outside.path().join("notes.md"),
        "- A secret from outside the tree\n",
    );

    let dir = tempfile::tempdir().expect("tempdir");
    let notes = dir.path().join(KNOWLEDGE_DIR);
    std::fs::create_dir_all(&notes).expect("mkdir");
    std::os::unix::fs::symlink(outside.path(), notes.join("team")).expect("symlink");
    let err = scan(dir.path(), "r").expect_err("refused");
    assert!(matches!(err, RepoKnowledgeError::Symlink { .. }), "{err:?}");

    // Hidden or not markdown, a link is still refused rather than skipped.
    for name in [".hidden", "notes.txt"] {
        let dir = tempfile::tempdir().expect("tempdir");
        let notes = dir.path().join(KNOWLEDGE_DIR);
        std::fs::create_dir_all(&notes).expect("mkdir");
        std::os::unix::fs::symlink(outside.path(), notes.join(name)).expect("symlink");
        let err = scan(dir.path(), "r").expect_err(name);
        assert!(
            matches!(err, RepoKnowledgeError::Symlink { .. }),
            "{name}: {err:?}"
        );
    }

    let dir = tempfile::tempdir().expect("tempdir");
    let role = dir.path().join(KNOWLEDGE_DIR).join("team");
    std::fs::create_dir_all(&role).expect("mkdir");
    std::os::unix::fs::symlink(outside.path().join("notes.md"), role.join("linked.md"))
        .expect("symlink");
    let err = scan(dir.path(), "r").expect_err("refused");
    assert!(matches!(err, RepoKnowledgeError::Symlink { .. }), "{err:?}");
}

/// The caps bound what reaches the prompt, so they count the whole tree:
/// splitting a note set across role directories must not multiply them.
#[test]
fn the_caps_are_totals_across_role_directories() {
    let refused = |fill: &dyn Fn(&Path)| {
        let dir = tempfile::tempdir().expect("tempdir");
        let notes = dir.path().join(KNOWLEDGE_DIR);
        std::fs::create_dir_all(&notes).expect("mkdir");
        fill(&notes);
        scan(dir.path(), "r").expect_err("over a cap")
    };

    // Files: under the cap in each directory, over it together.
    let err = refused(&|notes| {
        for i in 0..=MAX_FILES / 2 {
            write(&notes.join(format!("{i}.md")), "- A note at the top\n");
            write(
                &notes.join("team").join(format!("{i}.md")),
                "- A note for team\n",
            );
        }
    });
    assert!(err.to_string().contains("note files"), "{err}");

    // Checked per role directory: the scan stops before listing the next.
    let err = refused(&|notes| {
        for i in 0..=MAX_FILES {
            write(&notes.join("team").join(format!("{i}.md")), "- A note\n");
        }
        std::fs::create_dir_all(notes.join("zz not a role")).expect("mkdir");
    });
    assert!(matches!(err, RepoKnowledgeError::TooMuch(_)), "{err:?}");

    // Notes: the same.
    let err = refused(&|notes| {
        let many = |who: &str| {
            (0..=MAX_NOTES / 2)
                .map(|i| format!("- Observation {i} for {who} about this repository\n"))
                .collect::<String>()
        };
        write(&notes.join("top.md"), &many("everyone"));
        write(&notes.join("team").join("role.md"), &many("team"));
    });
    assert!(err.to_string().contains("notes under"), "{err}");

    // Bytes: every file under the per-file cap, the tree over the total.
    let err = refused(&|notes| {
        let file = "- ".to_owned() + &"b".repeat(MAX_FILE_BYTES as usize - 16) + "\n";
        let per_dir = (MAX_TOTAL_BYTES / MAX_FILE_BYTES) as usize / 2 + 1;
        for i in 0..per_dir {
            write(&notes.join(format!("{i}.md")), &file);
            write(&notes.join("team").join(format!("{i}.md")), &file);
        }
    });
    assert!(err.to_string().contains("bytes"), "{err}");

    // Role directories, each empty: every one costs a listing.
    let err = refused(&|notes| {
        for i in 0..=MAX_ROLE_DIRS {
            std::fs::create_dir_all(notes.join(format!("role{i}"))).expect("mkdir");
        }
    });
    assert!(err.to_string().contains("role directories"), "{err}");
}

#[test]
fn a_note_swapped_to_a_symlink_after_listing_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let notes = root.join(KNOWLEDGE_DIR);
    let path = notes.join("notes.md");
    write(&path, "- A safe repository convention\n");
    let listed = entries(&notes).unwrap();
    assert!(matches!(&listed[..], [Entry::Note(p)] if p == &path));
    let outside = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(
        outside.path(),
        "- Outside content must never become memory\n",
    )
    .unwrap();
    std::fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink(outside.path(), &path).unwrap();
    assert!(read_note(&root, &path).is_err());
}

#[test]
fn a_note_parent_swapped_to_a_symlink_after_listing_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let notes = root.join(KNOWLEDGE_DIR);
    let path = notes.join("worker/notes.md");
    write(&path, "- A safe repository convention\n");
    assert!(
        matches!(&entries(path.parent().unwrap()).unwrap()[..], [Entry::Note(p)] if p == &path)
    );
    let outside = tempfile::tempdir().unwrap();
    write(
        &outside.path().join("notes.md"),
        "- Outside content must never become memory\n",
    );
    std::fs::rename(notes.join("worker"), notes.join("original")).unwrap();
    std::os::unix::fs::symlink(outside.path(), notes.join("worker")).unwrap();
    assert!(read_note(&root, &path).is_err());
}

#[test]
fn a_note_that_grows_after_listing_is_bounded_at_read_time() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let notes = root.join(KNOWLEDGE_DIR);
    let path = notes.join("notes.md");
    write(&path, "- A safe repository convention\n");
    assert!(matches!(&entries(&notes).unwrap()[..], [Entry::Note(p)] if p == &path));
    std::fs::write(&path, "a".repeat(MAX_FILE_BYTES as usize + 1)).unwrap();
    assert!(matches!(
        read_note(&root, &path),
        Err(RepoKnowledgeError::TooMuch(_))
    ));
}

#[test]
fn a_note_swapped_to_a_fifo_after_listing_is_refused_without_blocking() {
    use rustix::fs::{CWD, Mode, mkfifoat};
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let notes = root.join(KNOWLEDGE_DIR);
    let path = notes.join("notes.md");
    write(&path, "- A safe repository convention\n");
    assert!(matches!(&entries(&notes).unwrap()[..], [Entry::Note(p)] if p == &path));
    std::fs::remove_file(&path).unwrap();
    mkfifoat(CWD, &path, Mode::RUSR | Mode::WUSR).unwrap();
    assert!(matches!(
        read_note(&root, &path),
        Err(RepoKnowledgeError::Layout { .. })
    ));
}
