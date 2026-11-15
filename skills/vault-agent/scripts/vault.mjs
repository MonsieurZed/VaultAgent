/* Commandes de remplissage uniquement, via le client VaultAgent Windows ; aucun mode de lecture des valeurs du coffre. */
import { spawn } from 'node:child_process';
import { join } from 'node:path';

const executable = join(process.env.LOCALAPPDATA || '', 'Programs', 'VaultAgentClient', 'VaultAgent-Client.exe');
const TARGET_REFUSED = 3;
const argv = process.argv.slice(2);
const vaultFlag = argv.indexOf('--vault'); const vaultArgs = [];
if (vaultFlag >= 0) {
  if (!/^[a-z0-9-]{1,64}$/i.test(argv[vaultFlag + 1] || '')) throw new Error('Identifiant de coffre invalide.');
  vaultArgs.push('--vault', argv[vaultFlag + 1]); argv.splice(vaultFlag, 2);
}
const [action, ...args] = argv;

function invoke(parameters, accepted = [0]) {
  return new Promise((resolve, reject) => {
    const child = spawn(executable, [...parameters, ...vaultArgs], { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
    let output = ''; let error = ''; let oversized = false;
    child.stdout.setEncoding('utf8'); child.stderr.setEncoding('utf8');
    child.stderr.on('data', value => { if (error.length < 1024) error += value; });
    child.stdout.on('data', value => { output += value; if (output.length > 4 * 1024 * 1024) { oversized = true; output = ''; child.kill(); } });
    child.on('error', () => reject(new Error('Client VaultAgent absent ou inaccessible sur ce PC Windows.')));
    child.on('close', code => {
      if (!accepted.includes(code) || oversized) { reject(new Error(error.trim() === 'locked' ? 'Coffre verrouille : lancer unlock.' : error.trim() === 'vault-required' ? 'Choisir un coffre avec --vault (voir vaults).' : 'Operation refusee. Verifier l\'association du client et l\'autorisation du coffre.')); return; }
      try { resolve(JSON.parse(output)); } catch { reject(new Error('Reponse du client invalide.')); }
    });
  });
}

async function unlock(waitSeconds) {
  if (!Number.isInteger(waitSeconds) || waitSeconds < 0 || waitSeconds > 300) throw new Error('Delai d\'attente entre 0 et 300 secondes.');
  const request = await invoke(['request-unlock']);
  if (request.unlocked || waitSeconds === 0) return { unlocked: !!request.unlocked, state: request.state };
  const deadline = Date.now() + waitSeconds * 1000;
  while (Date.now() < deadline) {
    await new Promise(resolve => setTimeout(resolve, 2000));
    const status = await invoke(['status']);
    if (status.unlocked) return { unlocked: true, state: 'unlocked' };
    const pending = status.unlockRequest;
    if (!pending) return { unlocked: false, state: 'expired' };
    if (['denied', 'failed', 'expired'].includes(pending.state)) return { unlocked: false, state: pending.state };
  }
  return { unlocked: false, state: 'pending' };
}

async function main() {
  if (!['status', 'vaults', 'catalog', 'unlock', 'lock', 'type', 'foreground', 'focus'].includes(action)) throw new Error('Commandes : status, vaults, catalog [recherche], unlock [secondes], lock, foreground, focus --process <exe> --title <fragment>, type UUID username|password|totp ORIGINE_HTTPS --process <exe> --title <fragment> ; option --vault <id>.');
  if (['status', 'vaults', 'lock', 'foreground'].includes(action) && args.length !== 0) throw new Error('Aucun argument attendu.');
  if (action === 'status') { const status = await invoke(['status']); return { vaultId: status.vaultId, vaultName: status.vaultName, configured: status.configured, unlocked: status.unlocked, timeoutMinutes: status.timeoutMinutes, expiresUtc: status.expiresUtc, version: status.version }; }
  if (action === 'vaults') return invoke(['vaults']);
  if (action === 'foreground') return invoke(['foreground']);
  if (action === 'lock') return invoke(['lock']);
  if (action === 'unlock') return unlock(args.length ? Number(args[0]) : 60);
  if (action === 'focus') {
    const at = ['--process', '--title'].map(flag => args.indexOf(flag));
    if (args.length !== 4 || at.some(index => index < 0 || index % 2 !== 0) || !args[at[0] + 1] || !args[at[1] + 1]) throw new Error('--process <exe> et --title <fragment du titre> requis.');
    const result = await invoke(['focus', ...args], [0, TARGET_REFUSED]);
    if (result.focused !== true) throw new Error('Mise au premier plan refusee : ' + result.reason + ' Fenetre active : ' + (result.foreground ? result.foreground.process + ' "' + result.foreground.title + '"' : 'illisible') + '.');
    return result;
  }
  if (action === 'catalog') {
    if (args.length > 1) throw new Error('Fournir une seule recherche entre guillemets.');
    return invoke(['catalog', ...args]);
  }
  const identity = ['--process', '--title'].map(flag => args.indexOf(flag));
  if (args.length !== 7 || identity.some(index => index < 3 || index + 1 >= args.length) || identity[0] === identity[1] || !args[identity[0] + 1] || !args[identity[1] + 1]) throw new Error('UUID, champ, origine HTTPS, --process <exe> et --title <fragment du titre> requis.');
  if (!/^[a-f\d]{8}(?:-[a-f\d]{4}){3}-[a-f\d]{12}$/i.test(args[0]) || !['username', 'password', 'totp'].includes(args[1])) throw new Error('UUID et champ username|password|totp requis.');
  let origin; try { origin = new URL(args[2]); } catch { origin = null; }
  if (!origin || origin.protocol !== 'https:' || origin.origin !== args[2]) throw new Error('Origine HTTPS exacte requise, sans chemin (ex. https://nvr.lan.zed.cx).');
  const result = await invoke(['type', ...args], [0, TARGET_REFUSED]);
  if (result.typed !== true) throw new Error('Saisie refusee : ' + result.reason + ' Fenetre active : ' + (result.foreground ? result.foreground.process + ' "' + result.foreground.title + '"' : 'illisible') + '. Attendu : ' + result.expected.process + ' "' + result.expected.title + '".');
  return result;
}

try { process.stdout.write(JSON.stringify(await main()) + '\n'); }
catch (error) { process.stderr.write(error.message + '\n'); process.exitCode = 1; }
