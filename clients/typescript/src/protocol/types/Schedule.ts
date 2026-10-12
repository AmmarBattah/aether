// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  Actor,
  AgentId,
  BusyPolicy,
  MissedRunPolicy,
  Recurrence,
  RunId,
  ScheduleId,
  ScheduleOccurrence,
  ScheduleStatus,
  SpawnSpec,
  status,
} from "../../protocol.ts";

/**
 * Timers enqueue work, never authority: targets keep their scope and budgets;
 * spawned runs get only what an operator could grant via `create_agent`.
 */
export interface Schedule {
  "busy"?: BusyPolicy;
  "created_at": string;
  "deliveries": number;
  "id": ScheduleId;
  "last"?: ScheduleOccurrence | null;
  "last_inbox_seq"?: number | null;
  "last_spawned"?: AgentId | null;
  "max_deliveries"?: number | null;
  "missed": MissedRunPolicy;
  "name": string;
  "owner": Actor;
  "payload": unknown;
  "recurrence": Recurrence;
  "run"?: RunId | null;
  "spawn"?: SpawnSpec | null;
  "status": ScheduleStatus;
  "target"?: AgentId | null;
  "updated_at": string;
}
