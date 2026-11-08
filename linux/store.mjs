/* Etat prive du serveur : aucun secret dans les variables d'environnement ou les journaux. */
import { mkdir, readFile, writeFile, rename, chmod, stat } from 'node:fs/promises';
import { join } from 'node:path';
import { randomBytes, createCipheriv, createDecipheriv, createHash, timingSafeEqual } from 'node:crypto';

export class Store {
  constructor(directory) { this.directory = directory; this.deviceWrites = Promise.resolve(); }
  updateDevices(transform) {
    const action = this.deviceWrites.then(async () => { const next = transform(this.devices); await this.save('devices.json', next); this.devices = next; });
    this.deviceWrites = action.catch(() => {}); return action;
  }
  async init() {
    await mkdir(this.directory, { recursive: true, mode: 0o700 }); await chmod(this.directory, 0o700);
    const path = join(this.directory, 'machine.key');
    try { await writeFile(path, randomBytes(32), { flag: 'wx', mode: 0o600 }); } catch (error) { if (error.code !== 'EEXIST') throw error; }
    if (process.platform !== 'win32' && ((await stat(path)).mode & 0o077)) throw new Error('private-key-permissions');
    this.key = await readFile(path); if (this.key.length !== 32) throw new Error('private-key-invalid');
    this.account = await this.load('account.json', null); this.devices = await this.load('devices.json', []);
    return this;
  }
  async load(name, fallback) { try { return JSON.parse(await readFile(join(this.directory, name), 'utf8')); } catch (error) { if (error.code === 'ENOENT') return fallback; throw error; } }
  async save(name, value) {
    const temporary = join(this.directory, name + '.' + randomBytes(8).toString('hex') + '.tmp');
    await writeFile(temporary, JSON.stringify(value), { flag: 'wx', mode: 0o600 }); await rename(temporary, join(this.directory, name));
  }
  protect(value) {
    const nonce = randomBytes(12); const cipher = createCipheriv('aes-256-gcm', this.key, nonce);
    const clear = Buffer.from(value); try { const data = Buffer.concat([cipher.update(clear), cipher.final()]); return { nonce: nonce.toString('base64'), tag: cipher.getAuthTag().toString('base64'), data: data.toString('base64') }; } finally { clear.fill(0); }
  }
  unprotect(value) { const decoder = createDecipheriv('aes-256-gcm', this.key, Buffer.from(value.nonce, 'base64')); decoder.setAuthTag(Buffer.from(value.tag, 'base64')); const clear = Buffer.concat([decoder.update(Buffer.from(value.data, 'base64')), decoder.final()]); try { return clear.toString('utf8'); } finally { clear.fill(0); } }
}
export const digest = value => createHash('sha256').update(value).digest('hex');
export const same = (left, right) => typeof left === 'string' && typeof right === 'string' && left.length === right.length && timingSafeEqual(Buffer.from(left), Buffer.from(right));
