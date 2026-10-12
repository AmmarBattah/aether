// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  ErrorTally,
  FallbackRecord,
  ProviderBackoff,
  ProviderUsage,
  QuotaWindow,
  providers,
} from "../../protocol.ts";

/**
 * One provider's scheduling picture. Derived state: the scheduler rebuilds
 * it from the turns in flight and the feedback providers have given.
 */
export interface ProviderHealth {
  "backoff"?: ProviderBackoff | null;
  "last_fallback"?: FallbackRecord | null;
  "limit"?: number | null;
  "provider": string;
  "quota"?: QuotaWindow | null;
  "recent_errors"?: Array<ErrorTally>;
  "running": number;
  "suspended_agents"?: number;
  "usage"?: ProviderUsage | null;
}
