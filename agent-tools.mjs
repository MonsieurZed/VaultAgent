/* Le modèle reçoit seulement la confirmation ; la valeur reste dans le moteur local de saisie. */
import { spawn } from 'node:child_process';
import { generateKeyPairSync, privateDecrypt, createDecipheriv, constants } from 'node:crypto';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

function invoke(executable, args) {
  return new Promise((resolve, reject) => {
    const child = spawn(executable, args, { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
    let output = ''; let size = 0;
    child.stdout.setEncoding('utf8');
    child.stdout.on('data', chunk => { size += chunk.length; if (size > 4 * 1024 * 1024) child.kill(); else output += chunk; });
    child.stderr.resume();
    child.on('error', () => reject(new Error('Outil local du coffre indisponible.')));
    child.on('close', code => { if (code !== 0) reject(new Error('Saisie refusée ou coffre indisponible.')); else {
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

export async function typeWindows({ itemId, field, executable = join(dirname(fileURLToPath(import.meta.url)), 'VaultAgent.exe') }) {
  return invoke(executable, ['type', itemId, field]);
}
