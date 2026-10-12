// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.


/**
 * The filesystem holding the daemon's data is below its floor: new turns
 * wait until it has room again. Turns already running carry on.
 */
export interface LowDisk {
  "available_bytes": number;
  "floor_bytes": number;
  "path": string;
  "since": string;
}
