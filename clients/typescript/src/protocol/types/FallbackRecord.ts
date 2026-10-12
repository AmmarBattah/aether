// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  ModelErrorClass,
  ModelRef,
} from "../../protocol.ts";

/**
 * The last time a turn gave up on a provider and moved to the next link of
 * its chain.
 */
export interface FallbackRecord {
  "at": string;
  "class"?: ModelErrorClass | null;
  "reason": string;
  "to": ModelRef;
}
