// Private-instance MCP client for Pi's native extension. The durable request and
// continuation contracts remain on the existing RambleDesk managed MCP server.
// No generic token files, proxy discovery, redirects, retries, or feedback polling.
import { request } from "node:http";
import { isIP } from "node:net";

const MAX_BYTES = 16 * 1024 * 1024;
const MESSAGE = "Managed RambleDesk feedback is unavailable or was revoked. Preserve the original request_id and recover it after the session resumes.";

export function managedCapability(env = process.env) {
  let url;
  try { url = new URL(env.RAMBLEDESK_MANAGED_MCP_URL); } catch { throw new Error(MESSAGE); }
  const host = url.hostname.replace(/^\[|\]$/g, "");
  const loopback = isIP(host) === 4 ? host.startsWith("127.") : host === "::1";
  const token = env.RAMBLEDESK_MANAGED_MCP_TOKEN;
  if (url.protocol !== "http:" || !loopback || url.pathname !== "/mcp-managed" || url.search || url.hash || url.username || url.password || url.port === "0" || !/^[a-f\d]{64}$/i.test(token ?? "")) throw new Error(MESSAGE);
  return { url, token };
}

function post(capability, sessionId, payload, signal) {
  const bytes = Buffer.from(JSON.stringify(payload));
  if (bytes.length > MAX_BYTES || signal?.aborted) return Promise.reject(new Error(MESSAGE));
  return new Promise((resolve, reject) => {
    let settled = false;
    let outbound;
    let deadline;
    const finish = (callback) => {
      if (settled) return;
      settled = true;
      clearTimeout(deadline);
      signal?.removeEventListener("abort", abort);
      callback();
    };
    const fail = () => finish(() => reject(new Error(MESSAGE)));
    const abort = () => { outbound?.destroy(); fail(); };
    deadline = setTimeout(abort, 60_000);
    const headers = { Authorization: `Bearer ${capability.token}`, "Content-Type": "application/json", Accept: "application/json, text/event-stream", "Content-Length": bytes.length, "MCP-Protocol-Version": "2025-03-26" };
    if (sessionId) headers["Mcp-Session-Id"] = sessionId;
    // node:http does not use HTTP_PROXY or follow redirects. The destination was
    // validated as a literal loopback address before any credentials were read.
    outbound = request(capability.url, { method: "POST", headers, agent: false, timeout: 60_000 }, (incoming) => {
      const chunks = [];
      let size = 0;
      incoming.on("data", (chunk) => {
        size += chunk.length;
        if (size > MAX_BYTES) { incoming.destroy(); abort(); } else chunks.push(chunk);
      });
      incoming.once("error", fail);
      incoming.once("aborted", fail);
      incoming.once("end", () => {
        if (incoming.statusCode < 200 || incoming.statusCode >= 300) { fail(); return; }
        try {
          if (!Object.hasOwn(payload, "id")) { finish(() => resolve({})); return; }
          const text = Buffer.concat(chunks).toString("utf8");
          let messages;
          if (String(incoming.headers["content-type"]).startsWith("application/json")) messages = [JSON.parse(text)];
          else messages = text.replaceAll("\r\n", "\n").split("\n\n").map((event) => event.split("\n").filter((line) => line.startsWith("data:")).map((line) => line.slice(5).trimStart()).join("\n")).filter(Boolean).map((data) => JSON.parse(data));
          const response = messages.find((message) => message.id === payload.id);
          if (!response || response.error || !Object.hasOwn(response, "result")) { fail(); return; }
          finish(() => resolve({ result: response.result, sessionId: incoming.headers["mcp-session-id"] }));
        } catch { fail(); }
      });
    });
    outbound.once("error", fail);
    outbound.once("timeout", abort);
    signal?.addEventListener("abort", abort, { once: true });
    if (signal?.aborted) { abort(); return; }
    outbound.end(bytes);
  });
}

export class ManagedFeedbackClient {
  #capability;
  #sessionId;
  #sequence = 0;
  #initialization;
  constructor(env = process.env) { this.#capability = managedCapability(env); }
  async initialize() {
    this.#initialization ??= (async () => {
      const response = await post(this.#capability, undefined, { jsonrpc: "2.0", id: ++this.#sequence, method: "initialize", params: { protocolVersion: "2025-03-26", capabilities: {}, clientInfo: { name: "rambledesk-managed-pi", version: "1" } } });
      if (typeof response.sessionId !== "string" || !response.sessionId) throw new Error(MESSAGE);
      this.#sessionId = response.sessionId;
      await post(this.#capability, this.#sessionId, { jsonrpc: "2.0", method: "notifications/initialized" });
      return response.result;
    })();
    return this.#initialization;
  }
  async #rpc(method, params, signal) {
    await this.initialize();
    return (await post(this.#capability, this.#sessionId, { jsonrpc: "2.0", id: ++this.#sequence, method, params }, signal)).result;
  }
  async tools() { return (await this.#rpc("tools/list", {})).tools; }
  async call(name, args, signal) {
    if (!["request_feedback", "get_feedback", "recover_feedback"].includes(name)) throw new Error(MESSAGE);
    return this.#rpc("tools/call", { name, arguments: args }, signal);
  }
}
