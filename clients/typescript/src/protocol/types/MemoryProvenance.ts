// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  Actor,
  AgentId,
  ArtifactId,
  EventId,
  MemorySource,
  TaskId,
} from "../../protocol.ts";

export interface MemoryProvenance {
  "actor"?: Actor | null;
  "agent"?: AgentId | null;
  "artifact"?: ArtifactId | null;
  "event"?: EventId | null;
  "source"?: MemorySource | null;
  "task"?: TaskId | null;
}
