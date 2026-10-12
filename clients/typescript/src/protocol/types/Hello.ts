// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.


export interface Hello {
  "client_name": string;
  "client_version": string;
  "features"?: Array<string>;
  "protocol_version": number;
  "token"?: string | null;
}
