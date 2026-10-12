//! memory support scenarios and fixtures.
use super::*;

// ----- VISION §6.4: repository knowledge ----------------------------------

/// Git in a scratch tree, deaf to the machine it runs on: signing, hooks or a
/// default branch from the developer's own config would fail these tests for
/// reasons unrelated to the code.
pub(super) fn git_in(root: &std::path::Path, args: &[&str]) {
    let out = std::process::Command::new("git")
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
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Seeding runs off the create path, so a test that needs the notes in place
/// waits for them rather than assuming `create_agent` did the reading.
pub(super) async fn wait_for_seeding(h: &Harness, at_least: usize) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let seeded = h
            .store
            .memories(&aether_store::MemoryQuery {
                limit: 100,
                ..Default::default()
            })
            .expect("memories")
            .into_iter()
            .filter(|m| m.provenance.source.is_some())
            .count();
        if seeded >= at_least {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "the repository's notes were never seeded"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
