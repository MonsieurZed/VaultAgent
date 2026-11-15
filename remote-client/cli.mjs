/* L'association lit le code masque sur le terminal (pair) ou attend l'approbation web (pair-request) ; jamais d'argument contenant un code. */
import { createInterface } from 'node:readline';
import { Writable } from 'node:stream';
import { mkdir, writeFile, rename } from 'node:fs/promises';
import { dirname } from 'node:path';
import { hostname } from 'node:os';
import { randomUUID } from 'node:crypto';
import { defaultConfig, validServer, status, catalog, lock, unlock, vaults, sharing } from './agent-tools.mjs';

async function pair(address) {
  if (process.platform === 'win32') throw new Error('Use VaultAgent-Client.exe to protect the token with Windows DPAPI');
  if (!process.stdin.isTTY) throw new Error('Interactive local terminal required for pairing');
  const server = validServer(address); let muted = false;
  const output = new Writable({ write(chunk, encoding, callback) { if (!muted) process.stdout.write(chunk, encoding); callback(); } });
  const terminal = createInterface({ input: process.stdin, output, terminal: true });
  const code = await new Promise(resolve => { terminal.question('Code: ', resolve); muted = true; }); terminal.close(); process.stdout.write('\n');
  const response = await fetch(server + '/api/pair', { method: 'POST', redirect: 'error', signal: AbortSignal.timeout(15000), headers: { 'content-type': 'application/json' }, body: JSON.stringify({ code: code.trim() }) });
  if (!response.ok) throw new Error('Pairing refused or code expired');
  await saveConfig(server, (await response.json()).token);
  return { paired: true };
}
async function saveConfig(server, token) {
  const file = defaultConfig(); await mkdir(dirname(file), { recursive: true, mode: 0o700 });
  const temporary = file + '.' + randomUUID(); await writeFile(temporary, JSON.stringify({ server, token }), { flag: 'wx', mode: 0o600 }); await rename(temporary, file);
}
async function post(server, body) {
  const response = await fetch(server + '/api/pair', { method: 'POST', redirect: 'error', signal: AbortSignal.timeout(15000), headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) });
  if (!response.ok) throw new Error('Pairing refused');
  return response.json();
}
async function pairRequest(address, name) {
  if (process.platform === 'win32') throw new Error('Use VaultAgent-Client.exe to protect the token with Windows DPAPI');
  const server = validServer(address); const request = await post(server, { action: 'request', name: name || hostname() });
  process.stderr.write('Verification code: ' + request.verification + '\nApprove this request on ' + server + ' (Clients tab) after checking the same code. Expires in ' + request.expiresInSeconds + ' seconds.\n');
  const deadline = Date.now() + request.expiresInSeconds * 1000;
  while (Date.now() < deadline) {
    await new Promise(resolve => setTimeout(resolve, 2000));
    const result = await post(server, { action: 'poll', requestId: request.requestId, secret: request.secret });
    if (result.state === 'approved') { await saveConfig(server, result.token); return { paired: true, vaultId: result.vaultId }; }
    if (result.state === 'denied') throw new Error('Pairing denied');
  }
  throw new Error('Pairing request expired');
}
try {
  const args = process.argv.slice(2); const flag = args.indexOf('--vault'); const options = {};
  if (flag >= 0) { if (!args[flag + 1]) throw new Error('Vault required'); options.vaultId = args[flag + 1]; args.splice(flag, 2); }
  const [command, argument, label] = args;
  const result = command === 'pair' ? await pair(argument) : command === 'pair-request' ? await pairRequest(argument, label) : command === 'vaults' ? await vaults(options) : command === 'sharing' ? await sharing(options) : command === 'status' ? await status(options) : command === 'unlock' ? await unlock({ ...options, waitSeconds: argument === undefined ? 0 : Number(argument) }) : command === 'catalog' ? await catalog(argument, options) : command === 'lock' ? await lock(options) : (() => { throw new Error('Commands: pair <https-server>, pair-request <https-server> [name], vaults, sharing, status, catalog [search], lock, unlock [wait-seconds]; --vault <id>'); })();
  console.log(JSON.stringify(result));
} catch { console.error('Operation refused. Check pairing, HTTPS address and server authorization.'); process.exitCode = 1; }
