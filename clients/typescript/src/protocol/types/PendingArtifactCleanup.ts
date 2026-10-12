// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  ArtifactId,
} from "../../protocol.ts";

export interface PendingArtifactCleanup {
  "pending_unlinks": Array<ArtifactId>;
  "staged_publications": Array<ArtifactId>;
}
