// Frozen entrypoint guard shipped before the common command workflow.
// Source: packages/pi-rambledesk/index.js at 3a8d66f. Registration is replaced
// with a diagnostic stub; this fixture never accesses a real adapter or server.
function registerRambleDeskPiTools() { process.stdout.write('external tools registered\n') }
function rambledeskPiPackage(pi) {
  if (process.env.RAMBLEDESK_MANAGED_PI_ACTIVE === "1" || process.env.RAMBLEDESK_MANAGED_MCP_URL || process.env.RAMBLEDESK_MANAGED_MCP_TOKEN) return;
  registerRambleDeskPiTools(pi);
}
rambledeskPiPackage({})
if (process.env.RAMBLEDESK_MANAGED_PI_ACTIVE === '1' || process.env.RAMBLEDESK_MANAGED_MCP_URL || process.env.RAMBLEDESK_MANAGED_MCP_TOKEN) process.stdout.write('suppressed\n')
