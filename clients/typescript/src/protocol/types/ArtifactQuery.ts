// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentId,
  TaskId,
} from "../../protocol.ts";

export interface ArtifactQuery {
  "agent"?: AgentId | null;
  "limit"?: number;
  "name"?: string | null;
  "task"?: TaskId | null;
  "with_bytes_only"?: boolean;
}
