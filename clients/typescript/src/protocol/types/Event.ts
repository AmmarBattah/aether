// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  Actor,
  AgentId,
  EventId,
  EventKind,
  RunId,
  TaskId,
} from "../../protocol.ts";

/**
 * A committed event.
 */
export interface Event {
  "actor": Actor;
  "agent_id"?: AgentId | null;
  "at": string;
  "causation"?: EventId | null;
  "id": EventId;
  "kind": EventKind;
  "parent_agent_id"?: AgentId | null;
  "run"?: RunId | null;
  "seq": number;
  "task_id"?: TaskId | null;
}
