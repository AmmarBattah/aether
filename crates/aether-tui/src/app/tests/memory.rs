//! memory reducer regressions.
use super::*;

/// Restoring a record prints the action asked for. Inferred from the
/// record that comes back it read "unpinned": a restored memory is
/// neither forgotten nor pinned.
#[test]
fn a_restored_memory_says_restored() {
    let (mut app, _) = app_with_agent();
    let record = aether_core::memory::MemoryRecord::from_candidate(
        aether_core::memory::MemoryCandidate {
            scope: aether_core::memory::MemoryScope::Global,
            kind: aether_core::memory::MemoryKind::Fact,
            text: "the staging database is rebuilt on Mondays".into(),
            structured: None,
            importance: 0.5,
            confidence: 0.5,
            tags: vec![],
            provenance: Default::default(),
            supersedes: None,
        },
        aether_core::time::now(),
    );
    for (action, expected) in [
        (MemoryAction::Restored, "restored"),
        (MemoryAction::Forgotten, "forgot"),
        (MemoryAction::Pinned, "pinned"),
        (MemoryAction::Unpinned, "unpinned"),
    ] {
        app.reduce(AppEvent::Reply {
            then: Then::MemoryMutated(action),
            result: Ok(Reply::Memory(record.clone())),
        });
        assert!(
            app.live_flash().is_some_and(|n| n.starts_with(expected)),
            "{action:?}: {:?}",
            app.live_flash()
        );
    }
}
