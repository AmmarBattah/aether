// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  ScheduleId,
  ScheduleOutcome,
} from "../../protocol.ts";

/**
 * History describes delivery or spawning, never claims that a model
 * completed the requested work. The ids link to the execution trace.
 */
export interface ScheduleOccurrence {
  "due": string;
  "outcome": ScheduleOutcome;
  "recorded_at": string;
  "schedule": ScheduleId;
}
