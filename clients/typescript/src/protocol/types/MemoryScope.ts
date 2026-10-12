// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentId,
} from "../../protocol.ts";

/**
 * Tagged by `scope`.
 */
export interface MemoryScopeGlobal {
  "scope": "global";
}

export interface MemoryScopeRepository {
  "id": string;
  "scope": "repository";
}

export interface MemoryScopeRole {
  "role": string;
  "scope": "role";
}

export interface MemoryScopeAgent {
  "id": AgentId;
  "scope": "agent";
}

/**
 * A repository's notes for one role family: agents in repository `id`
 * whose role is `role` or extends it at a dot (see [`role_families`]).
 */
export interface MemoryScopeRepositoryRole {
  "id": string;
  "role": string;
  "scope": "repository_role";
}

/**
 * Recorded by `root` for itself and every agent beneath it.
 */
export interface MemoryScopeSubtree {
  "root": AgentId;
  "scope": "subtree";
}

export type MemoryScope =
    MemoryScopeGlobal |
    MemoryScopeRepository |
    MemoryScopeRole |
    MemoryScopeAgent |
    MemoryScopeRepositoryRole |
    MemoryScopeSubtree
;
