use crate::Runtime;
use aether_core::agent::Agent;
use aether_core::event::Actor;

impl Runtime {
    /// §6.4: the notes a repository carries in its own tree become memory the
    /// first time an agent works there.
    ///
    /// Off the create path: reading the tree is filesystem work plus a git
    /// subprocess per file, and creating an agent must not wait for a
    /// repository's notes - nor fail because they cannot be read. Seeding is
    /// idempotent and takes the write path per note, so landing a moment
    /// later, or twice at once, changes nothing.
    ///
    /// Bounded at most once per agent id per process (not "already holds its
    /// notes" - that still costs the tree walk and the memory gate to find
    /// out): a client retry-storming a keyed root `create_agent` names the
    /// same agent id on every replay, so a process-local set turns every
    /// retry after the first into a hash lookup instead of a `read_dir` and a
    /// `WriteGate` acquisition per note.
    pub(crate) fn seed_repository_knowledge(&self, agent: &Agent, by: Actor) {
        // Root agents only. A child inherits its parent's repository id, so
        // the notes are already in by the time it exists, and a supervisor
        // fanning out twenty workers would otherwise rescan the tree twenty
        // times for nothing.
        if agent.parent.is_some() {
            return;
        }
        let (Some(root), Some(id)) = (&agent.scope.workspace_root, &agent.scope.repository) else {
            return;
        };
        if !self.seeded_repositories.lock().insert(agent.id) {
            return;
        }
        let (memory, root, id) = (self.memory.clone(), root.clone(), id.clone());
        let seed = move || {
            if !aether_memory::repo::has_knowledge(&root) {
                return;
            }
            match memory.seed_repository(&root, &id, by) {
                Ok(report) if report.admitted + report.superseded + report.retracted > 0 => {
                    tracing::info!(repository = %id, admitted = report.admitted, superseded = report.superseded, retracted = report.retracted, "seeded repository knowledge");
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!(repository = %id, error = %e, "could not read this repository's committed notes")
                }
            }
        };
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                handle.spawn_blocking(seed);
            }
            // No runtime to hand it to, so it is this thread's work after all.
            Err(_) => seed(),
        }
    }
}

/// A stable name for the repository at `root`: its `origin` remote when it has
/// one, so every clone of it shares one memory scope, else the canonical path.
///
/// The remote is normalized to host and path first. A remote can carry a
/// token in its userinfo, and this id reaches the system prompt, the event
/// log and every memory row - none of which can be retracted.
pub(crate) fn repository_id(root: &std::path::Path) -> Option<String> {
    let remote = aether_memory::repo::git(root, &["remote", "get-url", "origin"])
        .as_deref()
        .and_then(aether_core::repository::normalize_remote);
    remote.or_else(|| {
        root.canonicalize()
            .ok()
            .and_then(|p| aether_core::repository::normalize_id(&p.to_string_lossy()))
    })
}
