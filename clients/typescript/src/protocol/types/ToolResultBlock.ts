// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.


/**
 * Content allowed inside a tool result.
 * Tagged by `type`.
 */
export interface ToolResultBlockText {
  "text": string;
  "type": "text";
}

export interface ToolResultBlockImage {
  "data": string;
  "media_type": string;
  "type": "image";
}

export type ToolResultBlock = ToolResultBlockText | ToolResultBlockImage;
