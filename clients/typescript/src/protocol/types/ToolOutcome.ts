// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  ArtifactId,
} from "../../protocol.ts";

/**
 * What a tool produced.
 */
export interface ToolOutcome {
  "artifacts"?: Array<ArtifactId>;
  "content": string;
  "duration_ms"?: number;
  "is_error"?: boolean;
  "side_effect_occurred"?: boolean | null;
  "structured"?: unknown;
  "truncated"?: boolean;
}
