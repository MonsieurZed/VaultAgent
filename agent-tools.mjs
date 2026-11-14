/* Le modèle reçoit seulement la confirmation ; la valeur reste dans le moteur local de saisie. */
import { spawn } from 'node:child_process';
import { generateKeyPairSync, privateDecrypt, createDecipheriv, constants } from 'node:crypto';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

function invoke(executable, args, acceptedCodes = [0]) {
  return new Promise((resolve, reject) => {
    const child = spawn(executable, args, { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
    let output = ''; let size = 0;
    child.stdout.setEncoding('utf8');
    child.stdout.on('data', chunk => { size += chunk.length; if (size > 4 * 1024 * 1024) child.kill(); else output += chunk; });
    child.stderr.resume();
    child.on('error', () => reject(new Error('Outil local du coffre indisponible.')));
    child.on('close', code => { if (!acceptedCodes.includes(code)) reject(new Error('Saisie refusée ou coffre indisponible.')); else {
      try { resolve(JSON.parse(output)); } catch { reject(new Error('Réponse du coffre invalide.')); }
    }});
  });
}

export async function fillBrowser({ locator, itemId, field, executable = join(dirname(fileURLToPath(import.meta.url)), 'VaultAgent.exe'), probe = false }) {
  if (!['username', 'password', 'totp'].includes(field)) throw new Error('Champ invalide.');
  if (!locator || typeof locator.fill !== 'function') throw new Error('Fournir le champ cible du navigateur.');
  const { publicKey, privateKey } = generateKeyPairSync('rsa', { modulusLength: 2048 });
  const encoded = publicKey.export({ type: 'spki', format: 'der' }).toString('base64');
  const envelope = await invoke(executable, probe ? ['seal-probe', encoded] : ['seal', itemId, field, encoded]);
  let key; let clear; let text;
  try {
    key = privateDecrypt({ key: privateKey, padding: constants.RSA_PKCS1_OAEP_PADDING, oaepHash: 'sha256' }, Buffer.from(envelope.encryptedKey, 'base64'));
    const decoder = createDecipheriv('aes-256-gcm', key, Buffer.from(envelope.nonce, 'base64'));
    decoder.setAuthTag(Buffer.from(envelope.tag, 'base64'));
    clear = Buffer.concat([decoder.update(Buffer.from(envelope.data, 'base64')), decoder.final()]);
    text = clear.toString('utf8');
    await locator.fill(text);
    return { filled: true, field, clipboardUsed: false };
  } finally { text = undefined; if (key) key.fill(0); if (clear) clear.fill(0); }
}

const MISMATCH_EXIT_CODE = 3;

function describeWindow(window) {
  return window ? `${window.process} « ${window.title} »` : 'aucune fenêtre lisible';
}

/* Saisie clavier dans la fenêtre active ; `process` et `title` identifient la cible et sont obligatoires. */
export async function typeWindows({ itemId, field, process: processName, title, executable = join(dirname(fileURLToPath(import.meta.url)), 'VaultAgent.exe') }) {
  if (!['username', 'password', 'totp'].includes(field)) throw new Error('Champ invalide.');
  if (typeof processName !== 'string' || typeof title !== 'string' || !processName.trim() || !title.trim()) {
    throw new Error('Fournir process (exécutable) et title (fragment du titre) pour identifier la fenêtre cible.');
  }
  const result = await invoke(executable, ['type', itemId, field, '--process', processName, '--title', title], [0, MISMATCH_EXIT_CODE]);
  if (result.typed !== true) {
    throw new Error(`Saisie refusée : ${result.reason} Fenêtre active : ${describeWindow(result.foreground)}. Attendu : ${describeWindow({ process: result.expected?.process, title: result.expected?.title })}.`);
  }
  return result;
}

/* Identité de la fenêtre active, pour choisir process et title sans deviner. */
export async function foregroundWindow({ executable = join(dirname(fileURLToPath(import.meta.url)), 'VaultAgent.exe') } = {}) {
  return invoke(executable, ['foreground']);
}
