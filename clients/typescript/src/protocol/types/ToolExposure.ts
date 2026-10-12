// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.


/**
 * Which registered tools an agent may see.
 * Tagged by `mode`.
 */
export interface ToolExposureAll {
  "mode": "all";
}

export interface ToolExposureOnly {
  "mode": "only";
  "names": Array<string>;
}

export interface ToolExposureExcept {
  "mode": "except";
  "names": Array<string>;
}

export type ToolExposure = ToolExposureAll | ToolExposureOnly | ToolExposureExcept;
