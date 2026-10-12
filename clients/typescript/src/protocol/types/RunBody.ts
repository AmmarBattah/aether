// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentLine,
  ApprovalLine,
  ArtifactLine,
  FailureLine,
  Milestone,
  OpenLoop,
  RunOutcome,
  RunReport,
  TaskLine,
  ToolLine,
  Usage,
} from "../../protocol.ts";

/**
 * Everything derived from the run's own work. Carries no local sequence
 * numbers, so two stores holding the same run produce equal bodies once
 * their ids are put side by side ([`RunReport::canonical_json`]).
 */
export interface RunBody {
  "agents": Array<AgentLine>;
  "approvals": Array<ApprovalLine>;
  "artifacts": Array<ArtifactLine>;
  "duration_ms": number;
  "ended_at"?: string | null;
  "events": number;
  "failures": Array<FailureLine>;
  "objective"?: string | null;
  "open_loops": Array<OpenLoop>;
  "outcome": RunOutcome;
  "started_at": string;
  "tasks": Array<TaskLine>;
  "timeline": Array<Milestone>;
  "tools": Array<ToolLine>;
  "usage": Usage;
}
