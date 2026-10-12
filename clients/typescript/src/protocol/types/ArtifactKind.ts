// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.


/**
 * Tagged by `kind`.
 */
export interface ArtifactKindFile {
  "kind": "file";
}

export interface ArtifactKindCommandOutput {
  "kind": "command_output";
}

export interface ArtifactKindDiff {
  "kind": "diff";
}

export interface ArtifactKindCommit {
  "kind": "commit";
}

export interface ArtifactKindResult {
  "kind": "result";
}

export interface ArtifactKindOther {
  "kind": "other";
  "label": string;
}

export type ArtifactKind =
    ArtifactKindFile |
    ArtifactKindCommandOutput |
    ArtifactKindDiff |
    ArtifactKindCommit |
    ArtifactKindResult |
    ArtifactKindOther
;
