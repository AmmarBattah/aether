// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.


/**
 * What approval policy an agent gets when nothing else decides one. The
 * shipped default is `Yolo`: the operator asked for auto-approve out of the
 * box, and the execution backend - not the prompt - is the isolation
 * boundary.
 */
export type ApprovalDefault = "yolo" | "ask";
