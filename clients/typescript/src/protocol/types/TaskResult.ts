// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  ArtifactId,
} from "../../protocol.ts";

/**
 * The durable outcome of a task. Cheap to carry in context forever.
 */
export interface TaskResult {
  "artifacts"?: Array<ArtifactId>;
  "structured"?: unknown;
  "summary": string;
}
