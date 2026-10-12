use super::{invalid, same_run};
use crate::Result;
use aether_core::{
    event::EventKind,
    id::{ArtifactId, RunId},
    transfer::{EXPORT_FORMAT, Manifest, Record},
};
use aether_store::StoreError;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde::Serialize;
use sha2::Digest as _;
use std::collections::BTreeMap;
use std::fs::File;
use std::path::Path;

use super::reader::BoundedLines;
use aether_core::{
    agent::{Agent, AgentStatus},
    artifact::Artifact,
    event::{Actor, Event, EventDraft},
    id::{AgentId, EventId},
    schedule::{Schedule, ScheduleMode, ScheduleOccurrence, ScheduleStatus},
    task::{Task, TaskStatus},
    transfer::ImportSummary,
};
use aether_store::{ImportedEvent, Mutation};
use std::collections::BTreeSet;
use std::io::BufReader;

/// Every typed id met so far and the fresh one standing for it here.
///
/// The rewrite is whole-string: a field whose whole value is a
/// `prefix_ULID` is a reference and is remapped, while an id inside a
/// sentence is text and is left as it was written. That keeps every
/// structural cross-reference intact without editing what a model said.
#[derive(Debug, Default)]
struct IdMap(BTreeMap<String, String>);

impl IdMap {
    fn fresh_for(&mut self, old: &str, prefix: &str) -> String {
        self.0
            .entry(old.to_owned())
            .or_insert_with(|| format!("{prefix}_{}", ulid::Ulid::generate()))
            .clone()
    }

    fn rewrite(&mut self, value: &mut serde_json::Value) {
        match value {
            serde_json::Value::String(s) => {
                if let Some(prefix) = aether_core::id::prefix_of(s) {
                    let prefix = prefix.to_owned();
                    *s = self.fresh_for(s, &prefix);
                }
            }
            serde_json::Value::Array(items) => {
                for item in items {
                    self.rewrite(item);
                }
            }
            serde_json::Value::Object(map) => {
                for (_, v) in map.iter_mut() {
                    self.rewrite(v);
                }
            }
            _ => {}
        }
    }

    /// Remap every reference in a record and give it back as its own type.
    fn remap<T: Serialize + serde::de::DeserializeOwned>(&mut self, value: &T) -> Result<T> {
        let mut json = serde_json::to_value(value).map_err(StoreError::Serde)?;
        self.rewrite(&mut json);
        Ok(serde_json::from_value(json).map_err(StoreError::Serde)?)
    }
}

