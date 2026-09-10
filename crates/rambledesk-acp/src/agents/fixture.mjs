// Local subprocess fixture: no network, registry, user configuration or models.
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { spawn } from 'node:child_process';
const args = process.argv.slice(2);
if (args[0] === 'prefix') {
  process.stdout.write(process.env.FIXTURE_GLOBAL_PREFIX + '\n');
} else if (args[0] === 'install') {
  const prefix = args[args.indexOf('--prefix') + 1];
  mkdirSync(prefix, {recursive:true});
  writeFileSync(join(prefix, 'arguments.json'), JSON.stringify(args));
  const mode = process.env.FIXTURE_MODE ?? 'success';
  if (mode === 'hang') {
    spawn(process.execPath, ['-e', `const fs=require('node:fs');setInterval(()=>fs.writeFileSync(process.env.FIXTURE_HEARTBEAT,String(Date.now())),10)`], {stdio:'inherit',windowsHide:true});
    writeFileSync(process.env.FIXTURE_STARTED, 'started');
    setInterval(()=>{},1000);
  } else if (mode === 'fail') {
    process.stderr.write('EACCES fixture-secret-must-not-be-shown\n'); process.exitCode = 1;
  } else {
    const commands = {'deepseek-acp':'deepseek-acp','@deepseek-ai/dsh':'dsh','@agentclientprotocol/codex-acp':'codex-acp','@agentclientprotocol/claude-agent-acp':'claude-agent-acp','pi-acp':'pi-acp','@earendil-works/pi-coding-agent':'pi'};
    for (const spec of args.filter(value=>/^(@[^/]+\/)?[a-z][a-z0-9-]*@\d/.test(value))) {
      const offset = spec.lastIndexOf('@'), name = spec.slice(0,offset), version = spec.slice(offset+1);
      const command = commands[name];
      if (!command) throw new Error('Unexpected fixture package');
      const directory = join(prefix, 'node_modules', name);
      const entry = 'nested/entry.mjs';
      mkdirSync(join(directory,'nested'), {recursive:true});
      writeFileSync(join(directory,'package.json'), JSON.stringify({name, version:mode==='wrong-version'?'99.0.0':version, bin:{[command]:mode==='escape-bin'?'../../outside.mjs':entry}}));
      if (mode !== 'missing-bin') writeFileSync(join(directory,entry), `#!/usr/bin/env node\nprocess.stdout.write(${JSON.stringify(name+' '+version+'\n')});\n`);
      mkdirSync(join(prefix,'node_modules','.bin'),{recursive:true});
      writeFileSync(join(prefix,'node_modules','.bin',command+(process.platform==='win32'?'.cmd':'')),'fixture shim');
    }
    process.stdout.write(Buffer.from([0xff,0xfe]));
    process.stdout.write('installed\n');
  }
} else if (args[0] === 'flood') {
  process.stdout.write('x'.repeat(100000)); process.stderr.write('y'.repeat(100000));
} else { process.stdout.write('fixture-tool 2.3.4\n'); }
