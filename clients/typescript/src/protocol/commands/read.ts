// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  ArtifactId,
} from "../../protocol.ts";

/**
 * Bytes, base64-encoded, capped at `max_bytes`.
 */
export interface ReadArtifactCommand {
  "cmd": "read_artifact";
  "id": ArtifactId;
  "max_bytes": number;
}

export function readArtifact(args: Omit<ReadArtifactCommand, "cmd">): ReadArtifactCommand {
  return { cmd: "read_artifact", ...args };
}