/// Recreate a run from an export under fresh ids, with an event saying where
/// it came from. Returns what was written.
///
/// Every row of the run - agents, tasks, snapshots, invocations,
/// checkpoints, artifact records and events - lands in ONE store
/// transaction, so a file that goes wrong on its last line imports nothing
/// at all rather than leaving a partial run that no command can name or
/// remove. That is what the rows are held in memory for, and what
/// [`super::reader::MAX_RECORDS`] bounds.
///
/// Artifact payloads are the exception, and are read in a second pass AFTER
/// that commit: they are files rather than rows, they would put the largest
/// artifact in the run into the same transaction, and a payload written
/// before its record is an orphan no retention sweep can see. Written after,
/// the worst a failure leaves is bytes the record already accounts for,
/// which retention reclaims on its own schedule.
pub fn import_run(runtime: &crate::Runtime, path: &Path) -> Result<ImportSummary> {
    let mut lines = BoundedLines::new(
        BufReader::new(File::open(path).map_err(StoreError::Io)?),
        path,
    );
    let first = lines
        .next_line()?
        .ok_or_else(|| invalid(format!("{} is empty", path.display())))?;
    let manifest: Manifest = serde_json::from_str(&first)
        .map_err(|e| invalid(format!("{} is not an aether export: {e}", path.display())))?;
    if !(1..=EXPORT_FORMAT).contains(&manifest.aether_export) {
        return Err(invalid(format!(
            "{} is export format {}, this build reads {EXPORT_FORMAT}",
            path.display(),
            manifest.aether_export
        )));
    }

    let mut ids = IdMap::default();
    let run: RunId = ids
        .fresh_for(&manifest.source_run.to_string(), RunId::PREFIX)
        .parse()
        .map_err(|e| invalid(format!("minted run id: {e}")))?;
    let mut summary = ImportSummary {
        run,
        source_run: manifest.source_run,
        source_instance: manifest.source_instance.clone(),
        exported_at: manifest.exported_at,
        agents: 0,
        tasks: 0,
        events: 0,
        snapshots: 0,
        invocations: 0,
        checkpoints: 0,
        artifacts: 0,
        artifact_payloads: 0,
    };

    let mut events: Vec<ImportedEvent> = Vec::new();
    let mut mutations: Vec<Mutation> = Vec::new();
    let mut schedules = BTreeMap::new();
    let mut occurrences: Vec<ScheduleOccurrence> = Vec::new();
    // Every agent that arrived, and those the file's own log says ended: an
    // honest export carries a terminal event per agent, and a file that
    // stripped them would otherwise report a run running forever.
    let mut imported_agents: Vec<(AgentId, Option<AgentId>, RunId)> = Vec::new();
    let mut ended: BTreeSet<AgentId> = BTreeSet::new();
    // Payload lines are noted, not decoded: the bytes are read one at a time
    // in the second pass, so no artifact is ever held beside the rows.
    let mut payloads: Vec<(u64, ArtifactId, ArtifactId)> = Vec::new();
    let mut artifact_ids = BTreeSet::new();
    let mut payload_ids = BTreeSet::new();
    let mut line_no = 1u64;

    while let Some(line) = lines.next_line()? {
        line_no += 1;
        if line.trim().is_empty() {
            continue;
        }
        let record: Record = serde_json::from_str(&line)
            .map_err(|e| invalid(format!("{}: unreadable record: {e}", path.display())))?;
        match record {
            Record::Agent(agent) => {
                same_run(path, "agent", agent.run, manifest.source_run)?;
                let mut agent: Agent = ids.remap(&agent)?;
                archive(&mut agent);
                imported_agents.push((agent.id, agent.parent, agent.run));
                mutations.push(Mutation::UpsertAgent(agent));
                summary.agents += 1;
            }
            Record::Task(task) => {
                same_run(path, "task", task.run, manifest.source_run)?;
                let mut task: Task = ids.remap(&task)?;
                // Its agent can never take a turn again, so an unfinished
                // task is not going to finish here. Say so, once.
                if !task.status.is_terminal() {
                    task.status = TaskStatus::Cancelled {
                        reason: Some(format!("arrived {} in an imported run", task.status.kind())),
                    };
                }
                mutations.push(Mutation::UpsertTask(task));
                summary.tasks += 1;
            }
            Record::Schedule(job) => {
                let Some(source) = job.run() else {
                    return Err(invalid("spawn schedules are not part of a run export"));
                };
                same_run(path, "schedule", source, manifest.source_run)?;
                let mut job: Schedule = ids.remap(&job)?;
                job.owner = job.owner.imported();
                if let ScheduleMode::Deliver { last_inbox_seq, .. } = &mut job.mode {
                    *last_inbox_seq = None;
                }
                if schedules.insert(job.id, job).is_some() {
                    return Err(invalid("duplicate schedule record"));
                }
            }
            Record::ScheduleOccurrence(occurrence) => {
                occurrences.push(ids.remap(&occurrence)?);
            }
            Record::Snapshot(snapshot) => {
                mutations.push(Mutation::PutSnapshot(ids.remap(&snapshot)?));
                summary.snapshots += 1;
            }
            Record::Invocation(invocation) => {
                mutations.push(Mutation::PutInvocation(ids.remap(&invocation)?));
                summary.invocations += 1;
            }
            Record::Checkpoint(checkpoint) => {
                mutations.push(Mutation::PutCheckpoint(ids.remap(&checkpoint)?));
                summary.checkpoints += 1;
            }
            Record::Artifact(artifact) => {
                let mut artifact: Artifact = ids.remap(&artifact)?;
                if !artifact_ids.insert(artifact.id) {
                    return Err(invalid(format!(
                        "duplicate artifact record: {}",
                        artifact.id
                    )));
                }
                // The payload, if it travelled, lands at this store's own
                // path; until then the record must not claim to have bytes.
                artifact.path = None;
                mutations.push(Mutation::PutArtifact(artifact));
                summary.artifacts += 1;
            }
            Record::ArtifactBytes {
                id: source_id,
                base64: _,
            } => {
                let id: ArtifactId = ids
                    .fresh_for(&source_id.to_string(), ArtifactId::PREFIX)
                    .parse()
                    .map_err(|e| invalid(format!("artifact id: {e}")))?;
                if !payload_ids.insert(id) {
                    return Err(invalid(format!("duplicate artifact payload: {id}")));
                }
                payloads.push((line_no, source_id, id));
            }
            Record::Event(event) => {
                if let Some(run) = event.run {
                    same_run(path, "event", run, manifest.source_run)?;
                }
                let event: Event = ids.remap(&event)?;
                if let (Some(agent), true) = (
                    event.agent_id,
                    matches!(
                        event.kind,
                        EventKind::AgentCompleted { .. }
                            | EventKind::AgentFailed { .. }
                            | EventKind::AgentCancelled { .. }
                    ),
                ) {
                    ended.insert(agent);
                }
                events.push(ImportedEvent {
                    id: event.id,
                    at: event.at,
                    draft: EventDraft {
                        // Nothing here approved anything: an audit of this
                        // log must not read another machine's decisions as
                        // this operator's.
                        actor: event.actor.imported(),
                        agent_id: event.agent_id,
                        task_id: event.task_id,
                        parent_agent_id: event.parent_agent_id,
                        run: Some(run),
                        causation: event.causation,
                        kind: event.kind,
                    },
                });
                summary.events += 1;
            }
        }
    }
    let agent_ids: BTreeSet<_> = imported_agents.iter().map(|(id, _, _)| *id).collect();
    for job in schedules.values_mut() {
        let Some(target) = job.target().filter(|target| agent_ids.contains(target)) else {
            return Err(invalid(format!(
                "schedule {} targets an agent absent from the import",
                job.id
            )));
        };
        // An import is an archive, never permission to resume scheduled work.
        if matches!(
            job.status,
            ScheduleStatus::Active { .. } | ScheduleStatus::Paused { .. }
        ) {
            job.status = ScheduleStatus::Cancelled {
                at: manifest.exported_at,
            };
            job.updated_at = manifest.exported_at;
            events.push(ImportedEvent {
                id: EventId::new(), at: manifest.exported_at,
                draft: EventDraft::new(Actor::System, EventKind::Custom {
                    name: "schedule.cancelled".into(),
                    payload: serde_json::json!({"schedule": job, "reason": "arrived in an archived run"}),
                }).agent(target).run(run),
            });
        }
        mutations.push(Mutation::UpsertSchedule(job.clone()));
    }
    let mut occurrence_keys = BTreeSet::new();
    for occurrence in occurrences {
        if !schedules.contains_key(&occurrence.schedule) {
            return Err(invalid(format!(
                "occurrence has no schedule record: {}",
                occurrence.schedule
            )));
        }
        if !occurrence_keys.insert((occurrence.schedule, occurrence.due)) {
            return Err(invalid("duplicate schedule occurrence"));
        }
        mutations.push(Mutation::RecordScheduleOccurrence(occurrence));
    }

    for (_, _, id) in &payloads {
        if !artifact_ids.contains(id) {
            return Err(invalid(format!("artifact {id} has no record")));
        }
    }

    // Validate every payload before committing any rows. Legacy redacted exports
    // advertised the raw digest; repair their metadata from the bytes that travelled.
    let mut artifacts: BTreeMap<_, _> = mutations
        .iter_mut()
        .filter_map(|mutation| match mutation {
            Mutation::PutArtifact(artifact) => Some((artifact.id, artifact)),
            _ => None,
        })
        .collect();
    if manifest.aether_export >= 2 {
        for artifact in artifacts.values() {
            if artifact.digest.is_some() && !payload_ids.contains(&artifact.id) {
                return Err(invalid(format!(
                    "artifact {} advertises a digest but has no payload",
                    artifact.id
                )));
            }
        }
    }
    verify_payloads(path, &payloads, &mut artifacts, manifest.aether_export >= 2)?;
    for event in &mut events {
        match &mut event.draft.kind {
            EventKind::ArtifactCreated { artifact } => {
                if let Some(verified) = artifacts.get(&artifact.id) {
                    artifact.size_bytes = verified.size_bytes;
                    artifact.digest = verified.digest.clone();
                }
            }
            EventKind::ArtifactDeduplicated {
                artifact, digest, ..
            } => {
                if let Some(verified) = artifacts.get(artifact) {
                    *digest = verified.digest.clone().unwrap_or_default();
                }
            }
            _ => {}
        }
    }

    drop(artifacts);

    // The header said how many events would follow; a file that lost some on
    // the way is refused rather than imported as a shorter run.
    if summary.events != manifest.events {
        return Err(invalid(format!(
            "{}: the manifest promises {} events and the file carries {}",
            path.display(),
            manifest.events,
            summary.events
        )));
    }

    // A report is a fold of the log, so an agent the file never ended reads
    // as still running with its open loops, forever. Import already forces a
    // non-terminal task to Cancelled; an agent gets the same, said once. An
    // honest export carries these already and nothing is added.
    for (id, parent, agent_run) in &imported_agents {
        if ended.contains(id) {
            continue;
        }
        events.push(ImportedEvent {
            id: EventId::new(),
            at: manifest.exported_at,
            draft: EventDraft::new(
                Actor::System,
                EventKind::AgentCancelled {
                    reason: Some("arrived unfinished in an imported run".into()),
                },
            )
            .agent(*id)
            .parent(*parent)
            .run(*agent_run),
        });
    }

    // First, so the imported run says where it came from before anything it
    // did. The report reads it as provenance, never as part of the body.
    events.insert(
        0,
        ImportedEvent {
            id: EventId::new(),
            at: manifest.exported_at,
            draft: EventDraft::new(
                Actor::System,
                EventKind::RunImported {
                    source_run: manifest.source_run,
                    source_instance: manifest.source_instance.clone(),
                    exported_at: manifest.exported_at,
                    events: summary.events,
                },
            )
            .run(run),
        },
    );
    runtime.apply_imported(events, mutations)?;

    // Past the commit: the run is here whatever happens next, so a failure
    // has to say so or the operator retries and imports it twice.
    summary.artifact_payloads = write_payloads(runtime, path, &payloads).map_err(|e| {
        invalid(format!(
            "run {} was imported as {run}, but its artifact payloads did not all land: {e}. \
             It is here; do not import it again",
            manifest.source_run
        ))
    })?;
    Ok(summary)
}

