import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { randomUUID } from 'node:crypto';
import { Store } from '../store.mjs';
import { Vaults } from '../vaults.mjs';
import { createApp } from '../http.mjs';
import { applyWritePatch } from '../../fork/write-patch.mjs';

const orgId = '10000000-0000-4000-8000-000000000001';
const collectionId = '20000000-0000-4000-8000-000000000001';
const originalPassword = 'Fictif-ancien-42!'; const changedPassword = 'Fictif-modifie-42!';
async function fixture(t) {
  const directory = await mkdtemp(join(tmpdir(), 'vaultagent-multi-test-')); t.after(() => rm(directory, { recursive: true, force: true }));
  const root = await new Store(directory).init(); const items = new Map(); let revision = 0; let moves = 0;
  const permissions = { 'admin@example.test': 'edit', 'agent@example.test': 'edit' };
  const collections = [{ id: collectionId, name: 'Communs' }];
  const engines = [];
  const factory = async (directory, profile) => {
    const engine = { directory, profile, alive: true, cache: new Map(), kill() { this.alive = false; }, async request(request) {
      if (request.op === 'prepare') return { restartRequired: false };
      if (request.op === 'connect') { assert.equal(request.password, 'Fictif-maitre-42!'); this.email = request.email; return {}; }
      if (request.op === 'sharing-info') return permissions[this.email] ? [{ id: orgId, name: 'Famille factice', role: this.email.startsWith('admin') ? 0 : 2, collections: collections.map(row => ({ ...row, canEdit: permissions[this.email] === 'edit' })) }] : [];
      if (request.op === 'sharing-check') { if (request.organizationId !== orgId || permissions[this.email] !== 'edit' || request.collectionIds.some(id => !collections.some(row => row.id === id))) throw new Error('refused'); return { allowed: true }; }
      if (request.op === 'safe-update') {
        const source = items.get(request.itemId); if (!source || source.organizationId && permissions[this.email] !== 'edit') throw new Error('refused');
        const copy = structuredClone(source); applyWritePatch(copy, request.expectedRevisionDate, request.patch, request.organizationId ?? null);
        if ('password' in request.patch && source.login?.password && source.login.password !== copy.login.password) {
          copy.passwordHistory = [{ password: source.login.password, lastUsedDate: new Date().toISOString() }, ...(source.passwordHistory || []).slice(0, 4)];
          copy.login.passwordRevisionDate = new Date().toISOString();
        }
        copy.revisionDate = new Date(++revision * 1000).toISOString(); items.set(copy.id, copy); this.cache.set(copy.id, structuredClone(copy)); return { id: copy.id };
      }
      if (request.path === '/sync') { this.cache = new Map([...items].filter(([, item]) => item.organizationId ? !!permissions[this.email] : item.owner === this.email).map(([id, item]) => [id, structuredClone(item)])); return {}; }
      if (request.path.startsWith('/list/object/items')) return { data: [...this.cache.values()].map(value => structuredClone(value)) };
      if (request.path === '/list/object/folders') return { data: [] };
      if (request.path === '/object/item' && request.method === 'POST') {
        if (request.body.organizationId && permissions[this.email] !== 'edit') throw new Error('refused');
        const item = { ...structuredClone(request.body), id: randomUUID(), owner: this.email, revisionDate: new Date(++revision * 1000).toISOString() }; items.set(item.id, item); this.cache.set(item.id, structuredClone(item)); return structuredClone(item);
      }
      if (request.path.startsWith('/object/item/')) { const item = this.cache.get(request.path.split('/').at(-1)); if (!item) throw new Error('not-found'); return structuredClone(item); }
      if (request.path.startsWith('/move/')) {
        ++moves; const id = request.path.split('/')[2]; const item = items.get(id); item.organizationId = orgId; item.collectionIds = request.body; item.revisionDate = new Date(++revision * 1000).toISOString(); return structuredClone(item);
      }
      if (request.path === '/object/org-collection') { const item = { id: randomUUID(), name: request.body.name }; collections.push(item); return item; }
      throw new Error('mock-route');
    } }; engines.push(engine); return engine;
  };
  const pool = await new Vaults(root, factory).init(); t.after(() => pool.lock());
  const admin = pool.get(); const added = await pool.save({ name: 'Agent' }); const agent = pool.get(added.id);
  for (const [context, email] of [[admin, 'admin@example.test'], [agent, 'agent@example.test']]) await context.session.configure({ server: 'https://vault.example.test', email, password: 'Fictif-maitre-42!', timeoutMinutes: 15 });
  const paired = await admin.devices.pair(admin.devices.issue('PC factice').code); const header = 'Bearer ' + paired.token;
  return { pool, root, admin, agent, header, paired, items, permissions, engines, moves: () => moves };
}
test('coffres isolés, migration du compte existant et sélection explicite sans élargissement automatique', async t => {
  const f = await fixture(t); assert.equal(f.admin.store, f.root); assert.notEqual(f.admin.store.directory, f.agent.store.directory);
  assert.equal(f.pool.select(f.header).id, 'default'); assert.throws(() => f.pool.select(f.header, f.agent.id), /unauthorized/);
  await assert.rejects(f.pool.configure(f.agent, { server: 'https://vault.example.test', email: 'admin@example.test', password: 'Fictif-maitre-42!', timeoutMinutes: 15 }), /duplicate-account/);
  await f.admin.devices.setAutomatic(f.paired.deviceId, true); await f.pool.grant(f.agent, f.paired.deviceId);
  assert.equal(f.agent.devices.record(f.paired.deviceId).autoUnlock, false); assert.throws(() => f.pool.select(f.header), /vault-required/);
  assert.equal(f.pool.select(f.header, f.agent.id), f.agent);
  f.agent.session.lock(); assert.equal(f.admin.session.status().unlocked, true);
  await f.agent.devices.revoke(f.paired.deviceId); assert.equal(f.pool.select(f.header).id, 'default');
  const restored = await new Vaults(await new Store(f.root.directory).init(), () => { throw new Error('must-not-open'); }).init();
  assert.equal(restored.contexts.size, 2); assert.equal(restored.get(f.agent.id).name, 'Agent'); assert.equal(restored.get().session.status().unlocked, false);
});
test('une fiche partagée créée par Admin est modifiée par Agent puis relue à jour par Admin', async t => {
  const f = await fixture(t);
  const created = await f.admin.session.write({ operationId: randomUUID(), action: 'create', item: { type: 1, name: 'Connexion commune', organizationId: orgId, collectionIds: [collectionId], login: { password: originalPassword, uris: [{ uri: 'https://example.test' }] } } });
  const catalog = await f.agent.session.catalog(); assert.equal(catalog[0].id, created.id); assert.equal(catalog[0].organizationId, orgId);
  assert.ok(!JSON.stringify(catalog).includes(originalPassword));
  const before = await f.agent.session.writePreview({ itemId: created.id });
  const changed = await f.agent.session.write({ operationId: randomUUID(), action: 'update', itemId: created.id, expectedRevisionDate: before.revisionDate, organizationId: orgId, patch: { password: changedPassword } });
  assert.equal(changed.verified, true); assert.equal(f.items.size, 1); assert.equal(f.items.get(created.id).login.password, changedPassword);
  const fresh = await f.admin.session.writePreview({ itemId: created.id }); assert.notEqual(fresh.revisionDate, before.revisionDate);
  f.permissions['agent@example.test'] = undefined; assert.deepEqual(await f.agent.session.catalog(), []);
  await assert.rejects(f.agent.session.secret({ itemId: created.id, field: 'password', origin: 'https://example.test', publicKey: 'unused' }), /not-found/);
});
test('lecture seule refuse la création et la modification de fiches communes', async t => {
  const f = await fixture(t);
  const created = await f.admin.session.write({ operationId: randomUUID(), action: 'create', item: { type: 1, name: 'Commun lecture seule', organizationId: orgId, collectionIds: [collectionId], login: { password: originalPassword } } });
  f.permissions['agent@example.test'] = 'read';
  await assert.rejects(f.agent.session.write({ operationId: randomUUID(), action: 'create', item: { type: 1, name: 'Refuse', organizationId: orgId, collectionIds: [collectionId], login: {} } }), /refused/);
  const before = await f.agent.session.writePreview({ itemId: created.id });
  await assert.rejects(f.agent.session.write({ operationId: randomUUID(), action: 'update', itemId: created.id, expectedRevisionDate: before.revisionDate, organizationId: orgId, patch: { password: changedPassword } }));
  assert.equal(f.items.get(created.id).login.password, originalPassword); assert.equal(f.items.size, 1);
});
test('partage natif transfère une seule fiche après confirmation, puis retourne le même reçu', async t => {
  const f = await fixture(t); const created = await f.admin.session.write({ operationId: randomUUID(), action: 'create', item: { type: 1, name: 'Personnel vers commun', login: { password: originalPassword } } });
  const plan = { itemId: created.id, organizationId: orgId, collectionIds: [collectionId] };
  const preview = await f.admin.session.share({ ...plan, action: 'preview' }); assert.equal(preview.transfersOwnership, true); assert.ok(!JSON.stringify(preview).includes(originalPassword));
  const input = { ...plan, action: 'apply', operationId: randomUUID(), expectedRevisionDate: preview.expectedRevisionDate };
  await assert.rejects(f.admin.session.share(input), /revision-conflict/); assert.equal(f.moves(), 0);
  input.confirmOwnershipTransfer = true; const receipt = await f.admin.session.share(input); assert.equal(receipt.verified, true);
  assert.equal((await f.admin.session.share(input)).replayed, true); assert.equal(f.moves(), 1); assert.equal(f.items.size, 1);
  assert.equal((await f.agent.session.catalog())[0].id, created.id);
});
test('API refuse un coffre non autorisé et une sélection absente si plusieurs sont accessibles', async t => {
  const f = await fixture(t); const server = createApp({ vaults: f.pool, origin: 'https://service.example.test', owner: 'Owner', trustedProxy: '127.0.0.1' });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve)); t.after(() => new Promise(resolve => server.close(resolve)));
  const call = (path, input) => fetch('http://127.0.0.1:' + server.address().port + path, { method: 'POST', headers: { authorization: f.header, 'content-type': 'application/json' }, body: JSON.stringify(input) });
  assert.equal((await call('/api/agent/status', { vaultId: f.agent.id })).status, 403);
  await f.pool.grant(f.agent, f.paired.deviceId);
  assert.equal((await call('/api/agent/status', {})).status, 400); assert.equal((await call('/api/agent/status', { vaultId: f.agent.id })).status, 200);
  assert.equal((await (await call('/api/agent/vaults', {})).json()).length, 2);
  assert.equal((await call('/api/agent/write', { vaultId: f.agent.id, action: 'create', operationId: randomUUID(), item: { type: 2, name: 'Note factice', notes: originalPassword } })).status, 200);
});
test('création de collection vérifiée et relancée avec le même reçu sans doublon', async t => {
  const f = await fixture(t); const input = { action: 'create-collection', operationId: randomUUID(), organizationId: orgId, name: 'Autre commun' };
  const receipt = await f.admin.session.share(input); assert.equal(receipt.verified, true);
  assert.equal((await f.admin.session.share(input)).replayed, true);
  const info = await f.agent.session.sharing(); assert.equal(info.organizations[0].collections.filter(row => row.name === input.name).length, 1);
});
