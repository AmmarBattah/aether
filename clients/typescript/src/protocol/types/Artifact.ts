// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  Actor,
  AgentId,
  ArtifactId,
  ArtifactKind,
  TaskId,
} from "../../protocol.ts";

export interface Artifact {
  "agent_id"?: AgentId | null;
  "content_type"?: string | null;
  "created_at": string;
  "created_by": Actor;
  "digest"?: string | null;
  "evicted_at"?: string | null;
  "id": ArtifactId;
  "kind": ArtifactKind;
  "name": string;
  "path"?: string | null;
  "size_bytes": number;
  "summary": string;
  "task_id"?: TaskId | null;
}
