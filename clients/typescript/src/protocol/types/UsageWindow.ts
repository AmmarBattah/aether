// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  QuotaWindow,
} from "../../protocol.ts";

/**
 * One named account limit, including limits outside the preferred scheduling window.
 */
export interface UsageWindow {
  "name": string;
  "quota": QuotaWindow;
}
