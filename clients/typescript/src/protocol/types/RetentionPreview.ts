// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  ArtifactId,
  PendingArtifactCleanup,
  RunRetentionPreview,
} from "../../protocol.ts";

export interface RetentionPreview {
  "artifact_payload_bytes": number;
  "at": string;
  "eligible_artifacts": Array<ArtifactId>;
  "history_before": string;
  "history_payload_bytes": number;
  "pending_cleanup": PendingArtifactCleanup;
  "runs": Array<RunRetentionPreview>;
}
