// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentId,
  BusyPolicy,
  MissedRunPolicy,
  Recurrence,
  SpawnSpec,
} from "../../protocol.ts";

/**
 * On the wire `target` and `spawn` are sibling fields, exactly one set, so
 * a deliver request keeps the shape older clients send.
 */
export interface CreateScheduleSpec {
  "busy"?: BusyPolicy;
  "max_deliveries"?: number | null;
  "missed"?: MissedRunPolicy;
  "name": string;
  "payload"?: unknown;
  "recurrence": Recurrence;
  "spawn"?: SpawnSpec | null;
  "target"?: AgentId | null;
}
