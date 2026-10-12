// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentId,
  Lease,
  ResourceKey,
} from "../../protocol.ts";


export interface EventKindLeaseGranted {
  "lease": Lease;
  "type": "lease.granted";
}

export interface EventKindLeaseRenewed {
  "expires_at": string;
  "type": "lease.renewed";
}

export interface EventKindLeaseReleased {
  "type": "lease.released";
}

export interface EventKindLeaseExpired {
  "holder": string;
  "type": "lease.expired";
}

/**
 * A turn is holding resource keys for its mutating tool calls.
 */
export interface EventKindLeaseResourceAcquired {
  "keys": Array<ResourceKey>;
  "type": "lease.resource_acquired";
}

export interface EventKindLeaseResourceReleased {
  "keys": Array<ResourceKey>;
  "type": "lease.resource_released";
}

/**
 * Another agent holds a key this turn needs; it waits, visibly Blocked.
 */
export interface EventKindLeaseResourceWaiting {
  "blocked_on": ResourceKey;
  "holder": AgentId;
  "keys": Array<ResourceKey>;
  "type": "lease.resource_waiting";
}
