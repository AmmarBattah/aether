// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  TaskId,
} from "../../protocol.ts";

/**
 * Publish UTF-8 evidence associated with a durable task. The daemon owns
 * blob storage and derives the agent/run association from that task.
 */
export interface PublishArtifact {
  "content_type": string;
  "name": string;
  "summary": string;
  "task": TaskId;
  "text": string;
}
