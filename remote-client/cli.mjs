/* L'association lit le code masque sur le terminal ; jamais d'argument contenant ce code. */
import { createInterface } from 'node:readline';
import { Writable } from 'node:stream';
import { mkdir, writeFile, rename } from 'node:fs/promises';
import { dirname } from 'node:path';
import { randomUUID } from 'node:crypto';
import { defaultConfig, validServer, status, catalog, lock, unlock } from './agent-tools.mjs';

async function pair(address) {
  if (process.platform === 'win32') throw new Error('Use VaultAgent-Client.exe to protect the token with Windows DPAPI');
  if (!process.stdin.isTTY) throw new Error('Interactive local terminal required for pairing');
  const server = validServer(address); let muted = false;
  const output = new Writable({ write(chunk, encoding, callback) { if (!muted) process.stdout.write(chunk, encoding); callback(); } });
  const terminal = createInterface({ input: process.stdin, output, terminal: true });
  const code = await new Promise(resolve => { terminal.question('Code: ', resolve); muted = true; }); terminal.close(); process.stdout.write('\n');
  const response = await fetch(server + '/api/pair', { method: 'POST', redirect: 'error', signal: AbortSignal.timeout(15000), headers: { 'content-type': 'application/json' }, body: JSON.stringify({ code: code.trim() }) });
  if (!response.ok) throw new Error('Pairing refused or code expired');
  const result = await response.json(); const file = defaultConfig(); await mkdir(dirname(file), { recursive: true, mode: 0o700 });
  const temporary = file + '.' + randomUUID(); await writeFile(temporary, JSON.stringify({ server, token: result.token }), { flag: 'wx', mode: 0o600 }); await rename(temporary, file);
  return { paired: true };
}
try {
  const [command, argument] = process.argv.slice(2);
  const result = command === 'pair' ? await pair(argument) : command === 'status' ? await status() : command === 'unlock' ? await unlock({ waitSeconds: argument === undefined ? 0 : Number(argument) }) : command === 'catalog' ? await catalog(argument) : command === 'lock' ? await lock() : (() => { throw new Error('Commands: pair <https-server>, status, catalog [search], lock, unlock [wait-seconds]'); })();
  console.log(JSON.stringify(result));
} catch { console.error('Operation refused. Check pairing, HTTPS address and server authorization.'); process.exitCode = 1; }
