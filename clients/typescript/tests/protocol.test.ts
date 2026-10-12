import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import * as protocol from "../src/protocol.ts";

test("every generated command builder remains exported and preserves its frame", () => {
  const schema = JSON.parse(readFileSync(new URL("../../protocol.schema.json", import.meta.url), "utf8"));
  const variants = schema.$defs.ClientFrame.oneOf;
  assert.ok(Array.isArray(variants) && variants.length > 0);
  for (const variant of variants) {
    const tag = variant.properties.cmd.const as string;
    const name = tag.replace(/_([a-z])/g, (_, letter: string) => letter.toUpperCase());
    const builder = (protocol as unknown as Record<string, unknown>)[name];
    assert.equal(typeof builder, "function", name);
    // This checks the generated builder's forwarding contract, not payload
    // validation: static types and the daemon validate real command inputs.
    const args = { marker: "preserved", enabled: false, amount: 0, optional: null };
    const fields = Object.keys(variant.properties).filter(key => key !== "cmd" && key !== "req");
    const reference = variant.$ref && schema.$defs[variant.$ref.split("/").at(-1)];
    const hasFields = fields.length > 0 || Boolean(reference);
    const frame = (builder as (args: object) => object)(args);
    assert.deepEqual(frame, hasFields ? { cmd: tag, ...args } : { cmd: tag }, name);
  }
});