fn verify_bytes(artifact: &Artifact, bytes: &[u8]) -> Result<()> {
    let digest = hex::encode(sha2::Sha256::digest(bytes));
    if artifact.digest.as_deref() != Some(digest.as_str())
        || artifact.size_bytes != bytes.len() as u64
    {
        return Err(invalid(format!(
            "artifact {} payload does not match advertised size and digest",
            artifact.id
        )));
    }
    Ok(())
}

fn verify_payloads(
    path: &Path,
    payloads: &[(u64, ArtifactId, ArtifactId)],
    artifacts: &mut BTreeMap<ArtifactId, &mut Artifact>,
    authenticated: bool,
) -> Result<()> {
    let mut lines = BoundedLines::new(
        BufReader::new(File::open(path).map_err(StoreError::Io)?),
        path,
    );
    let mut wanted = payloads.iter().peekable();
    let mut line_no = 0;
    while let Some(line) = lines.next_line()? {
        line_no += 1;
        let Some(&&(at, source, id)) = wanted.peek() else {
            break;
        };
        if line_no != at {
            continue;
        }
        let record: Record =
            serde_json::from_str(&line).map_err(|e| invalid(format!("unreadable payload: {e}")))?;
        let Record::ArtifactBytes { id: actual, base64 } = record else {
            return Err(invalid("artifact payload changed during verification"));
        };
        if actual != source {
            return Err(invalid(
                "artifact payload identity changed during verification",
            ));
        }
        let bytes = BASE64
            .decode(base64.as_bytes())
            .map_err(|e| invalid(format!("artifact {id} payload: {e}")))?;
        let artifact = artifacts
            .get_mut(&id)
            .ok_or_else(|| invalid(format!("artifact {id} has no record")))?;
        if authenticated {
            verify_bytes(artifact, &bytes)?;
        } else {
            artifact.size_bytes = bytes.len() as u64;
            artifact.digest = Some(hex::encode(sha2::Sha256::digest(&bytes)));
        }
        wanted.next();
    }
    if wanted.peek().is_some() {
        return Err(invalid("artifact payloads disappeared during verification"));
    }
    Ok(())
}

