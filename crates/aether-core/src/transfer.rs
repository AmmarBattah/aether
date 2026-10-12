//! The shape of a run export: what one file says, and what moving a run
//! between daemons produced.
//!
//! Types only. `aether-runtime` writes and reads the file; this crate is
//! what both ends and every client agree the file contains.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::agent::Agent;
use crate::artifact::Artifact;
use crate::checkpoint::Checkpoint;
use crate::event::Event;
use crate::id::{ArtifactId, RunId};
use crate::snapshot::{Invocation, InvocationSnapshot};
use crate::task::Task;
use crate::time::Timestamp;

/// The file format. Bumped when an older reader would misread a newer file;
/// a new record kind is additive and does not bump it.
pub const EXPORT_FORMAT: u32 = 2;

/// The first line of an export: what it is and where it came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    /// Always [`EXPORT_FORMAT`]; the field name makes a stray file obvious.
    pub aether_export: u32,
    pub source_instance: String,
    pub source_run: RunId,
    pub exported_at: Timestamp,
    pub binary_version: String,
    pub store_schema: u32,
    /// Events the file carries. The exporter is the one that knows, and it
    /// is what lets an import notice a file that lost records on the way.
    pub events: u64,
}

/// One line of an export after the manifest.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "record", rename_all = "snake_case")]
pub enum Record {
    Agent(Agent),
    Task(Task),
    Schedule(crate::schedule::Schedule),
    ScheduleOccurrence(crate::schedule::ScheduleOccurrence),
    Event(Event),
    Snapshot(InvocationSnapshot),
    Invocation(Invocation),
    Checkpoint(Checkpoint),
    Artifact(Artifact),
    /// An artifact's payload, base64-encoded, on the line after its record.
    ArtifactBytes {
        id: ArtifactId,
        base64: String,
    },
}

/// What an export wrote.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ExportSummary {
    pub run: RunId,
    pub path: PathBuf,
    pub agents: u64,
    pub tasks: u64,
    pub events: u64,
    pub snapshots: u64,
    pub invocations: u64,
    pub checkpoints: u64,
    pub artifacts: u64,
    /// Artifacts whose payload was still on disk and travelled with them.
    pub artifact_payloads: u64,
    pub bytes_written: u64,
}

/// What an import recreated.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ImportSummary {
    /// The run's new id here.
    pub run: RunId,
    pub source_run: RunId,
    pub source_instance: String,
    pub exported_at: Timestamp,
    pub agents: u64,
    pub tasks: u64,
    pub events: u64,
    pub snapshots: u64,
    pub invocations: u64,
    pub checkpoints: u64,
    pub artifacts: u64,
    pub artifact_payloads: u64,
}
