// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.


export interface AuthInfo {
  "configured": boolean;
  "detail"?: string | null;
  "methods"?: Array<string>;
  "program"?: string | null;
  "provider": string;
  "provider_name": string;
  "source"?: string | null;
  "subscription"?: boolean;
}
