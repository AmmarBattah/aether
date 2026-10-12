// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  Actor,
} from "../../protocol.ts";

/**
 * Why a rule stopped applying. Kept on the record so the log explains a
 * decision that would be made differently today.
 */
export interface Revocation {
  "at": string;
  "by": Actor;
  "reason"?: string | null;
}
