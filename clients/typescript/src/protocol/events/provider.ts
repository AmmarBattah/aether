// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  ModelErrorClass,
  ModelRef,
  ProviderUsage,
  QuotaWindow,
} from "../../protocol.ts";

/**
 * The scheduler is holding turns back from a provider: a 429, a
 * repeated failure, or a subscription window near its limit.
 */
export interface EventKindProviderBackoff {
  "provider": string;
  "reason": string;
  "type": "provider.backoff";
  "until": string;
}

export interface EventKindProviderRecovered {
  "provider": string;
  "type": "provider.recovered";
}

/**
 * A subscription engine reported how much of its window is spent.
 * A turn moved to the next link of its model chain (VISION §4.5).
 */
export interface EventKindProviderFallback {
  "class"?: ModelErrorClass | null;
  "from": ModelRef;
  "reason": string;
  "to": ModelRef;
  "type": "provider.fallback";
}

export interface EventKindProviderQuota {
  "provider": string;
  "type": "provider.quota";
  "usage"?: ProviderUsage | null;
  "window": QuotaWindow;
}
