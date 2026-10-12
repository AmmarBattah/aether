// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.


/**
 * A configured MCP server answered and its tools are offered to agents.
 */
export interface EventKindMcpServerConnected {
  "server": string;
  "tools": Array<string>;
  "type": "mcp.server_connected";
}

/**
 * A configured MCP server could not be reached or stopped answering. Its
 * tools are withdrawn; a call against it fails rather than hanging.
 */
export interface EventKindMcpServerFailed {
  "error": string;
  "server": string;
  "type": "mcp.server_failed";
}
