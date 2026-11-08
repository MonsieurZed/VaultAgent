/* Commandes de remplissage uniquement ; aucun mode de lecture des valeurs du coffre. */
import { spawn } from 'node:child_process';
import { join } from 'node:path';

const executable = join(process.env.ProgramW6432 || process.env.ProgramFiles || 'C:\\Program Files', 'HomelabVaultAgent', 'VaultAgent.exe');
const [action, ...args] = process.argv.slice(2);

function invoke(parameters, json = true) {
  return new Promise((resolve, reject) => {
    const child = spawn(executable, parameters, { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
    let output = ''; let oversized = false;
    child.stdout.setEncoding('utf8'); child.stderr.resume();
    child.stdout.on('data', value => {
      output += value;
      if (output.length > 4 * 1024 * 1024) { oversized = true; output = ''; child.kill(); }
    });
    child.on('error', () => reject(new Error('VaultAgent absent ou inaccessible sur ce PC Windows.')));
    child.on('close', code => {
      if (code !== 0 || oversized) { reject(new Error('Operation refusee ou annulee. Verifier le service et la validation Windows.')); return; }
      try { resolve(json ? JSON.parse(output) : undefined); }
      catch { reject(new Error('Reponse du service invalide.')); }
    });
  });
}

async function main() {
  if (!['status', 'catalog', 'unlock', 'lock', 'type'].includes(action)) throw new Error('Commandes : status, catalog [recherche], unlock, lock, type UUID username|password|totp.');
  if (['status', 'unlock', 'lock'].includes(action) && args.length !== 0) throw new Error('Aucun argument attendu.');
  if (action === 'catalog') {
    if (args.length > 1) throw new Error('Fournir une seule recherche entre guillemets.');
    return invoke(['catalog', ...args]);
  }
  if (action === 'type') {
    if (args.length !== 2 || !/^[a-f\d]{8}(?:-[a-f\d]{4}){3}-[a-f\d]{12}$/i.test(args[0]) || !['username', 'password', 'totp'].includes(args[1])) throw new Error('UUID et champ username|password|totp requis.');
    const status = await invoke(['status']);
    if (!status.unlocked) throw new Error('Deverrouiller le coffre avant de donner le focus au champ cible.');
    return invoke(['type', ...args]);
  }
  if (action === 'unlock') await invoke(['unlock'], false);
  if (action === 'lock') { await invoke(['lock'], false); return { locked: true }; }
  const status = await invoke(['status']);
  return { configured: status.configured, unlocked: status.unlocked, mode: status.mode,
    timeoutMinutes: status.timeoutMinutes, expiresUtc: status.expiresUtc, version: status.version };
}

try { process.stdout.write(JSON.stringify(await main()) + '\n'); }
catch (error) { process.stderr.write(error.message + '\n'); process.exitCode = 1; }
