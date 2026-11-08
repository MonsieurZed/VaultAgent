/* Client multiplateforme : ne renvoyer que des metadonnees ou une confirmation, jamais la valeur remplie. */
import { spawn } from 'node:child_process';
import { readFile, stat } from 'node:fs/promises';
import { homedir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { generateKeyPairSync, privateDecrypt, createDecipheriv, constants } from 'node:crypto';

const root = dirname(fileURLToPath(import.meta.url));
export const defaultConfig = () => join(homedir(), '.config', 'vaultagent', 'client.json');
export function validServer(address) { const url = new URL(address); if (url.protocol !== 'https:' || url.username || url.password || url.pathname !== '/' || url.search || url.hash) throw new Error('HTTPS VaultAgent server required'); return url.origin; }
async function native(args, executable = join(root, 'VaultAgent-Client.exe')) {
  return new Promise((resolve, reject) => {
    const child = spawn(executable, args, { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] }); let output = '';
    const timer = setTimeout(() => child.kill(), 115000); child.stderr.resume();
    child.stdout.on('data', value => { output += value; if (output.length > 4 * 1024 * 1024) child.kill(); });
    child.once('error', () => { clearTimeout(timer); reject(new Error('Remote client unavailable')); });
    child.once('close', code => { clearTimeout(timer); try { if (code !== 0) throw new Error(); resolve(JSON.parse(output)); } catch { reject(new Error('Request refused; check pairing and server vault authorization')); } });
  });
}
export async function request(operation, payload = {}, options = {}) {
  if (!['status', 'catalog', 'lock', 'seal'].includes(operation)) throw new Error('Operation refused');
  if (process.platform === 'win32' && !options.configFile) {
    const args = operation === 'catalog' ? ['catalog', payload.search || ''] : operation === 'seal' ? ['seal', payload.itemId, payload.field, payload.publicKey, payload.origin] : [operation];
    return native(args, options.executable);
  }
  const file = options.configFile || defaultConfig(); const details = await stat(file);
  if (process.platform !== 'win32' && ((details.mode & 0o077) || details.uid !== process.getuid())) throw new Error('Client configuration must be private (0600)');
  const config = JSON.parse(await readFile(file, 'utf8'));
  const response = await fetch(validServer(config.server) + '/api/agent/' + operation, { method: 'POST', redirect: 'error', signal: AbortSignal.timeout(110000), headers: { authorization: 'Bearer ' + config.token, 'content-type': 'application/json' }, body: JSON.stringify(payload) });
  if (!response.ok) throw new Error(response.status === 423 ? 'Vault locked: authorize clients in server UI' : 'Request refused; check client pairing');
  const text = await response.text(); if (text.length > 4 * 1024 * 1024) throw new Error('Invalid response size'); return JSON.parse(text);
}
export const status = options => request('status', {}, options);
export const catalog = (search = '', options) => request('catalog', { search }, options);
export const lock = options => request('lock', {}, options);
export async function fillBrowser({ locator, itemId, field, origin, ...options }) {
  if (!['username', 'password', 'totp'].includes(field) || !locator || typeof locator.fill !== 'function' || validServer(origin) !== origin) throw new Error('Target field and HTTPS origin required');
  const { publicKey, privateKey } = generateKeyPairSync('rsa', { modulusLength: 2048 });
  const envelope = await request('seal', { itemId, field, origin, publicKey: publicKey.export({ type: 'spki', format: 'der' }).toString('base64') }, options);
  let key, clear;
  try {
    key = privateDecrypt({ key: privateKey, oaepHash: 'sha256', padding: constants.RSA_PKCS1_OAEP_PADDING }, Buffer.from(envelope.encryptedKey, 'base64'));
    const decoder = createDecipheriv('aes-256-gcm', key, Buffer.from(envelope.nonce, 'base64')); decoder.setAuthTag(Buffer.from(envelope.tag, 'base64'));
    clear = Buffer.concat([decoder.update(Buffer.from(envelope.data, 'base64')), decoder.final()]); await locator.fill(clear.toString('utf8'));
    return { filled: true, field, clipboardUsed: false };
  } finally { key?.fill(0); clear?.fill(0); }
}
export async function typeWindows({ itemId, field, origin, executable }) {
  if (process.platform !== 'win32') throw new Error('Windows required');
  if (!['username', 'password', 'totp'].includes(field) || validServer(origin) !== origin) throw new Error('Invalid target');
  return native(['type', itemId, field, origin], executable);
}
