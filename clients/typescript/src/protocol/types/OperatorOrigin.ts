// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.


/**
 * Server-observed connection identity, never a bearer token or client-supplied name.
 * Tagged by `transport`.
 */
export interface OperatorOriginLocalSocket {
  "connection": string;
  "transport": "local_socket";
}

export interface OperatorOriginTcp {
  "connection": string;
  "peer": string;
  "transport": "tcp";
}

export type OperatorOrigin = OperatorOriginLocalSocket | OperatorOriginTcp;
