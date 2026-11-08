import { randomUUID, randomBytes, createPublicKey, createCipheriv, publicEncrypt, constants } from 'node:crypto';
import { performance } from 'node:perf_hooks';

export function seal(value, publicKey) {
  const key = createPublicKey({ key: Buffer.from(publicKey, 'base64'), type: 'spki', format: 'der' });
  if (key.asymmetricKeyType !== 'rsa' || key.asymmetricKeyDetails.modulusLength < 2048 || key.asymmetricKeyDetails.modulusLength > 4096) throw new Error('invalid-key');
  const aes = randomBytes(32); const nonce = randomBytes(12); const clear = Buffer.from(value);
  try {
    const cipher = createCipheriv('aes-256-gcm', aes, nonce); const data = Buffer.concat([cipher.update(clear), cipher.final()]);
    return { encryptedKey: publicEncrypt({ key, oaepHash: 'sha256', padding: constants.RSA_PKCS1_OAEP_PADDING }, aes).toString('base64'), nonce: nonce.toString('base64'), tag: cipher.getAuthTag().toString('base64'), data: data.toString('base64') };
  } finally { aes.fill(0); clear.fill(0); }
}
export function originOf(text) { try { const url = new URL(text); return url.protocol === 'https:' && !url.username && !url.password ? url.origin : null; } catch { return null; } }
export class Session {
  constructor(store, factory, clock = () => performance.now()) { this.store = store; this.factory = factory; this.clock = clock; this.generation = 0; this.tail = Promise.resolve(); }
  serial(action) { const result = this.tail.then(action); this.tail = result.catch(() => {}); return result; }
  status() { this.expire(); return { version: '3.0.0-preview.1', configured: !!this.store.account, unlocked: !!this.engine, timeoutMinutes: this.store.account?.timeoutMinutes ?? null, expiresUtc: this.engine && this.deadline !== null ? new Date(Date.now() + Math.max(0, this.deadline - this.clock())).toISOString() : null }; }
  touch() { this.deadline = this.store.account.timeoutMinutes === null ? null : this.clock() + this.store.account.timeoutMinutes * 60000; }
  lock() { ++this.generation; this.engine?.kill(); this.engine = null; this.deadline = null; }
  expire() { if (this.engine && (this.engine.alive === false || this.deadline !== null && this.clock() >= this.deadline)) this.lock(); }
  access(generation = this.generation) { this.expire(); if (!this.engine || generation !== this.generation) throw new Error('locked'); return this.engine; }
  async connect(account, password, code) {
    let candidate;
    try {
      candidate = await this.factory(account.profile);
      const prepare = { op: 'prepare', server: account.server, email: account.email };
      if ((await candidate.request(prepare)).restartRequired) { candidate.kill(); candidate = await this.factory(account.profile); if ((await candidate.request(prepare)).restartRequired) throw new Error('engine-prepare'); }
      await candidate.request({ op: 'connect', server: account.server, email: account.email, password, code }); return candidate;
    } catch (error) { candidate?.kill(); throw error; }
  }
  configure(input) { return this.serial(async () => {
    if (!originOf(input.server) || new URL(input.server).pathname !== '/' || !/^[^\s@]+@[^\s@]+$/.test(input.email || '')) throw new Error('invalid-account');
    if (input.timeoutMinutes !== null && (!Number.isInteger(input.timeoutMinutes) || input.timeoutMinutes < 1 || input.timeoutMinutes > 1440)) throw new Error('invalid-timeout');
    const previous = this.store.account;
    const changed = !previous || input.server !== previous.server || input.email.toLowerCase() !== previous.email.toLowerCase();
    const password = input.password || (!changed && this.store.unprotect(previous.password)); if (!password || password.length > 8192) throw new Error('password-required');
    const account = { server: input.server, email: input.email.toLowerCase(), timeoutMinutes: input.timeoutMinutes, profile: changed ? randomUUID() : previous.profile, password: this.store.protect(password) };
    const generation = this.generation; const candidate = await this.connect(account, password, input.code);
    if (generation !== this.generation) { candidate.kill(); throw new Error('cancelled'); }
    try { if (changed) await this.store.updateDevices(() => []); await this.store.save('account.json', account); } catch (error) { candidate.kill(); throw error; }
    this.lock(); this.store.account = account;
    this.engine = candidate; this.touch(); return this.status();
  }); }
  unlock(code) { return this.serial(async () => {
    if (!this.store.account) throw new Error('not-configured');
    this.expire(); if (this.engine) { this.touch(); return this.status(); }
    const generation = this.generation; const candidate = await this.connect(this.store.account, this.store.unprotect(this.store.account.password), code);
    if (generation !== this.generation) { candidate.kill(); throw new Error('cancelled'); }
    this.engine = candidate; this.touch(); return this.status();
  }); }
  catalog(search = '', authorize = () => {}) { return this.serial(async () => {
    authorize(); const generation = this.generation; const engine = this.access(generation); const result = await engine.request({ op: 'call', method: 'GET', path: '/list/object/items?search=' + encodeURIComponent(String(search).slice(0, 200)) }); authorize(); this.access(generation); this.touch();
    return result.data.filter(item => item.type === 1).map(item => ({ id: item.id, name: item.name, origins: [...new Set((item.login?.uris || []).map(value => originOf(value.uri)).filter(Boolean))], username: !!item.login?.username, password: !!item.login?.password, totp: !!item.login?.totp }));
  }); }
  secret(input, authorize = () => {}) { return this.serial(async () => {
    authorize();
    if (!/^[a-fA-F0-9]{8}(?:-[a-fA-F0-9]{4}){3}-[a-fA-F0-9]{12}$/.test(input.itemId || '') || !['username', 'password', 'totp'].includes(input.field) || originOf(input.origin) !== input.origin) throw new Error('invalid-field');
    const generation = this.generation; const engine = this.access(generation); const item = await engine.request({ op: 'call', method: 'GET', path: '/object/item/' + input.itemId });
    if (!(item.login?.uris || []).some(value => originOf(value.uri) === input.origin)) throw new Error('origin-refused');
    let value;
    if (input.field === 'totp') {
      let period = 30; try { const uri = new URL(item.login.totp); period = Number(uri.searchParams.get('period')) || 30; } catch { }
      if (period < 15 || period > 120) throw new Error('totp-period');
      const remaining = period - Date.now() / 1000 % period;
      if (remaining < 5) await new Promise(resolve => setTimeout(resolve, remaining * 1000 + 100));
      this.access(generation); value = (await engine.request({ op: 'call', method: 'GET', path: '/object/totp/' + input.itemId })).data;
    } else value = item.login?.[input.field];
    if (typeof value !== 'string' || !value || value.length > 65536) throw new Error('field-missing');
    authorize(); this.access(generation); const result = seal(value, input.publicKey); this.touch(); return result;
  }); }
}
