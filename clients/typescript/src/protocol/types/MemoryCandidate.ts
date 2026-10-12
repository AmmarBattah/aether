// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  MemoryId,
  MemoryKind,
  MemoryProvenance,
  MemoryScope,
} from "../../protocol.ts";

/**
 * A proposed memory. Admission decides whether it becomes a record.
 */
export interface MemoryCandidate {
  "confidence"?: number;
  "importance"?: number;
  "kind": MemoryKind;
  "provenance"?: MemoryProvenance;
  "scope": MemoryScope;
  "structured"?: unknown;
  "supersedes"?: MemoryId | null;
  "tags"?: Array<string>;
  "text": string;
}
