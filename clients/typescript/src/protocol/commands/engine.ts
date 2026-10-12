// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentId,
} from "../../protocol.ts";

/**
 * A harness tool call or permission prompt from an external engine's
 * MCP bridge, authenticated by the per-turn token.
 */
export interface EngineToolCommand {
  "cmd": "engine_tool";
  "agent": AgentId;
  "input": unknown;
  "name": string;
  "token": string;
}

export function engineTool(args: Omit<EngineToolCommand, "cmd">): EngineToolCommand {
  return { cmd: "engine_tool", ...args };
}

/**
 * The tools an engine's MCP bridge should advertise for this agent:
 * Aether's harness tools plus the MCP servers' tools it may use.
 */
export interface EngineToolsCommand {
  "cmd": "engine_tools";
  "agent": AgentId;
  "token": string;
}

export function engineTools(args: Omit<EngineToolsCommand, "cmd">): EngineToolsCommand {
  return { cmd: "engine_tools", ...args };
}
