// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  TaskId,
} from "../../protocol.ts";


export interface PublishArtifactCommand {
  "cmd": "publish_artifact";
  "content_type": string;
  "name": string;
  "summary": string;
  "task": TaskId;
  "text": string;
}

export function publishArtifact(args: Omit<PublishArtifactCommand, "cmd">): PublishArtifactCommand {
  return { cmd: "publish_artifact", ...args };
}
