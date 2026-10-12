// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.


/**
 * What a rule matches. An absent field matches anything, so the empty
 * matcher covers every call — deliberately, so "allow everything for this
 * run" is expressible without a magic pattern.
 */
export interface RuleMatch {
  "argument"?: string | null;
  "resource_prefix"?: string | null;
  "tool"?: string | null;
}
