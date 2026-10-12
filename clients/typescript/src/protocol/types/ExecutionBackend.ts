// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.


/**
 * Where an agent's tools actually run.
 * Tagged by `backend`.
 */
/**
 * The daemon's own machine, inside the scoped roots.
 */
export interface ExecutionBackendHost {
  "backend": "host";
}

/**
 * A container per call, with the workspace mounted writable at the same
 * path, everything else in scope mounted read-only, and network per the
 * scope's policy.
 */
export interface ExecutionBackendContainer {
  "backend": "container";
  "image": string;
}

export type ExecutionBackend = ExecutionBackendHost | ExecutionBackendContainer;
