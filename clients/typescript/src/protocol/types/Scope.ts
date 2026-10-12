// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  ExecutionBackend,
  NetworkPolicy,
} from "../../protocol.ts";

/**
 * Execution scope: where an agent may act. A first-class runtime attribute,
 * not prose in a prompt.
 */
export interface Scope {
  "allowed_paths"?: Array<string>;
  "backend"?: ExecutionBackend;
  "labels"?: Record<string, string>;
  "network"?: NetworkPolicy;
  "repository"?: string | null;
  "workspace_root"?: string | null;
}
