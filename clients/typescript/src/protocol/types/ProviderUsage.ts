// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  UsageWindow,
} from "../../protocol.ts";

/**
 * Latest normalized account report. The observation time is not a reset time.
 */
export interface ProviderUsage {
  "observed_at": string;
  "windows": Array<UsageWindow>;
}
