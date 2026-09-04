// Used only by the process-supervision tests. A standalone script keeps the Rust
// test harness's progress output out of the JSON-RPC stdout stream.
import { spawn } from 'node:child_process';
import { writeFileSync, existsSync } from 'node:fs';
import { createInterface } from 'node:readline';
const mode = process.env.RAMBLEDESK_ACP_TEST_PROCESS_MODE;
const path = process.env.RAMBLEDESK_ACP_TEST_PID_FILE;
const child = spawn(process.execPath, ['-e', 'setInterval(() => {}, 1000)'], { stdio: 'ignore', windowsHide: true });
writeFileSync(path, `${process.pid} ${child.pid}`);
while (!existsSync(path + '.ready')) await new Promise(resolve => setTimeout(resolve, 10));
const reader = createInterface({ input: process.stdin });
for await (const line of reader) {
  const request = JSON.parse(line);
  if (request.id === undefined) continue;
  if (request.method === 'session/close' && mode === 'protocol-close-hang') continue;
  const error = (request.method === 'initialize' && mode === 'protocol-init-error')
    || (request.method === 'session/close' && mode === 'protocol-close-error');
  const result = request.method === 'initialize'
    ? { protocolVersion: 1, agentCapabilities: { sessionCapabilities: { close: {} } }, authMethods: [] }
    : request.method === 'session/new' ? { sessionId: 'supervised-fixture' } : {};
  console.log(JSON.stringify({ jsonrpc: '2.0', id: request.id,
    ...(error ? { error: { code: -32603, message: 'intentional fixture failure' } } : { result }) }));
}
process.exit(0);
