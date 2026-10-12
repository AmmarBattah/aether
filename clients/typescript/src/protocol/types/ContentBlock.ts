// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  ToolResultBlock,
} from "../../protocol.ts";

/**
 * A block inside a message.
 * Tagged by `type`.
 */
export interface ContentBlockText {
  "text": string;
  "type": "text";
}

export interface ContentBlockThinking {
  "redacted"?: boolean;
  "signature"?: string | null;
  "text": string;
  "type": "thinking";
}

export interface ContentBlockToolUse {
  "id": string;
  "input": unknown;
  "name": string;
  "type": "tool_use";
}

export interface ContentBlockToolResult {
  "content": Array<ToolResultBlock>;
  "is_error"?: boolean;
  "tool_use_id": string;
  "type": "tool_result";
}

export interface ContentBlockImage {
  "data": string;
  "media_type": string;
  "type": "image";
}

export type ContentBlock =
    ContentBlockText |
    ContentBlockThinking |
    ContentBlockToolUse |
    ContentBlockToolResult |
    ContentBlockImage
;
