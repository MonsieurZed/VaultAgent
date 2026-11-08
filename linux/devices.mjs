import { randomBytes, randomUUID } from 'node:crypto';
import { digest, same } from './store.mjs';

export class Devices {
  constructor(store, clock = () => Date.now()) { this.store = store; this.clock = clock; this.pairings = new Map(); this.attempts = []; this.tail = Promise.resolve(); }
  list() { return this.store.devices.filter(value => value.profile === this.store.account?.profile).map(({ id, name, created, autoUnlock }) => ({ id, name, created, autoUnlock: !!autoUnlock })); }
  record(id) { return this.store.devices.find(value => value.id === id && value.profile === this.store.account?.profile); }
  async setAutomatic(id, enabled) {
    if (typeof enabled !== 'boolean' || !this.record(id)) throw new Error('invalid-device');
    await this.store.updateDevices(current => current.map(value => value.id === id && value.profile === this.store.account?.profile ? { ...value, autoUnlock: enabled } : value));
  }
  async clearAutomatic() { await this.store.updateDevices(current => current.map(value => ({ ...value, autoUnlock: false }))); }
  async authorizeHelloEnrollment(id) {
    const profile = this.store.account?.profile;
    await this.store.updateDevices(current => {
      if (this.store.account?.profile !== profile || !current.some(value => value.id === id && value.profile === profile && value.approverPublicKey)) throw new Error('hello-enrollment-refused');
      return current.map(value => value.id === id && value.profile === profile ? { ...value, helloEnrollmentAllowed: true } : value);
    });
  }
  async enrollHello(id, credential) {
    const profile = this.store.account?.profile;
    await this.store.updateDevices(current => {
      const device = current.find(value => value.id === id && value.profile === profile);
      if (this.store.account?.profile !== profile || !device?.approverPublicKey || device.helloCredential) throw new Error('hello-enrollment-refused');
      return current.map(value => value === device ? { ...value, helloEnrollmentAllowed: false, helloCredential: { id: credential.id, keyType: credential.keyType, first: credential.first, second: credential.second } } : value);
    });
  }
  issue(name) {
    if (typeof name !== 'string' || !name.trim() || name.length > 80) throw new Error('invalid-device');
    for (const [id, entry] of this.pairings) if (entry.expires < this.clock()) this.pairings.delete(id);
    if (this.pairings.size >= 8) throw new Error('pairing-limit');
    const code = randomBytes(18).toString('base64url'); this.pairings.set(digest(code), { name: name.trim(), profile: this.store.account?.profile, expires: this.clock() + 300000 });
    return { code, expiresInSeconds: 300 };
  }
  pair(code) {
    const action = this.tail.then(async () => {
      this.attempts = this.attempts.filter(time => time > this.clock() - 60000);
      if (this.attempts.length >= 10) throw new Error('pairing-rate-limit'); this.attempts.push(this.clock());
      const id = digest(String(code || '')); const entry = this.pairings.get(id); this.pairings.delete(id);
      if (!entry || entry.expires < this.clock() || entry.profile !== this.store.account?.profile) throw new Error('pairing-refused');
      if (this.store.devices.length >= 32) throw new Error('device-limit');
      const token = 'va1.' + randomUUID() + '.' + randomBytes(32).toString('base64url');
      const device = { id: token.split('.')[1], name: entry.name, profile: entry.profile, created: new Date(this.clock()).toISOString(), hash: digest(token) };
      await this.store.updateDevices(current => {
        if (entry.profile !== this.store.account?.profile) throw new Error('pairing-refused');
        return [...current.filter(value => value.profile === entry.profile), device];
      });
      return { token, deviceId: device.id };
    }); this.tail = action.catch(() => {}); return action;
  }
  authenticate(header) {
    const token = /^Bearer (va1\.[a-f0-9-]{36}\.[A-Za-z0-9_-]{43})$/.exec(header || '')?.[1];
    if (!token) throw new Error('unauthorized');
    const device = this.store.devices.find(value => value.id === token.split('.')[1]);
    if (!device || device.profile !== this.store.account?.profile || !same(device.hash, digest(token))) throw new Error('unauthorized'); return device.id;
  }
  async revoke(id) { await this.store.updateDevices(current => current.filter(device => device.id !== id)); }
}
