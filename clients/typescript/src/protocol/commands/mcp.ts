// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.


/**
 * Configured MCP servers and their tools. `refresh` reconnects first,
 * so tools added since daemon start are discovered on demand.
 */
export interface McpServersCommand {
  "cmd": "mcp_servers";
  "refresh"?: boolean;
  "server"?: string | null;
}

export function mcpServers(args: Omit<McpServersCommand, "cmd">): McpServersCommand {
  return { cmd: "mcp_servers", ...args };
}
