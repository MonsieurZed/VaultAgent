import { test } from 'node:test';
import assert from 'node:assert/strict';
import { randomBytes } from 'node:crypto';
import { Session } from '../session.mjs';
import { applyWritePatch } from '../../fork/write-patch.mjs';
import { saveFromSource } from '../../remote-client/agent-tools.mjs';

const id = '00000000-0000-4000-8000-000000000001';
const operationId = '00000000-0000-4000-8000-000000000002';
const revision = '2026-11-08T00:00:00.000Z';
const secret = 'Fictif-seulement-42!';
function fixture() {
  const disk = {}; const items = []; const folders = []; let writes = 0; let fail = false;
  const store = { key: randomBytes(32), account: { profile: 'fake', timeoutMinutes: 15 }, async load(name, fallback) { return structuredClone(disk[name] ?? fallback); }, async save(name, value) { disk[name] = structuredClone(value); } };
  const engine = { alive: true, kill() {}, async request(request) {
    if (request.op === 'safe-update') {
      ++writes; const item = items.find(i => i.id === request.itemId); const previous = item.login?.password;
      applyWritePatch(item, request.expectedRevisionDate, request.patch);
      if ('password' in request.patch && previous && request.patch.password !== previous) item.passwordHistory = [{ password: previous, lastUsedDate: revision }, ...(item.passwordHistory || [])].slice(0, 5);
      item.revisionDate = '2026-11-08T00:01:00.000Z'; return { id: item.id };
    }
    if (request.path === '/sync') return {};
    if (request.path === '/list/object/items') return { data: structuredClone(items) };
    if (request.path === '/list/object/folders') return { data: structuredClone(folders) };
    if (request.method === 'POST' && request.path === '/object/folder') { ++writes; const folder = { id, name: request.body.name }; folders.push(folder); return folder; }
    if (request.method === 'POST' && request.path === '/object/item') { ++writes; if (fail) throw new Error(secret); const item = { ...structuredClone(request.body), id, revisionDate: revision }; items.push(item); return structuredClone(item); }
    if (request.path === '/object/item/' + id) return structuredClone(items.find(i => i.id === id));
    throw new Error('mock-route');
  } };
  const session = new Session(store); session.engine = engine; session.touch();
  return { session, disk, items, writes: () => writes, fail: () => fail = true };
}
const create = () => ({ operationId, action: 'create', item: { type: 1, name: 'Exemple factice', login: { username: 'test@example.invalid', password: secret, totp: 'FAKESEED', uris: [{ uri: 'https://example.test' }] } } });

test('création, vérification et répétition idempotente sans valeur dans reçus', async () => {
  const f = fixture(); const request = create(); const receipt = await f.session.write(request);
  assert.equal(receipt.verified, true); assert.equal(receipt.id, id); assert.equal(f.items[0].login.password, secret);
  assert.ok(!JSON.stringify(receipt).includes(secret)); assert.ok(!JSON.stringify(f.disk).includes(secret));
  assert.equal((await f.session.write(request)).replayed, true); assert.equal(f.writes(), 1);
  await assert.rejects(f.session.write({ ...request, item: { ...request.item, name: 'Autre' } }), /operation-conflict/);
  await assert.rejects(f.session.write({ ...request, operationId: '00000000-0000-4000-8000-000000000003' }), /duplicate-item/);
});
test('mise à jour ciblée garde passkeys, champs, pièces jointes et secrets non visés', async () => {
  const f = fixture(); await f.session.write(create());
  f.items[0].login.fido2Credentials = [{ credentialId: 'fake', keyValue: 'dummy-key' }]; f.items[0].login.autofillOnPageLoad = true;
  f.items[0].fields = [{ type: 1, name: 'Secret', value: 'dummy-field' }]; f.items[0].attachments = [{ id: 'dummy-attachment', fileName: 'Exemple' }];
  const original = structuredClone(f.items[0]);
  const receipt = await f.session.write({ action: 'update', operationId: '00000000-0000-4000-8000-000000000004', itemId: id, expectedRevisionDate: revision, patch: { name: 'Renommé', password: 'Fictif-nouveau' } });
  assert.equal(receipt.verified, true); assert.equal(f.items[0].login.password, 'Fictif-nouveau');
  assert.deepEqual(f.items[0].login.fido2Credentials, original.login.fido2Credentials); assert.equal(f.items[0].login.totp, original.login.totp);
  assert.deepEqual(f.items[0].fields, original.fields); assert.deepEqual(f.items[0].attachments, original.attachments);
  await assert.rejects(f.session.write({ action: 'update', operationId: '00000000-0000-4000-8000-000000000005', itemId: id, expectedRevisionDate: revision, patch: { password: 'Fictif-refusé' } }), /revision-conflict/);
});
test('note sécurisée et prévisualisation n’exposent pas les notes', async () => {
  const f = fixture(); await f.session.write({ operationId, action: 'create', item: { type: 2, name: 'Note exemple', notes: secret } });
  const preview = await f.session.writePreview({}); assert.equal(preview.items[0].type, 2); assert.ok(!JSON.stringify(preview).includes(secret));
  assert.deepEqual(f.items[0].secureNote, { type: 0 });
});
test('création de dossier réutilise un dossier existant sans doublon', async () => {
  const f = fixture(); const input = { operationId, action: 'create-folder', item: { name: 'Exemples' } };
  assert.equal((await f.session.write(input)).verified, true);
  assert.equal((await f.session.write({ ...input, operationId: '00000000-0000-4000-8000-000000000009' })).id, id);
  assert.equal(f.writes(), 1); assert.deepEqual((await f.session.writePreview({})).folders, [{ id, name: 'Exemples' }]);
});
test('panne après intention persistée refuse toute répétition aveugle', async () => {
  const f = fixture(); f.fail();
  await assert.rejects(f.session.write(create()), error => error.message === 'write-indeterminate');
  await assert.rejects(f.session.write(create()), /write-indeterminate/); assert.equal(f.writes(), 1);
});
test('schémas, état verrouillé et autorisation restent stricts', async () => {
  const f = fixture();
  await assert.rejects(f.session.write({ ...create(), get: 'password' }), /invalid-write/);
  await assert.rejects(f.session.write({ action: 'update', operationId, itemId: id, expectedRevisionDate: revision, patch: { fields: [] } }), /invalid-patch/);
  await assert.rejects(f.session.write(create(), () => { throw new Error('unauthorized'); }), /unauthorized/);
  f.session.lock(); await assert.rejects(f.session.write(create()), /locked/); assert.equal(f.writes(), 0);
});
test('le helper masque aussi une exception de source contenant un secret', async () => {
  await assert.rejects(saveFromSource({ source: async () => { throw new Error(secret); } }), error => !error.message.includes(secret));
});
test('CipherView est modifié sans reconstruire le sous-objet login', () => {
  const login = { password: secret, fido2Credentials: [{ keyValue: 'fake' }], passwordRevisionDate: revision }; const view = { type: 1, login, revisionDate: new Date(revision) };
  applyWritePatch(view, revision, { password: 'new-fake', totp: 'FAKE' }); assert.equal(view.login, login); assert.equal(view.login.fido2Credentials[0].keyValue, 'fake');
  assert.throws(() => applyWritePatch({ ...view, organizationId: 'shared' }, revision, { name: 'Refusé' }), /item-refused/);
});
