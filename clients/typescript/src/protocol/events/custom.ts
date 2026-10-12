// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.


/**
 * Escape hatch for orchestration layers to record their own events.
 */
export interface EventKindCustom {
  "name": string;
  "payload": unknown;
  "type": "custom";
}
