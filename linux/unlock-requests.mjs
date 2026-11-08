/* Un client demande ; seule la session administrateur Authentik peut approuver. */
import { randomUUID, randomBytes, createPublicKey, verify, constants } from 'node:crypto';
export const decisionMessage = (origin, requestId, challenge, approve, remember, vaultId = 'default') => JSON.stringify(['VaultAgent.Unlock.v2', origin, vaultId, requestId, challenge, approve, remember]);

export class UnlockRequests {
  constructor(session, devices, clock = () => Date.now(), vaultId = 'default') { this.session = session; this.devices = devices; this.clock = clock; this.vaultId = vaultId; this.entries = new Map(); this.contexts = new Map(); }
  valid(entry) {
    this.session.status();
    return entry && entry.expires > this.clock() && entry.generation === this.session.generation &&
      entry.profile === this.session.store.account?.profile && this.devices.list().some(device => device.id === entry.deviceId);
  }
  prune() { for (const [id, entry] of this.entries) if (!this.valid(entry)) this.entries.delete(id); }
  public(entry) { return { vaultId: this.vaultId, requestId: entry.id, state: entry.state, expiresUtc: new Date(entry.expires).toISOString() }; }
  current(deviceId) { this.prune(); const entry = this.entries.get(deviceId); return entry ? this.public(entry) : null; }
  async request(deviceId) {
    this.prune();
    if (this.session.status().unlocked) return { state: 'unlocked', unlocked: true };
    if (!this.session.store.account) throw new Error('not-configured');
    const existing = this.entries.get(deviceId);
    // Une demande refusée reste consommée jusqu'à expiration, sans nouvelle notification.
    if (existing && (existing.state !== 'pending' || !this.devices.record?.(deviceId)?.autoUnlock)) return this.public(existing);
    if (!this.devices.list().some(device => device.id === deviceId)) throw new Error('unauthorized');
    const entry = existing || { id: randomUUID(), deviceId, profile: this.session.store.account.profile,
      generation: this.session.generation, expires: this.clock() + 300000, state: 'pending', challenge: randomBytes(32).toString('base64') };
    this.entries.set(deviceId, entry);
    if (this.devices.record?.(deviceId)?.autoUnlock) {
      entry.state = 'approving';
      try {
        await this.session.unlock();
        if (!this.valid(entry) || !this.devices.record(deviceId)?.autoUnlock) { this.session.lock(); throw new Error('request-expired'); }
        entry.state = 'approved'; return { ...this.public(entry), unlocked: true };
      } catch (error) { entry.state = 'failed'; throw error; }
    }
    return this.public(entry);
  }
  list() {
    if (this.session.status().unlocked) return [];
    this.prune(); const names = new Map(this.devices.list().map(device => [device.id, device.name]));
    return [...this.entries.values()].filter(entry => entry.state === 'pending').map(entry => ({ ...this.public(entry), challenge: entry.challenge, deviceName: names.get(entry.deviceId) }));
  }
  context(deviceId) {
    if (!this.devices.record(deviceId)?.approverPublicKey) throw new Error('approver-required');
    const entry = { requestId: 'reset-auto', challenge: randomBytes(32).toString('base64'), expires: this.clock() + 60000 };
    this.contexts.set(deviceId, entry); return { vaultId: this.vaultId, requestId: entry.requestId, challenge: entry.challenge };
  }
  async signedDecision(deviceId, input, origin) {
    const key = this.devices.record(deviceId)?.approverPublicKey;
    if (!key || typeof input.approve !== 'boolean' || typeof input.remember !== 'boolean' || typeof input.signature !== 'string') throw new Error('approver-required');
    this.prune();
    const reset = input.requestId === 'reset-auto';
    const entry = reset ? this.contexts.get(deviceId) : [...this.entries.values()].find(value => value.id === input.requestId && value.state === 'pending');
    if (!entry || entry.expires <= this.clock()) throw new Error('request-expired');
    const publicKey = createPublicKey({ key: Buffer.from(key, 'base64'), type: 'spki', format: 'der' });
    if (!verify('sha256', Buffer.from(decisionMessage(origin, input.requestId, entry.challenge, input.approve, input.remember, this.vaultId)), { key: publicKey, padding: constants.RSA_PKCS1_PSS_PADDING, saltLength: 32 }, Buffer.from(input.signature, 'base64'))) throw new Error('approval-refused');
    if (reset) { this.contexts.delete(deviceId); await this.devices.clearAutomatic(); return { automaticAcceptance: false }; }
    const result = await this.decide(input.requestId, input.approve);
    if (input.approve && input.remember) await this.devices.setAutomatic(entry.deviceId, true);
    return result;
  }
  async decide(requestId, approve, code) {
    this.prune(); const entry = [...this.entries.values()].find(value => value.id === requestId);
    if (!entry || entry.state !== 'pending' || typeof approve !== 'boolean') throw new Error('request-expired');
    entry.state = approve ? 'approving' : 'denied';
    if (!approve) return this.public(entry);
    try {
      await this.session.unlock(code);
      if (!this.valid(entry)) { this.session.lock(); throw new Error('request-expired'); }
      entry.state = 'approved'; return this.public(entry);
    } catch (error) { entry.state = 'failed'; throw error; }
  }
}
