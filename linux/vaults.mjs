/* Chaque compte garde son moteur, son stockage, ses clients et son delai propres. */
import { randomUUID } from 'node:crypto';
import { join } from 'node:path';
import { Store } from './store.mjs';
import { Session } from './session.mjs';
import { Devices } from './devices.mjs';
import { UnlockRequests } from './unlock-requests.mjs';

export class Vaults {
  constructor(root, factory) { this.root = root; this.factory = factory; this.contexts = new Map(); this.tail = Promise.resolve(); }
  async init() {
    this.registry = await this.root.load('vaults.json', [{ id: 'default', name: 'Coffre existant' }]);
    if (!Array.isArray(this.registry) || !this.registry.length || this.registry.length > 8 || new Set(this.registry.map(entry => entry.id)).size !== this.registry.length || !this.registry.some(entry => entry.id === 'default')) throw new Error('invalid-vault-registry');
    for (const entry of this.registry) {
      if (entry.id !== 'default' && !/^[a-f0-9]{8}(?:-[a-f0-9]{4}){3}-[a-f0-9]{12}$/i.test(entry.id)) throw new Error('invalid-vault');
      const store = entry.id === 'default' ? this.root : await new Store(join(this.root.directory, 'vaults', entry.id)).init();
      this.attach(entry, store);
    }
    return this;
  }
  attach(entry, store) {
    const session = new Session(store, profile => this.factory(store.directory, profile));
    const devices = new Devices(store); const approvals = new UnlockRequests(session, devices, undefined, entry.id);
    const context = { ...entry, store, session, devices, approvals }; this.contexts.set(entry.id, context); return context;
  }
  metadata(context) { return { id: context.id, name: context.name, ...context.session.status() }; }
  list() { return [...this.contexts.values()].map(context => this.metadata(context)); }
  get(id = 'default') { const context = this.contexts.get(id); if (!context) throw new Error('vault-missing'); return context; }
  authorized(header) { return [...this.contexts.values()].filter(context => { try { context.devices.authenticate(header); return true; } catch { return false; } }); }
  select(header, id) {
    const allowed = this.authorized(header); if (!allowed.length) throw new Error('unauthorized');
    if (id === undefined && allowed.length !== 1) throw new Error('vault-required');
    const context = id === undefined ? allowed[0] : allowed.find(value => value.id === id);
    if (!context) throw new Error('unauthorized'); return context;
  }
  async saveName(id, name) {
    if (typeof name !== 'string' || !name.trim() || name.length > 80 || /[\x00-\x1f]/.test(name)) throw new Error('invalid-vault-name');
    if (id === undefined && this.contexts.size >= 8) throw new Error('vault-limit');
    let context; const adding = id === undefined;
    if (id === undefined) { id = randomUUID(); context = this.attach({ id, name: name.trim() }, await new Store(join(this.root.directory, 'vaults', id)).init()); }
    else context = this.get(id);
    const previousName = context.name; context.name = name.trim(); const registry = [...this.contexts.values()].map(({ id, name }) => ({ id, name }));
    try { await this.root.save('vaults.json', registry); this.registry = registry; }
    catch (error) { if (adding) this.contexts.delete(context.id); else context.name = previousName; throw error; }
    return this.metadata(context);
  }
  save(input) { const task = this.tail.then(() => this.saveName(input.vaultId, input.name)); this.tail = task.catch(() => {}); return task; }
  configure(context, input) {
    const task = this.tail.then(async () => {
      if ([...this.contexts.values()].some(other => other !== context && other.store.account && new URL(other.store.account.server).origin === new URL(input.server).origin && other.store.account.email.toLowerCase() === String(input.email).toLowerCase())) throw new Error('duplicate-account');
      return context.session.configure(input);
    }); this.tail = task.catch(() => {}); return task;
  }
  availableClients() {
    const clients = new Map();
    for (const context of this.contexts.values()) for (const device of context.devices.list()) if (!clients.has(device.id)) clients.set(device.id, { id: device.id, name: device.name });
    return [...clients.values()];
  }
  async grant(context, deviceId) {
    if (!context.store.account) throw new Error('not-configured');
    const profile = context.store.account.profile;
    const source = [...this.contexts.values()].map(value => value.devices.record(deviceId)).find(Boolean);
    if (!source) throw new Error('invalid-device');
    if (context.devices.record(deviceId)) return { granted: true };
    await context.store.updateDevices(current => {
      if (context.store.account.profile !== profile) throw new Error('cancelled');
      if (current.length >= 32) throw new Error('device-limit');
      return [...current, { ...source, profile, autoUnlock: false }];
    }); return { granted: true };
  }
  expire() { for (const context of this.contexts.values()) context.session.expire(); }
  lock() { for (const context of this.contexts.values()) context.session.lock(); }
}
