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
async function native(args, executable = join(root, 'VaultAgent-Client.exe'), payload) {
  return new Promise((resolve, reject) => {
    const child = spawn(executable, args, { windowsHide: true, stdio: [payload ? 'pipe' : 'ignore', 'pipe', 'pipe'] }); let output = '';
    if (payload) { child.stdin.on('error', () => {}); child.stdin.end(JSON.stringify(payload), 'utf8'); }
    const timer = setTimeout(() => child.kill(), 115000); child.stderr.resume();
    child.stdout.on('data', value => { output += value; if (output.length > 4 * 1024 * 1024) child.kill(); });
    child.once('error', () => { clearTimeout(timer); reject(new Error('Remote client unavailable')); });
    child.once('close', code => { clearTimeout(timer); try { if (code !== 0) throw new Error(); resolve(JSON.parse(output)); } catch { reject(new Error('Request refused; check pairing and server vault authorization')); } });
  });
}
export async function request(operation, payload = {}, options = {}) {
  if (!['status', 'catalog', 'lock', 'seal', 'write', 'write-preview', 'request-unlock'].includes(operation)) throw new Error('Operation refused');
  if (process.platform === 'win32' && !options.configFile) {
    const args = operation === 'catalog' ? ['catalog', payload.search || ''] : operation === 'seal' ? ['seal', payload.itemId, payload.field, payload.publicKey, payload.origin] : operation === 'write-preview' ? payload.itemId ? ['write-preview', payload.itemId] : ['write-preview'] : [operation];
    return native(args, options.executable, operation === 'write' ? payload : undefined);
  }
  const file = options.configFile || defaultConfig(); const details = await stat(file);
  if (process.platform !== 'win32' && ((details.mode & 0o077) || details.uid !== process.getuid())) throw new Error('Client configuration must be private (0600)');
  const config = JSON.parse(await readFile(file, 'utf8'));
  const response = await fetch(validServer(config.server) + '/api/agent/' + operation, { method: 'POST', redirect: 'error', signal: AbortSignal.timeout(110000), headers: { authorization: 'Bearer ' + config.token, 'content-type': 'application/json' }, body: JSON.stringify(payload) });
  if (!response.ok) throw new Error(response.status === 423 ? 'Vault locked: authorize clients in server UI' : 'Request refused; check client pairing');
  const text = await response.text(); if (text.length > 4 * 1024 * 1024) throw new Error('Invalid response size'); return JSON.parse(text);
}
export const status = options => request('status', {}, options);
export const requestUnlock = options => request('request-unlock', {}, options);
// Attente explicite et bornée ; jamais de nouvelle demande après refus ou expiration.
export async function unlock({ waitSeconds = 0, signal, ...options } = {}) {
  if (!Number.isInteger(waitSeconds) || waitSeconds < 0 || waitSeconds > 300) throw new Error('Invalid wait duration');
  signal?.throwIfAborted();
  const result = await requestUnlock(options);
  if (result.unlocked || waitSeconds === 0 || !['pending', 'approving'].includes(result.state)) return result;
  const deadline = Date.now() + waitSeconds * 1000;
  while (Date.now() < deadline) {
    signal?.throwIfAborted();
    await new Promise(resolve => setTimeout(resolve, Math.min(2000, deadline - Date.now())));
    signal?.throwIfAborted(); const current = await status(options);
    const approval = current.unlockRequest;
    if (approval && approval.requestId === result.requestId && ['denied', 'failed'].includes(approval.state)) return { ...approval, unlocked: false };
    if (current.unlocked) return { requestId: result.requestId, state: 'unlocked', unlocked: true };
    if (!approval || approval.requestId !== result.requestId) return { requestId: result.requestId, state: 'expired', unlocked: false };
  }
  return { ...result, state: 'waiting', unlocked: false };
}
export const catalog = (search = '', options) => request('catalog', { search }, options);
export const lock = options => request('lock', {}, options);
export const writePreview = (itemId, options) => request('write-preview', { itemId }, options);
export const createFolder = (operationId, name, options) => request('write', { operationId, action: 'create-folder', item: { name } }, options);
// Le fournisseur lit la source dans le même processus ; ses valeurs ne sont jamais renvoyées.
export async function saveFromSource({ operationId, action, itemId, expectedRevisionDate, metadata, source, ...options }) {
  if (typeof source !== 'function') throw new Error('Source callback required');
  let values;
  try {
    if (!['create', 'update'].includes(action) || !metadata || typeof metadata !== 'object' || Array.isArray(metadata)) throw new Error('Invalid metadata');
    const allowed = action === 'create' ? ['type', 'name', 'folderId', 'login'] : ['name', 'folderId'];
    if (Object.keys(metadata).some(key => !allowed.includes(key)) || metadata.login && Object.keys(metadata.login).some(key => key !== 'uris')) throw new Error('Secrets belong in source');
    values = await source();
    if (!values || typeof values !== 'object' || Array.isArray(values) || Object.keys(values).some(k => !['username', 'password', 'totp', 'notes'].includes(k))) throw new Error('Invalid source');
    let payload;
    if (action === 'create') {
      const item = { ...metadata }; if ('notes' in values) item.notes = values.notes;
      if (item.type === 1) { item.login = { ...(metadata?.login || {}) }; for (const field of ['username', 'password', 'totp']) if (field in values) item.login[field] = values[field]; }
      else if (Object.keys(values).some(k => k !== 'notes')) throw new Error('Invalid source');
      payload = { operationId, action, item };
    } else payload = { operationId, action, itemId, expectedRevisionDate, patch: { ...metadata, ...values } };
    return await request('write', payload, options);
  } catch { throw new Error('Save refused or indeterminate: retain operationId; inspect receipt and preview before retry'); }
  finally { values = null; }
}
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