/// Read each noted payload line and land its bytes, one at a time, pointing
/// the artifact record that is already committed at the file. Returns how
/// many landed.
fn write_payloads(
    runtime: &crate::Runtime,
    path: &Path,
    payloads: &[(u64, ArtifactId, ArtifactId)],
) -> Result<u64> {
    if payloads.is_empty() {
        return Ok(0);
    }
    let mut lines = BoundedLines::new(
        BufReader::new(File::open(path).map_err(StoreError::Io)?),
        path,
    );
    let mut line_no = 0u64;
    let mut wanted = payloads.iter().peekable();
    let mut written = 0u64;
    while let Some(line) = lines.next_line()? {
        line_no += 1;
        let Some(&&(at, expected_source, id)) = wanted.peek() else {
            break;
        };
        if line_no != at {
            continue;
        }
        wanted.next();
        let Record::ArtifactBytes {
            id: source_id,
            base64,
        } = serde_json::from_str(&line)
            .map_err(|e| invalid(format!("{}: unreadable record: {e}", path.display())))?
        else {
            return Err(invalid(format!(
                "{}: line {at} is no longer an artifact payload",
                path.display()
            )));
        };
        // The file can change between the validation pass and this read.
        // Refuse bytes that no longer belong to the committed artifact.
        if source_id != expected_source {
            return Err(invalid(format!(
                "{}: line {at} changed artifact identity",
                path.display()
            )));
        }
        let bytes = BASE64
            .decode(base64.as_bytes())
            .map_err(|e| invalid(format!("artifact {id} payload: {e}")))?;
        let mut artifact = runtime
            .store
            .artifact(id)?
            .ok_or_else(|| invalid(format!("artifact {id} has no record")))?;
        verify_bytes(&artifact, &bytes)?;
        let publication = runtime.store.stage_artifact_bytes(id, &bytes)?;
        artifact.path = Some(publication.path.clone());
        if let Err(error) =
            runtime.apply_imported(Vec::new(), vec![Mutation::PutArtifact(artifact)])
        {
            if let Err(cleanup) = runtime.store.remove_unreferenced_artifact_bytes(id) {
                return Err(invalid(format!(
                    "{error}; cleaning unpublished artifact {id} also failed: {cleanup}"
                )));
            }
            return Err(error);
        }
        if let Err(error) = runtime.store.finish_artifact_publication(&publication) {
            tracing::warn!(artifact = %id, %error, "artifact imported; publication deferred to recovery");
        }
        written += 1;
    }
    if wanted.peek().is_some() {
        return Err(invalid(format!(
            "{}: artifact payloads disappeared during import",
            path.display()
        )));
    }
    Ok(written)
}

/// Make an agent read as history: terminal by construction, so no command
/// and no scheduler pass can give it a turn.
///
/// The scope, instructions and approval policy in the file are the record of
/// what ran elsewhere, and nothing local acts on them once the status is
/// [`AgentStatus::Imported`] - `resume`, `restart`, `send` and `set` all
/// refuse a terminal agent, the scheduler only reads live ones, and recovery
/// only classifies `Running`. `paused` stays set as a second layer.
fn archive(agent: &mut Agent) {
    if !matches!(agent.status, AgentStatus::Imported { .. }) {
        agent.status_since = None;
    }
    agent.status = AgentStatus::Imported {
        was: agent.status.kind(),
    };
    agent.paused = true;
    // Names a live engine conversation on the exporting machine, and the
    // counter reading billing measures it against.
    agent.engine_session = None;
    agent.engine_usage = None;
    agent.engine_sent = None;
}
