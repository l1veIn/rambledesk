import { test } from "node:test";
import { apply } from "../index.js";

test("managed command session skips external dsh tools and persisted mode prompt", async () => {
  const previous = process.env.RAMBLEDESK_MANAGED_SESSION;
  process.env.RAMBLEDESK_MANAGED_SESSION = "1";
  try {
    await apply(new Proxy({}, { get() { throw new Error("external dsh registration"); } }));
  } finally {
    if (previous === undefined) delete process.env.RAMBLEDESK_MANAGED_SESSION;
    else process.env.RAMBLEDESK_MANAGED_SESSION = previous;
  }
});
