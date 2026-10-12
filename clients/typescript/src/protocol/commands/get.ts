// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentId,
  ArtifactId,
  CheckpointId,
  EventQuery,
  InvocationId,
  RunId,
  ScheduleId,
  SnapshotId,
  TaskId,
} from "../../protocol.ts";


export interface GetAgentCommand {
  "cmd": "get_agent";
  "id": AgentId;
}

export function getAgent(args: Omit<GetAgentCommand, "cmd">): GetAgentCommand {
  return { cmd: "get_agent", ...args };
}

export interface GetArtifactCommand {
  "cmd": "get_artifact";
  "id": ArtifactId;
}

export function getArtifact(args: Omit<GetArtifactCommand, "cmd">): GetArtifactCommand {
  return { cmd: "get_artifact", ...args };
}

export interface GetCheckpointCommand {
  "cmd": "get_checkpoint";
  "id": CheckpointId;
}

export function getCheckpoint(args: Omit<GetCheckpointCommand, "cmd">): GetCheckpointCommand {
  return { cmd: "get_checkpoint", ...args };
}

export interface GetEventsCommand {
  "cmd": "get_events";
  "after": number;
  "limit": number;
  "query": EventQuery;
}

export function getEvents(args: Omit<GetEventsCommand, "cmd">): GetEventsCommand {
  return { cmd: "get_events", ...args };
}

/**
 * Inspect the opt-in GitHub watcher and its per-repository health.
 */
export interface GetGithubWatcherCommand {
  "cmd": "get_github_watcher";
}

export function getGithubWatcher(): GetGithubWatcherCommand {
  return { cmd: "get_github_watcher" };
}

export interface GetInboxCommand {
  "cmd": "get_inbox";
  "after": number;
  "agent": AgentId;
}

export function getInbox(args: Omit<GetInboxCommand, "cmd">): GetInboxCommand {
  return { cmd: "get_inbox", ...args };
}

export interface GetInvocationCommand {
  "cmd": "get_invocation";
  "id": InvocationId;
}

export function getInvocation(args: Omit<GetInvocationCommand, "cmd">): GetInvocationCommand {
  return { cmd: "get_invocation", ...args };
}

/**
 * Compose a run's report from its events.
 */
export interface GetRunReportCommand {
  "cmd": "get_run_report";
  "run": RunId;
}

export function getRunReport(args: Omit<GetRunReportCommand, "cmd">): GetRunReportCommand {
  return { cmd: "get_run_report", ...args };
}

export interface GetScheduleCommand {
  "cmd": "get_schedule";
  "id": ScheduleId;
}

export function getSchedule(args: Omit<GetScheduleCommand, "cmd">): GetScheduleCommand {
  return { cmd: "get_schedule", ...args };
}

export interface GetSnapshotCommand {
  "cmd": "get_snapshot";
  "id": SnapshotId;
}

export function getSnapshot(args: Omit<GetSnapshotCommand, "cmd">): GetSnapshotCommand {
  return { cmd: "get_snapshot", ...args };
}

export interface GetTaskCommand {
  "cmd": "get_task";
  "id": TaskId;
}

export function getTask(args: Omit<GetTaskCommand, "cmd">): GetTaskCommand {
  return { cmd: "get_task", ...args };
}

export interface GetTranscriptCommand {
  "cmd": "get_transcript";
  "after": number;
  "agent": AgentId;
}

export function getTranscript(args: Omit<GetTranscriptCommand, "cmd">): GetTranscriptCommand {
  return { cmd: "get_transcript", ...args };
}

/**
 * The live graph: every agent in the run (or all runs) with relations.
 */
export interface GetTreeCommand {
  "cmd": "get_tree";
  "include_archived"?: boolean;
  "run"?: RunId | null;
}

export function getTree(args: Omit<GetTreeCommand, "cmd">): GetTreeCommand {
  return { cmd: "get_tree", ...args };
}
