// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  ModelErrorClass,
} from "../../protocol.ts";

/**
 * One normalized error class a provider returned recently, and how often.
 */
export interface ErrorTally {
  "class": ModelErrorClass;
  "count": number;
  "last_at": string;
}
