// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentId,
  ArtifactId,
  TaskId,
} from "../../protocol.ts";

export interface ArtifactLine {
  "agent"?: AgentId | null;
  "created_at": string;
  "evicted"?: boolean;
  "id": ArtifactId;
  "kind": string;
  "name": string;
  "size_bytes": number;
  "summary": string;
  "task"?: TaskId | null;
}
