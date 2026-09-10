import assert from "node:assert/strict";
import { test } from "node:test";
import register from "../index.js";

test("managed command session does not register external Pi tools or prompts", () => {
  const previous = process.env.RAMBLEDESK_MANAGED_SESSION;
  process.env.RAMBLEDESK_MANAGED_SESSION = "1";
  try {
    register(new Proxy({}, { get() { throw new Error("external Pi registration"); } }));
  } finally {
    if (previous === undefined) delete process.env.RAMBLEDESK_MANAGED_SESSION;
    else process.env.RAMBLEDESK_MANAGED_SESSION = previous;
  }
  assert.ok(true);
});
