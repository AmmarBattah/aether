// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  Artifact,
  ArtifactId,
} from "../../protocol.ts";


export interface EventKindArtifactCreated {
  "artifact": Artifact;
  "type": "artifact.created";
}

/**
 * An identical artifact already existed, so the bytes were not stored
 * again and the call was given the existing one.
 */
export interface EventKindArtifactDeduplicated {
  "artifact": ArtifactId;
  "digest": string;
  "shared_with"?: ArtifactId | null;
  "type": "artifact.deduplicated";
}

/**
 * Retention dropped the bytes. The record stays; the payload is gone.
 */
export interface EventKindArtifactEvicted {
  "artifact": ArtifactId;
  "reason": string;
  "size_bytes": number;
  "type": "artifact.evicted";
}
